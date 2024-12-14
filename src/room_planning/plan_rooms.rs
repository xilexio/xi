use crate::algorithms::matrix_common::MatrixCommon;
use crate::utils::game_tick::first_tick;
use crate::kernel::kernel::should_finish;
use crate::kernel::sleep::{sleep, sleep_until};
use crate::room_states::room_states::for_each_owned_room;
use crate::utils::multi_map_utils::MultiMapUtils;
use crate::{a, log_err, u};
use log::{debug, error, trace};
use screeps::{game, StructureType};
use screeps::StructureType::{Container, Rampart, Road};
use crate::room_planning::room_planner::{RoomPlanner, MIN_RAMPART_RCL};
use crate::room_states::room_state::{RoomState, StructuresMap};

pub const MIN_CONTAINER_RCL: u8 = 3;

const MIN_PLAN_ROOMS_CPU: f64 = 300.0;

pub async fn plan_rooms() {
    // TODO Set to run only a total of CONST% of time unless it is the first room. Kernel should measure run times
    //      of processes and adjust the run time accordingly. The process should have voluntary interruption points
    //      inserted in various places to facilitate this. Similarly, kernel should interrupt the process when
    //      there is not enough CPU in the bucket.
    // TODO Should run as long as it needs during the planning of the first room.

    sleep_until(first_tick() + 5).await;
    
    loop {
        // Iterating over all scanned and owned rooms.
        for_each_owned_room(|room_name, room_state| {
            if game::cpu::tick_limit() - game::cpu::get_used() < MIN_PLAN_ROOMS_CPU {
                return;
            }

            // Creating the room plan if there isn't one.
            if room_state.plan.is_none() {
                // Creating the planner. It should not fail unless it is a bug.
                if room_state.planner.is_none() {
                    match RoomPlanner::new(room_state, true) {
                        Ok(planner) => {
                            room_state.planner = Some(Box::new(planner));
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
                                // Removing the planner data.
                                room_state.planner = None;

                                plan_current_rcl_structures(room_state);
                            }
                            break;
                        } else if should_finish() {
                            break;
                        }
                    }
                }
            } else if room_state.current_rcl_structures.is_none() {
                plan_current_rcl_structures(room_state);
            }
        });

        // Running only once per few ticks.
        sleep(10).await;
    }
}

/// Creates a map of structures to be built for given RCL.
pub fn plan_current_rcl_structures(room_state: &mut RoomState) {
    debug!(
        "Creating a RCL{} plan for room {}.",
        room_state.rcl, room_state.room_name
    );
    a!(room_state.rcl > 0 && room_state.rcl <= 8);

    let plan = u!(room_state.plan.as_ref());

    let rcl8_structures_map = plan.tiles.to_structures_map();

    let structures_map = if room_state.rcl == 8 {
        rcl8_structures_map
    } else {
        let mut structures_map = StructuresMap::default();

        for (xy, tile) in plan.tiles.iter() {
            if tile.structures().road() && tile.min_rcl() <= room_state.rcl {
                structures_map.push_or_insert(Road, xy);
            }
            if let Ok(structure_type) = StructureType::try_from(tile.structures().main()) {
                if tile.min_rcl() <= room_state.rcl {
                    structures_map.push_or_insert(structure_type, xy);
                }
            }
            if tile.structures().rampart() && room_state.rcl >= MIN_RAMPART_RCL {
                structures_map.push_or_insert(Rampart, xy);
            }
        }

        for source_info in plan.sources.iter() {
            if MIN_CONTAINER_RCL <= room_state.rcl && room_state.rcl < plan.tiles.get(source_info.link_xy).min_rcl() {
                structures_map.push_or_insert(Container, source_info.work_xy);
            }
        }

        if MIN_CONTAINER_RCL <= room_state.rcl && room_state.rcl < plan.tiles.get(plan.controller.link_xy).min_rcl() {
            structures_map.push_or_insert(Container, plan.controller.work_xy);
        }

        structures_map
    };

    room_state.current_rcl_structures = Some(structures_map);
    room_state.current_rcl_structures_built = false;
}
