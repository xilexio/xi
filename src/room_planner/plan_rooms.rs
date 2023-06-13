use std::thread::spawn;
use log::{error, trace};
use screeps::StructureType::{Extension, Spawn};
use crate::game_time::{first_tick, game_tick};
use crate::kernel::should_finish;
use crate::kernel::sleep::{sleep, sleep_until};
use crate::{a, log_err, u};
use crate::algorithms::matrix_common::MatrixCommon;
use crate::room_planner::RoomPlanner;
use crate::room_state::room_states::{for_each_owned_room};
use crate::room_state::{RoomState, StructuresMap};
use crate::utils::map_utils::MultiMapUtils;

pub async fn plan_rooms() {
    // TODO Set to run only a total of CONST% of time unless it is the first room. Kernel should measure run times
    //      of processes and adjust the run time accordingly. The process should have voluntary interruption points
    //      inserted in various places to facilitate this. Similarly, kernel should interrupt the process when
    //      there is not enough CPU in the bucket.
    // TODO Should run as long as it needs during the planning of the first room.

    sleep_until(first_tick() + 5).await;

    // Iterating over all scanned and owned rooms.
    for_each_owned_room(|room_name, room_state| {
        // Creating the room plan if there isn't one.
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

                            plan_current_rcl_structures(room_state);
                        }
                        break;
                    } else if should_finish() {
                        break;
                    }
                }
            }
        } else if !room_state.current_rcl_structures_built && room_state.current_rcl_structures.is_none() {
            plan_current_rcl_structures(room_state);
        }
    });

    // Running only once per few ticks.
    sleep(10).await;
}

/// Creates a map of structures to be built for given RCL.
pub fn plan_current_rcl_structures(room_state: &mut RoomState) {
    a!(room_state.rcl > 0 && room_state.rcl <= 8);

    let plan = u!(room_state.plan.as_ref());

    let rcl8_structures_map = plan.tiles.to_structures_map();

    let structures_map = if room_state.rcl == 8 {
        rcl8_structures_map
    } else {
        let mut structures_map = StructuresMap::default();

        // TODO use the min_rcl after all
        //  min_rcl is only about main structures
        //  ramparts should be from RAMPARTS_RCL - configurable
        //  roads should be to all ramparts/other stuff on the shortest road only except on ramparts - there always
        //  however, do not make any roads before rcl 3

        if room_state.rcl >= 1 {
            // structures_map.push_or_insert(Spawn, spawns[0]);
        }

        if room_state.rcl >= 2 {
            // structures_map.push_or_insert(Spawn, spawns[0]);
        }

        if room_state.rcl >= 3 {
            // TODO
        }

        if room_state.rcl >= 4 {
            // TODO
        }

        if room_state.rcl >= 5 {
            // TODO
        }

        if room_state.rcl >= 6 {
            // TODO
        }

        if room_state.rcl >= 7 {
            // TODO
        }

        structures_map
    };

    room_state.current_rcl_structures = Some(structures_map);
}