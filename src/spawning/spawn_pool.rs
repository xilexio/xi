use crate::creeps::{find_idle_creep, CreepRef};
use crate::utils::game_tick::game_tick;
use crate::kernel::process_handle::ProcessHandle;
use crate::kernel::kernel::{current_process_wrapped_meta, kill, schedule};
use crate::travel::{predicted_travel_ticks, travel, TravelSpec};
use crate::{a, u};
use log::{debug, trace};
use screeps::RoomName;
use std::cell::RefCell;
use std::cmp::max;
use std::future::Future;
use std::rc::Rc;
use crate::economy::room_eco_stats::RoomCreepStats;
use crate::room_states::room_states::with_room_state;
use crate::spawning::reserved_creep::ReservedCreep;
use crate::spawning::scheduling_creeps::{cancel_scheduled_creep, schedule_creep};
use crate::spawning::spawn_schedule::{SpawnPromise, SpawnRequest};
use crate::utils::uid::UId;

#[derive(Debug, Clone)]
pub struct SpawnPoolOptions {
    travel_spec: Option<TravelSpec>,
    target_number_of_creeps: u32,
}

impl Default for SpawnPoolOptions {
    fn default() -> Self {
        Self {
            travel_spec: None,
            target_number_of_creeps: 1,
        }
    }
}

impl SpawnPoolOptions {
    pub fn travel_spec(self, value: Option<TravelSpec>) -> Self {
        Self {
            travel_spec: value,
            ..self
        }
    }

    pub fn target_number_of_creeps(self, value: u32) -> Self {
        Self {
            target_number_of_creeps: value,
            ..self
        }
    }
}

pub type WId = UId<'W'>;

/// A pool of dynamically configurable number of creeps with dynamically configurable body being
/// constantly spawned and prespawned in a room, executing given future using `with_spawned_creeps`.
#[derive(Debug)]
pub struct SpawnPool {
    id: WId,
    room_name: RoomName,
    /// The template spawn request used to spawn creeps. Only the tick is changed in the actual
    /// request. Everything but the role may be modified.
    pub base_spawn_request: SpawnRequest,
    pub travel_spec: Option<TravelSpec>,
    pub target_number_of_creeps: u32,
    current_creeps_and_processes: Vec<SpawnPoolElement>,
}

#[derive(Debug)]
pub struct SpawnPoolElement {
    current_creep_and_process: Option<(ReservedCreep, ProcessHandle<()>)>,
    prespawned_creep: Option<MaybeSpawned>,
    /// Will spawn subsequent creeps as long as this is true.
    respawn: bool,
}

#[derive(Debug)]
pub enum MaybeSpawned {
    Spawned(ReservedCreep),
    Spawning(SpawnRequest, Rc<RefCell<SpawnPromise>>),
}

impl Drop for SpawnPool {
    /// Removing all scheduled spawns when dropping the spawn pool.
    /// If the drop is not called, the creeps will simply be spawned and potentially wasted.
    /// The current process is killed. The current creep is released as a result of dropping
    /// `ReservedCreep`.
    /// The prespawned creep, if it exists, is also released.
    /// Scheduled spawns are cancelled, though they may still finish if the spawning is already
    /// occurring.
    fn drop(&mut self) {
        debug!("Dropping a spawn pool element in {} for {}.", self.room_name, self.base_spawn_request.role);

        for mut element in self.current_creeps_and_processes.drain(..) {
            // Cancelling scheduled spawns.
            if let Some(MaybeSpawned::Spawning(_, prespawned_creep)) = element.prespawned_creep.take() {
                cancel_scheduled_creep(self.room_name, prespawned_creep);
            }

            if let Some((_, current_process)) = element.current_creep_and_process.take() {
                kill(current_process, ());
            }
        }

        with_room_state(self.room_name, |room_state| {
            if let Some(room_eco_stats) = room_state.eco_stats.as_mut() {
                if let Some(role_stats) = room_eco_stats.creep_stats_by_role.get_mut(&self.base_spawn_request.role) {
                    role_stats.remove(&self.id);
                }
            }
        });
    }
}

impl SpawnPool {
    pub fn new(
        room_name: RoomName,
        base_spawn_request: SpawnRequest,
        options: SpawnPoolOptions
    ) -> Self {
        Self {
            id: WId::new(),
            room_name,
            base_spawn_request,
            travel_spec: options.travel_spec,
            target_number_of_creeps: options.target_number_of_creeps,
            current_creeps_and_processes: Vec::new(),
        }
    }

    /// Keeps given number of creeps with given parameters spawned and does the prespawning and
    /// optionally travelling to given by `travel_spec` position.
    /// When the creep is spawned, it creates a future using `creep_future_constructor` and runs it
    /// until the creep dies. When it dies, it kills its process.
    /// It is guaranteed that the creep exists in each tick the future is executed.
    /// It is guaranteed that only one constructed future exists at a time per creep.
    /// This function must be called at least once each tick for the scheduling and spawning to work
    /// correctly.
    /// The number of creeps, `target_number_of_creeps`, is a target, not the exact number, and
    /// may be modified.
    /// The function will always attempt to spawn more creeps to reach the target and will never
    /// prespawn new creeps when there are too many creeps already. However, it will not release
    /// already spawned creeps or prespawned creeps or kill their processes.
    /// The `base_spawn_request` can be modified, modifying the body of creeps that are not already
    /// spawned or scheduled. Existing and already scheduled creeps are not killed or cancelled.
    // TODO Cancel scheduled creeps on base_spawn_request change.
    pub fn with_spawned_creeps<G, F>(&mut self, mut creep_future_constructor: G) where
        G: FnMut(CreepRef) -> F,
        F: Future<Output = ()> + 'static,
    {
        let current_number_of_processes = self
            .current_creeps_and_processes
            .iter()
            .filter(|element| element.respawn)
            .count();
        match current_number_of_processes.cmp(&(self.target_number_of_creeps as usize)) {
            std::cmp::Ordering::Equal => (),
            std::cmp::Ordering::Greater => {
                let mut extra_processes = current_number_of_processes - self.target_number_of_creeps as usize;
                trace!(
                    "Disabling respawn of up to {} {} creeps to the spawn pool.",
                    extra_processes, self.base_spawn_request.role
                );
                // First trying to remove an element without active process and without a creep.
                // If that fails, trying to mark a process without a prespawned creep to not
                // respawn. If that fails too, marking a process with minimum time to next creep
                // to not respawn.
                for element in self.current_creeps_and_processes.iter_mut() {
                    if extra_processes == 0 {
                        break;
                    }

                    if !element.respawn {
                        continue;
                    }

                    // Doing nothing in the cases when the spawning has already begun or the creep is
                    // already prespawned. In these cases, just waiting until there is something
                    // to remove.
                    let remove_process = element.prespawned_creep.as_ref().map_or(true, |pc| match pc {
                        MaybeSpawned::Spawned(_) => false,
                        MaybeSpawned::Spawning(_, promise) => promise.borrow().cancelled || promise.borrow().spawn_end_tick.is_none(),
                    });
                    if remove_process {
                        element.respawn = false;
                        extra_processes -= 1;
                    }
                }
            }
            std::cmp::Ordering::Less => {
                let mut missing_processes = self.target_number_of_creeps as usize - current_number_of_processes;
                trace!(
                    "Adding or restarting respawning of {} {} creeps to the spawn pool.",
                    missing_processes, self.base_spawn_request.role
                );
                // First, trying to bring back old processes.
                // If their respawn is set to false then this function must have already run, so
                // there should be no processes here with a scheduled creep.
                let mut not_respawning_creeps_and_process_refs = self
                    .current_creeps_and_processes
                    .iter_mut()
                    .filter(|element| !element.respawn)
                    .collect::<Vec<_>>();

                // Preferring ones with a creep already prespawned (with maximum TTL), then the ones
                // with a creep already spawned (with maximum TTL), then whatever. Having the most TTL
                // makes fitting the next prespawned creep into the schedule easier.
                not_respawning_creeps_and_process_refs.sort_by(|e1, e2| {
                    // Note the reversed order - we want the highest TTL first.
                    e2.prespawned_creep_ticks_to_live()
                        .cmp(&e1.prespawned_creep_ticks_to_live())
                        .then_with(|| {
                            e2.current_creep_ticks_to_live().cmp(&e1.current_creep_ticks_to_live())
                        })
                });

                trace!(
                    "Restarting respawning of {} {} creeps to the spawn pool.",
                    max(not_respawning_creeps_and_process_refs.len(), missing_processes), self.base_spawn_request.role
                );
                for element in not_respawning_creeps_and_process_refs.drain(..).take(missing_processes) {
                    element.respawn = true;
                    missing_processes -= 1;
                }

                // If that does not help, adding new processes.
                if missing_processes != 0 {
                    trace!(
                        "Adding {} new {} creeps to the spawn pool.",
                        missing_processes, self.base_spawn_request.role
                    );
                    self.current_creeps_and_processes
                        .resize_with(
                            self.current_creeps_and_processes.len() + missing_processes,
                            SpawnPoolElement::default
                        );
                }
            }
        }

        let mut respawning_creeps_and_processes = 0;
        self.current_creeps_and_processes.retain_mut(|element| {
            element.with_spawned_creep(&mut creep_future_constructor, self.room_name, &self.base_spawn_request, self.travel_spec.as_ref());
            if element.respawn {
                respawning_creeps_and_processes += 1;
            }
            // Only retaining the elements that have something to do.
            element.respawn || element.prespawned_creep.is_some() || element.current_creep_and_process.is_some()
        });
        a!(respawning_creeps_and_processes >= self.target_number_of_creeps as usize);
        
        with_room_state(self.room_name, |room_state| {
            if let Some(room_eco_stats) = room_state.eco_stats.as_mut() {
                room_eco_stats
                    .creep_stats_by_role
                    .entry(self.base_spawn_request.role)
                    .or_default()
                    .insert(self.id, self.stats());
            }
        });
    }
    
    pub fn stats(&self) -> RoomCreepStats {
        let mut stats = RoomCreepStats::default();
        
        for element in self.current_creeps_and_processes.iter() {
            if let Some((current_creep, _)) = element.current_creep_and_process.as_ref() {
                stats.number_of_active_creeps += 1;
                stats.max_active_creep_ttl = max(stats.max_active_creep_ttl, current_creep.borrow_mut().ticks_to_live());
            }
            if let Some(MaybeSpawned::Spawned(creep_ref)) = element.prespawned_creep.as_ref() {
                stats.number_of_creeps += 1;
                stats.max_creep_ttl = max(stats.max_creep_ttl, creep_ref.borrow_mut().ticks_to_live());
            }
        }
        stats.number_of_creeps += stats.number_of_active_creeps;
        stats.max_creep_ttl = max(stats.max_active_creep_ttl, stats.max_creep_ttl);
        
        stats
    }
}

impl Default for SpawnPoolElement {
    fn default() -> Self {
        Self {
            current_creep_and_process: None,
            prespawned_creep: None,
            respawn: true,
        }
    }
}

impl SpawnPoolElement {
    /// Keeps a creep with given parameters spawned and does the prespawning and optionally
    /// travelling to given by `travel_spec` position.
    /// When the creep is spawned, it creates a future using `creep_future_constructor` and runs it
    /// until the creep dies. When it dies, it kills the process.
    /// It is guaranteed that the creep exists in each tick the future is executed.
    /// It is guaranteed that only one constructed future exists at a time per creep (and thus
    /// `SpawnPoolElement`).
    /// This function stops prespawning creeps once the `respawn` property is set to false.
    /// This function must be called at least once each tick for the scheduling and spawning to work
    /// correctly.
    pub fn with_spawned_creep<G, F>(
        &mut self,
        mut creep_future_constructor: G,
        room_name: RoomName,
        base_spawn_request: &SpawnRequest,
        travel_spec: Option<&TravelSpec>
    ) where
        G: FnMut(CreepRef) -> F,
        F: Future<Output = ()> + 'static,
    {
        // If the current creep is dead, killing its process and discarding its information.
        if let Some((current_creep, _)) = self.current_creep_and_process.as_ref() {
            if current_creep.borrow().dead {
                trace!(
                    "A current {:?} creep from the spawn pool died.",
                    base_spawn_request.role
                );
                let (_, current_process) = u!(self.current_creep_and_process.take());
                kill(current_process, ());
            }
        }

        // If there is a prespawned creep, we check if it spawned already and handle its movement to
        // the target location (if supplied). At the beginning we also use this to spawn the first
        // creep.
        // Additionally, if `respawn` is false, but a creep is scheduled and not spawning yet,
        // cancelling the spawn request.
        if let Some(prespawned_creep) = self.prespawned_creep.as_ref() {
            match prespawned_creep {
                MaybeSpawned::Spawned(creep_ref) => {
                    // If the creep is spawned, it should be travelling to its target (if needed)
                    // and there is nothing to do with it aside from checking whether it is still
                    // alive.
                    if creep_ref.borrow().dead {
                        self.prespawned_creep = None;
                        debug!(
                            "A prespawned {} creep from the spawn pool died.",
                            base_spawn_request.role
                        );
                    }
                }
                MaybeSpawned::Spawning(spawn_request, spawn_promise) => {
                    let mut borrowed_spawn_promise  = spawn_promise.borrow_mut();
                    if borrowed_spawn_promise.cancelled {
                        // The spawn request was cancelled (externally).
                        drop(borrowed_spawn_promise);
                        self.prespawned_creep = None;
                        debug!(
                            "Spawn request of {} creep from the spawn pool was cancelled.",
                            base_spawn_request.role
                        );
                    } else if let Some(creep_ref) = borrowed_spawn_promise.creep.take() {
                        // The prespawned creep is expected to be alive now. Making it travel to the
                        // target point if there is one. Note that this happens even if the creep is
                        // being used as the new current creep right away.
                        // TODO This relies on spawn_end_tick being updated properly. Check if this is the case.
                        drop(borrowed_spawn_promise);
                        if let Some(travel_spec) = travel_spec {
                            travel(&creep_ref, travel_spec.clone());
                        }
                        self.prespawned_creep = Some(MaybeSpawned::Spawned(ReservedCreep::new(creep_ref)));
                        trace!(
                            "A prespawned {} creep from the spawn pool has spawned.",
                            base_spawn_request.role
                        );
                    } else {
                        let mut cancel = false;
                        if !self.respawn && borrowed_spawn_promise.spawn_end_tick.is_none() {
                            // Cancelling the scheduled prespawned creep. Removing it will happen
                            // in subsequent tick.
                            trace!("Cancelling the prespawn request for {}.", base_spawn_request.role);
                            cancel = true;

                        } else if self.respawn && self.current_creep_and_process.is_none() {
                            // The case when the current creep has prematurely died or before the first
                            // creep has spawned. To differentiate it from the creep being scheduled
                            // to spawn now, but not having a spawn timer, the max preferred tick is
                            // checked.
                            if borrowed_spawn_promise.spawn_end_tick.is_none() && spawn_request.tick.0 > game_tick() {
                                trace!(
                                    "Cancelling the prespawn request for {} because it is not scheduled to spawn now.",
                                    base_spawn_request.role
                                );
                                cancel = true;
                            }
                        }
                        
                        if cancel {
                            // Removing the scheduled creep immediately.
                            // In the case of too late schedule, it enables rescheduling it this
                            // tick.
                            drop(borrowed_spawn_promise);
                            if let Some(MaybeSpawned::Spawning(_, spawn_promise)) = self.prespawned_creep.take() {
                                cancel_scheduled_creep(room_name, spawn_promise);
                            }
                        }
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
                    // This is the case after a reset or after `respawn` is false or after it was
                    // impossible to spawn a creep for a long time.
                    if self.respawn {
                        // Trying to get an existing creep before spawning a new one.
                        // If that fails, a new one will be scheduled.
                        find_idle_creep(
                            room_name,
                            base_spawn_request.role,
                            &base_spawn_request.body,
                            travel_spec.as_ref().map(|travel_spec| travel_spec.target.xy()),
                        ).inspect(|creep| {
                            debug!("Found idle {} creep.", base_spawn_request.role);
                        })
                    } else {
                        // At this point, `respawn` is false, there is no current process and there
                        // is no prespawned creep. This element of the spawn pool will be removed.
                        return;
                    }
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

            if let Some(reserved_creep) = existing_creep {
                // Replacing the current creep with the one found above.
                // Running the user code on the current creep by constructing the future and
                // scheduling it.
                let future = creep_future_constructor(reserved_creep.as_ref());
                let wrapper_priority = current_process_wrapped_meta().borrow().priority;
                let current_process = schedule(
                    &format!("spawn_pool_{}_creep_process", base_spawn_request.role),
                    wrapper_priority.saturating_sub(1),
                    future,
                );
                self.current_creep_and_process = Some((reserved_creep, current_process));
            }
        }

        if self.respawn && self.prespawned_creep.is_none() {
            // Scheduling the spawning of a creep if `respawn` is true, both as prespawning when
            // there is already a creep alive and when there is none. The difference is that in
            // the latter case, we want the creep spawned as fast as possible.
            let mut spawn_request = base_spawn_request.clone();
            if let Some((current_creep, _)) = self.current_creep_and_process.as_ref() {
                // The prespawning case.
                let creep_death_tick = game_tick() + current_creep.borrow_mut().ticks_to_live();
                let preferred_spawn_pos = spawn_request.preferred_spawns[0].pos;
                // TODO Cache this, maybe just by moving out of the scope of the loop.
                let creep_travel_ticks = travel_spec
                    .as_ref()
                    .map(|travel_spec| {
                        predicted_travel_ticks(
                            preferred_spawn_pos,
                            travel_spec.target,
                            1,
                            travel_spec.range,
                            &spawn_request.body,
                            false // TODO
                        )
                    })
                    .unwrap_or(0);

                let min_preferred_tick = creep_death_tick - creep_travel_ticks;
                // TODO Implement the margin properly even if creep_travel_ticks exceeeds base tick
                //      range.
                let max_preferred_tick = max(
                    game_tick() + 100,
                    min_preferred_tick + base_spawn_request.tick.1 - base_spawn_request.tick.0
                );
                spawn_request.tick = (min_preferred_tick, max_preferred_tick);
            } else {
                // The case with spawning as fast as possible.
                let min_preferred_tick = game_tick();
                // TODO Implement the margin properly.
                let max_preferred_tick = min_preferred_tick + 200;
                spawn_request.tick = (min_preferred_tick, max_preferred_tick);
            }

            // Scheduling the creep.
            let spawn_promise = u!(schedule_creep(room_name, spawn_request.clone()));
            // TODO Some other process may reserve this creep using find_idle_creep immediately after spawn, need to prevent that.
            self.prespawned_creep = Some(MaybeSpawned::Spawning(spawn_request, spawn_promise));
            debug!(
                "Scheduled a prespawn of {} creep from the spawn pool.",
                base_spawn_request.role
            );
        }
    }

    /// Returns the TTL of the prespawned creep or zero if it has not spawned yet.
    fn prespawned_creep_ticks_to_live(&self) -> u32 {
        self.prespawned_creep.as_ref().map_or(0, |pc| match pc {
            MaybeSpawned::Spawned(creep) => {
                creep.as_ref().borrow_mut().ticks_to_live()
            },
            MaybeSpawned::Spawning(_, promise) => {
                promise
                    .borrow()
                    .creep
                    .as_ref()
                    .map(|creep| creep.borrow_mut().ticks_to_live())
                    .unwrap_or(0)
            },
        })
    }

    /// Returns the TTL of the prespawned creep or zero if it has not spawned yet.
    fn current_creep_ticks_to_live(&self) -> u32 {
        self.current_creep_and_process
            .as_ref()
            .map_or(0, |(creep, _)| creep.as_ref().borrow_mut().ticks_to_live())
    }
}