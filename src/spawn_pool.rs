use crate::config::SPAWN_SCHEDULE_TICKS;
use crate::creeps::{find_idle_creep, CreepRef};
use crate::game_time::game_tick;
use crate::kernel::process_handle::ProcessHandle;
use crate::kernel::{current_process_wrapped_meta, kill, schedule};
use crate::spawning::{cancel_scheduled_creep, schedule_creep, SpawnPromise, SpawnRequest};
use crate::travel::{travel, TravelSpec};
use crate::{a, u};
use log::{debug, trace};
use screeps::RoomName;
use std::cell::RefCell;
use std::collections::VecDeque;
use std::future::Future;
use std::rc::Rc;

// TODO Cancel spawns on drop.
pub struct SpawnPool {
    base_spawn_request: SpawnRequest,
    current_creep_and_process: Option<(CreepRef, ProcessHandle<()>)>,
    prespawned_creep: Option<MaybeSpawned>,
    subsequent_creeps: VecDeque<Rc<RefCell<SpawnPromise>>>,
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
        for spawn_promise in self.subsequent_creeps.drain(..) {
            cancel_scheduled_creep(self.room_name, spawn_promise);
        }
    }
}

impl SpawnPool {
    pub fn new(room_name: RoomName, base_spawn_request: SpawnRequest, travel_spec: Option<TravelSpec>) -> Self {
        Self {
            base_spawn_request,
            current_creep_and_process: None,
            prespawned_creep: None,
            subsequent_creeps: VecDeque::new(),
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

        // If there is a prespawned creep, we check if it spawned already and handle its movement to target location
        // if supplied. At the beginning we also use this to spawn the first creep.
        if let Some(prespawned_creep) = self.prespawned_creep.as_ref() {
            match prespawned_creep {
                MaybeSpawned::Spawned(creep_ref) => {
                    // If the creep is spawned, it should be travelling to its target (if needed) and there is
                    // nothing to do with it aside from checking whether it is still alive.
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
                        // The spawn request was cancelled. Cancelling every subsequent spawn request to reissue
                        // them later.
                        debug!(
                            "Spawning of a prespawned {:?} creep from the spawn pool was cancelled. Cancelling subsequent spawns.",
                            self.base_spawn_request.role
                        );
                        for spawn_promise in self.subsequent_creeps.drain(..) {
                            cancel_scheduled_creep(self.room_name, spawn_promise);
                        }
                    } else if borrowed_spawn_promise.spawn_end_tick >= game_tick() {
                        // The prespawned creep is expected to be alive now. Making it travel to the target point
                        // if there is one. Note that this happens even if the creep is being used as the new current
                        // creep right away.
                        let creep_ref = u!(borrowed_spawn_promise.creep.take());
                        drop(borrowed_spawn_promise);
                        if let Some(travel_spec) = self.travel_spec.as_ref() {
                            travel(&creep_ref, travel_spec.clone());
                        }
                        self.prespawned_creep = Some(MaybeSpawned::Spawned(creep_ref));
                        trace!(
                            "A prespawned {:?} creep from the spawn pool spawned.",
                            self.base_spawn_request.role
                        );
                    }
                }
            }
        }

        // If there is no current creep (before the first one spawns or after the previous one dies), making the
        // prespawned one the current one if it already spawned or spawning one.
        if self.current_creep_and_process.is_none() {
            let existing_creep = match self.prespawned_creep.take() {
                None => {
                    // This is the rare case when the first creep is spawned, it is freshly after a reset or it was not
                    // possible to prespawn a creep for a longer time.
                    // Trying to get an existing creep before spawning a new one. If that fails, a new one will be
                    // scheduled.
                    // TODO Actually implement `find_idle_creep` and add a way to inform of minimum acceptable time to live.
                    // TODO Also try to find an already spawning creep, but only if it is less than 50*3 ticks since restart.
                    find_idle_creep(
                        self.room_name,
                        self.base_spawn_request.role,
                        &self.base_spawn_request.body,
                        self.travel_spec.as_ref().map(|travel_spec| travel_spec.target.xy()),
                    )
                }
                Some(MaybeSpawned::Spawned(prespawned_creep)) => {
                    // The prespawned creep is ready to become the current creep.
                    // Note that it is guaranteed to exist, as we checked that above and would have set the prespawned
                    // creep to None if it did not.
                    Some(prespawned_creep)
                }
                Some(prespawned_creep) => {
                    // The rare case when the creep is still spawning. It can happen in the same cases as None.
                    // Assigning the still spawning prespawned creep back and waiting until it spawns.
                    self.prespawned_creep = Some(prespawned_creep);
                    None
                }
            };

            // If we have a spawned creep (found an existing one or the prespawned one spawned), we set it as the
            // current creep. If we do not, we schedule the first creep. Note that we do not spawn subsequent creeps
            // yet, this happens when we already have a creep and know its lifetime for sure.
            match existing_creep {
                Some(creep_ref) => {
                    // We have an already existing creep. Making it the current one.
                    // Running the user code on the current creep by constructing the future and constructing it.
                    let future = creep_future_constructor(creep_ref.clone());
                    let wrapper_process_priority = current_process_wrapped_meta().borrow().priority;
                    let current_process = schedule(
                        "spawn_pool_creep_process",
                        wrapper_process_priority.saturating_sub(1),
                        future,
                    );
                    self.current_creep_and_process = Some((creep_ref, current_process));
                }
                None => {
                    if self.prespawned_creep.is_none() {
                        // Assigning the prespawned creep if it is missing.
                        // This should only happen if prespawning failed.
                        if let Some(prespawned_creep) = self.subsequent_creeps.pop_front() {
                            self.prespawned_creep = Some(MaybeSpawned::Spawning(prespawned_creep));
                        }
                    } else {
                        // If we got here than we are just initializing everything or a few attempts of scheduling
                        // prespawned creeps failed in a row. We should not be here if there is a creep in the queue.
                        a!(self.subsequent_creeps.is_empty());

                        // We schedule the first spawn. The first creep scheduled is special in that we want it as fast
                        // as possible.
                        let mut spawn_request = self.base_spawn_request.clone();
                        let min_preferred_tick = game_tick();
                        let max_preferred_tick = min_preferred_tick + 1500; // TODO preferred now, no real limit
                        spawn_request.preferred_tick = (min_preferred_tick, max_preferred_tick);
                        if let Some(spawn_promise) = schedule_creep(self.room_name, spawn_request) {
                            self.subsequent_creeps.push_back(spawn_promise);
                        }
                    }
                }
            }
        }

        // The state when the creep is alive. We already checked if it is the case, both if it is the old current creep
        // or if it is a prespawned creep.
        if let Some((current_creep, current_process)) = self.current_creep_and_process.as_ref() {
            // Scheduling the spawns of subsequent creeps. These spawns may get cancelled, but we are guaranteed that
            // their order will be preserved.
            while self
                .subsequent_creeps
                .back()
                .map(|last_creep| last_creep.borrow().spawn_end_tick < game_tick() + SPAWN_SCHEDULE_TICKS)
                .unwrap_or(true)
            {
                let creep_lifetime = self.base_spawn_request.body.lifetime();
                let creep_death_tick = self
                    .subsequent_creeps
                    .back()
                    .map(|last_creep| last_creep.borrow().spawn_end_tick + creep_lifetime)
                    .unwrap_or(game_tick() + current_creep.borrow().ticks_to_live());
                let creep_travel_ticks = self
                    .travel_spec
                    .as_ref()
                    .map(|travel_spec| {
                        0 // TODO
                    })
                    .unwrap_or(0);

                let mut spawn_request = self.base_spawn_request.clone();
                let min_preferred_tick = creep_death_tick - creep_travel_ticks;
                let max_preferred_tick = min_preferred_tick + self.base_spawn_request.preferred_tick.1
                    - self.base_spawn_request.preferred_tick.0;
                spawn_request.preferred_tick = (min_preferred_tick, max_preferred_tick);
                // Scheduling the creep.
                if let Some(spawn_promise) = schedule_creep(self.room_name, spawn_request) {
                    self.subsequent_creeps.push_back(spawn_promise);
                } else {
                    // Scheduling failed. Attempting it next time.
                    break;
                }
            }

            // TODO Checking once in a while that the scheduled subsequent creeps are still scheduled and not cancelled.

            // Assigning the prespawned creep if it is missing.
            if self.prespawned_creep.is_none() {
                if let Some(prespawned_creep) = self.subsequent_creeps.pop_front() {
                    self.prespawned_creep = Some(MaybeSpawned::Spawning(prespawned_creep));
                }
            }
        }
    }
}
