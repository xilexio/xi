use log::{error, trace};
use crate::game_time::{first_tick, game_tick};
use crate::kernel::should_finish;
use crate::kernel::sleep::{sleep, sleep_until};
use crate::log_err;
use crate::room_planner::RoomPlanner;
use crate::room_state::room_states::{for_each_owned_room};

pub async fn plan_rooms() {
    // TODO Set to run only a total of CONST% of time unless it is the first room. Kernel should measure run times
    //      of processes and adjust the run time accordingly. The process should have voluntary interruption points
    //      inserted in various places to facilitate this. Similarly, kernel should interrupt the process when
    //      there is not enough CPU in the bucket.
    // TODO Should run as long as it needs during the planning of the first room.

    sleep_until(first_tick() + 5).await;

    // Iterating over all scanned and owned rooms.
    for_each_owned_room(|room_name, room_state| {
        if room_state.plan.is_none() {
            // Creating the planner. It should not fail unless it is a bug.
            if room_state.planner.is_none() {
                match RoomPlanner::new(room_state, true) {
                    Ok(planner) => {
                        room_state.planner = Some(planner);
                    }
                    err => {
                        log_err!(err);
                    }
                }
            }

            if let Some(planner) = room_state.planner.as_mut() {
                loop {
                    // Errors are normal when planning.
                    let result = planner.plan();
                    if let Err(err) = result {
                        trace!("Failed to create a plan for room {}: {}.", room_name, err);
                    }

                    // TODO Finishing planning should depend on used CPU more than on the number of tries.
                    if planner.plans_count >= 1 && planner.tries_count >= 20 || planner.is_finished() {
                        if planner.best_plan.is_none() {
                            error!("Failed to create a plan for room {}.", room_name);
                            // Resetting the planner.
                            room_state.planner = None;
                        } else {
                            trace!("Successfully created a plan for room {}.", room_name);
                            room_state.plan = planner.best_plan.clone();
                        }
                        break;
                    } else if should_finish() {
                        break;
                    }
                }
            }
        }
    });

    // Running only once per few ticks.
    sleep(10).await;
}