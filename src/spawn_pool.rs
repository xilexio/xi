use crate::creeps::{find_idle_creep, CreepRef};
use crate::game_time::game_tick;
use crate::kernel::process_handle::ProcessHandle;
use crate::kernel::{current_process_wrapped_meta, kill, schedule};
use crate::spawning::{cancel_scheduled_creep, schedule_creep, SpawnPromise, SpawnRequest};
use crate::travel::{travel, TravelSpec};
use crate::u;
use log::{debug, trace};
use screeps::RoomName;
use std::cell::RefCell;
use std::future::Future;
use std::rc::Rc;

// TODO Cancel spawns on drop.
pub struct SpawnPool {
    base_spawn_request: SpawnRequest,
    current_creep_and_process: Option<(CreepRef, ProcessHandle<()>)>,
    prespawned_creep: Option<MaybeSpawned>,
    room_name: RoomName,
    travel_spec: Option<TravelSpec>,
}

pub enum MaybeSpawned {
    Spawned(CreepRef),
    Spawning(Rc<RefCell<SpawnPromise>>),
}

impl MaybeSpawned {
    pub fn as_ref(&self) -> Option<&CreepRef> {
        match self {
            Self::Spawned(creep_ref) => Some(creep_ref),
            Self::Spawning(_) => None,
        }
    }
}

impl Drop for SpawnPool {
    /// Removing all scheduled spawns when dropping the spawn pool. If the drop is not called, the creeps will simply be
    /// spawned and potentially wasted.
    fn drop(&mut self) {
        debug!("Dropping a spawn pool in {} for {}.", self.room_name, self.base_spawn_request.role);
        if let Some(MaybeSpawned::Spawning(prespawned_creep)) = self.prespawned_creep.take() {
            cancel_scheduled_creep(self.room_name, prespawned_creep);
        }
    }
}

impl SpawnPool {
    pub fn new(room_name: RoomName, base_spawn_request: SpawnRequest, travel_spec: Option<TravelSpec>) -> Self {
        Self {
            base_spawn_request,
            current_creep_and_process: None,
            prespawned_creep: None,
            room_name,
            travel_spec,
        }
    }

    /// Keeps a creep with given parameters spawned and optionally prespawning and travelling to given position.
    /// When the creep is spawned, it creates a future using `creep_future_constructor` and runs it until the creep
    /// dies. When it dies, it kills it. It is guaranteed that the creep exists in each tick the future is ran.
    /// It is guaranteed that only one constructed future exists at a time, per spawn pool.
    /// This function must be called at least once each tick for the scheduling and spawning to work correctly.
    pub fn with_spawned_creep<G, F>(&mut self, creep_future_constructor: G)
    where
        G: FnOnce(CreepRef) -> F,
        F: Future<Output = ()> + 'static,
    {
        // If the current creep is dead, killing its process and discarding its information.
        if let Some((current_creep, current_process)) = self.current_creep_and_process.as_ref() {
            if !current_creep.borrow().exists() {
                trace!(
                    "A current {:?} creep from the spawn pool died.",
                    self.base_spawn_request.role
                );
                kill(u!(self.current_creep_and_process.take()).1, ());
            }
        }

        // If there is a prespawned creep, we check if it spawned already and handle its movement to
        // the target location (if supplied). At the beginning we also use this to spawn the first
        // creep.
        if let Some(prespawned_creep) = self.prespawned_creep.as_ref() {
            match prespawned_creep {
                MaybeSpawned::Spawned(creep_ref) => {
                    // If the creep is spawned, it should be travelling to its target (if needed)
                    // and there is nothing to do with it aside from checking whether it is still
                    // alive.
                    if !creep_ref.borrow().exists() {
                        self.prespawned_creep = None;
                        debug!(
                            "A prespawned {:?} creep from the spawn pool died.",
                            self.base_spawn_request.role
                        );
                    }
                }
                MaybeSpawned::Spawning(spawn_promise) => {
                    let mut borrowed_spawn_promise = spawn_promise.borrow_mut();
                    if borrowed_spawn_promise.cancelled {
                        // The spawn request was cancelled (externally).
                        drop(borrowed_spawn_promise);
                        self.prespawned_creep = None;
                        debug!(
                            "Spawn request of {:?} creep from the spawn pool was cancelled.",
                            self.base_spawn_request.role
                        );
                    } else if let Some(creep_ref) = borrowed_spawn_promise.creep.take() {
                        // The prespawned creep is expected to be alive now. Making it travel to the
                        // target point if there is one. Note that this happens even if the creep is
                        // being used as the new current creep right away.
                        // TODO This relies on spawn_end_tick being updated properly. Check if this is the case.
                        drop(borrowed_spawn_promise);
                        if let Some(travel_spec) = self.travel_spec.as_ref() {
                            travel(&creep_ref, travel_spec.clone());
                        }
                        self.prespawned_creep = Some(MaybeSpawned::Spawned(creep_ref));
                        trace!(
                            "A prespawned {:?} creep from the spawn pool has spawned.",
                            self.base_spawn_request.role
                        );
                    }
                }
            }
        }
        
        // If there is no current creep (before the first one spawns or after the previous one
        // dies), making the prespawned one the current one if it already spawned and, if
        // travel_spec was added, at the destination.
        if self.current_creep_and_process.is_none() {
            let existing_creep = match self.prespawned_creep.take() {
                None => {
                    // This is the case after a reset or after it was impossible to spawn a creep
                    // for a long time. Trying to get an existing creep before spawning a new one.
                    // If that fails, a new one will be scheduled.
                    // TODO Actually implement `find_idle_creep` and add a way to inform of minimum acceptable time to live.
                    // TODO Also try to find an already spawning creep, but only if it is less than 50*3 ticks since restart.
                    find_idle_creep(
                        self.room_name,
                        self.base_spawn_request.role,
                        &self.base_spawn_request.body,
                        self.travel_spec.as_ref().map(|travel_spec| travel_spec.target.xy()),
                    ).map(|creep| {
                        debug!("Found idle {:?} creep.", self.base_spawn_request.role);
                        creep
                    })
                }
                Some(MaybeSpawned::Spawned(prespawned_creep)) => {
                    // The prespawned creep is ready to become the current creep. It is likely
                    // already travelling to the destination, if it was specified.
                    // Note that in this case the creep is guaranteed to exist, as we checked that
                    // above and would have set the prespawned creep to None if it did not.
                    // TODO if the previous creep died too early, check if the tick is updated to now. If not, reschedule.
                    Some(prespawned_creep)
                }
                Some(prespawned_creep) => {
                    // The case when the creep is still spawning. It can happen in the same cases as
                    // None. Assigning the still spawning prespawned creep back and waiting until it
                    // spawns.
                    self.prespawned_creep = Some(prespawned_creep);
                    None
                }
            };

            if let Some(creep_ref) = existing_creep {
                // Replacing the current creep with the one found above. 
                // Running the user code on the current creep by constructing the future and
                // scheduling it.
                let future = creep_future_constructor(creep_ref.clone());
                let wrapper_priority = current_process_wrapped_meta().borrow().priority;
                let current_process = schedule(
                    "spawn_pool_creep_process",
                    wrapper_priority.saturating_sub(1),
                    future,
                );
                self.current_creep_and_process = Some((creep_ref, current_process));
            }
        }
        
        if self.prespawned_creep.is_none() {
            // Scheduling the spawning of a creep, both as prespawning when there is already a creep
            // alive and when there is none. The difference is that in the latter case, we want the
            // creep spawned as fast as possible.
            let mut spawn_request = self.base_spawn_request.clone();
            if let Some((current_creep, current_process)) = self.current_creep_and_process.as_ref() {
                // The prespawning case.
                let creep_death_tick = game_tick() + current_creep.borrow().ticks_to_live();
                // TODO Cache this, maybe just by moving out of the scope of the loop.
                let creep_travel_ticks = self
                    .travel_spec
                    .as_ref()
                    .map(|travel_spec| {
                        0 // TODO
                    })
                    .unwrap_or(0);

                let min_preferred_tick = creep_death_tick - creep_travel_ticks;
                let max_preferred_tick = min_preferred_tick + self.base_spawn_request.preferred_tick.1
                    - self.base_spawn_request.preferred_tick.0;
                spawn_request.preferred_tick = (min_preferred_tick, max_preferred_tick);
            } else {
                // The case with spawning as fast as possible.
                let min_preferred_tick = game_tick();
                let max_preferred_tick = min_preferred_tick;
                spawn_request.preferred_tick = (min_preferred_tick, max_preferred_tick);
            }
            
            // Scheduling the creep.
            if let Some(spawn_promise) = schedule_creep(self.room_name, spawn_request) {
                self.prespawned_creep = Some(MaybeSpawned::Spawning(spawn_promise));
                debug!(
                    "Scheduled a spawn of {:?} creep from the spawn pool.",
                    self.base_spawn_request.role
                );
            } else {
                // Scheduling failed. Attempting it next time.
                debug!(
                    "Failed to schedule the spawning of {:?} creep from the spawn pool.",
                    self.base_spawn_request.role
                );
            }
        }
    }
}
