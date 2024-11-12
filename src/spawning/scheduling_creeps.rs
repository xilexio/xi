use std::cell::RefCell;
use std::collections::Bound;
use std::rc::Rc;
use screeps::RoomName;
use crate::errors::XiError;
use crate::errors::XiError::SpawnRequestTickInThePast;
use crate::game_tick::game_tick;
use crate::spawning::spawn_schedule::{with_spawn_schedule, SpawnEvent, SpawnPromise, SpawnPromiseRef, SpawnRequest};

/// Schedule a creep to be spawned within given tick and resource constraints.
pub fn schedule_creep(room_name: RoomName, request: SpawnRequest) -> Result<SpawnPromiseRef, XiError> {
    with_spawn_schedule(room_name, move |room_spawn_schedule| {
        let spawn_promise = SpawnPromise::new();
        let id = spawn_promise.id;

        let current_tick = game_tick();
        let preferred_spawn_start_tick = request.tick.0;

        if preferred_spawn_start_tick < current_tick {
            return Err(SpawnRequestTickInThePast);
        }

        // Create a SpawnEvent and add it to the schedule.
        let spawn_promise_ref = Rc::new(RefCell::new(spawn_promise));
        let energy_cost = request.body.energy_cost();
        let spawn_duration = request.body.spawn_duration();

        let spawn_event = SpawnEvent {
            request,
            promise: spawn_promise_ref.clone(),
            energy_cost,
            spawn_duration,
            end_tick: None,
        };

        room_spawn_schedule
            .future_spawns
            .entry(preferred_spawn_start_tick)
            .or_default()
            .insert(id, spawn_event);

        Ok(spawn_promise_ref)
    })
}

/// Cancels scheduled spawn event.
/// Does not cancel creeps that are already spawning. This function is rather inefficient.
pub fn cancel_scheduled_creep(room_name: RoomName, spawn_promise: SpawnPromiseRef) {
    let mut spawn_promise = spawn_promise.borrow_mut();

    spawn_promise.cancelled = true;

    with_spawn_schedule(room_name, |room_spawn_schedule| {
        // If the spawn_id is set then the event can only be in `spawns_in_progress`.
        // Skipping this case and just letting the creep spawn to be used as an idle creep later.
        if spawn_promise.spawn_id.is_none() {
            // Removing the spawn event from `current_spawns`.
            let maybe_removed_event = room_spawn_schedule.current_spawns.extract_if(|&(_, id), _| id == spawn_promise.id).next();
            // If it wasn't there, the only remaining place is `future_spawns`.
            if maybe_removed_event.is_none() {
                let mut cursor = room_spawn_schedule.future_spawns.lower_bound_mut(Bound::Unbounded);
                while let Some((_, events)) = cursor.next() {
                    if events.contains_key(&spawn_promise.id) {
                        events.remove(&spawn_promise.id);
                        if events.is_empty() {
                            cursor.remove_prev();
                        }
                        break;
                    }
                }
            }
        }
    })
}