use log::debug;
use crate::algorithms::matrix_common::MatrixCommon;
use crate::kernel::sleep::sleep;
use crate::profiler::measure_time;
use crate::room_state::room_states::for_each_owned_room;
use crate::utils::find::get_structure;
use room_visual_ext::RoomVisualExt;
use screeps::StructureType::{Rampart, Road};
use screeps::{game, StructureType};

const CURRENT_RCL_PLAN_OPACITY: f32 = 0.4;
const RCL8_PLAN_OPACITY: f32 = 0.12;

pub async fn show_visualizations() {
    loop {
        // TODO This should be more dynamic.
        if game::cpu::tick_limit() - game::cpu::get_used() > 100.0 {
            measure_time("show_visualizations", || {
                for_each_owned_room(|room_name, room_state| {
                    if let Some(plan) = room_state.plan.as_ref() {
                        if let Some(current_rcl_structures) = room_state.current_rcl_structures.as_ref() {
                            let mut vis = RoomVisualExt::new(room_name);

                            for (xy, tile) in plan.tiles.iter() {
                                if tile.structures().road() && get_structure(room_name, xy, Road).is_none() {
                                    let opacity = if current_rcl_structures
                                        .get(&Road)
                                        .map(|xys| xys.contains(&xy))
                                        .unwrap_or(false)
                                    {
                                        CURRENT_RCL_PLAN_OPACITY
                                    } else {
                                        RCL8_PLAN_OPACITY
                                    };
                                    vis.structure_roomxy(xy, Road, opacity);
                                }
                            }

                            for (xy, tile) in plan.tiles.iter() {
                                if let Ok(structure_type) = StructureType::try_from(tile.structures().main()) {
                                    if get_structure(room_name, xy, structure_type).is_none() {
                                        let opacity = if current_rcl_structures
                                            .get(&structure_type)
                                            .map(|xys| xys.contains(&xy))
                                            .unwrap_or(false)
                                        {
                                            CURRENT_RCL_PLAN_OPACITY
                                        } else {
                                            RCL8_PLAN_OPACITY
                                        };
                                        vis.structure_roomxy(xy, structure_type, opacity);
                                    }
                                }
                            }

                            for (xy, tile) in plan.tiles.iter() {
                                if tile.structures().rampart() && get_structure(room_name, xy, Rampart).is_none() {
                                    let opacity = if current_rcl_structures
                                        .get(&Road)
                                        .map(|xys| xys.contains(&xy))
                                        .unwrap_or(false)
                                    {
                                        CURRENT_RCL_PLAN_OPACITY
                                    } else {
                                        RCL8_PLAN_OPACITY
                                    };
                                    vis.structure_roomxy(xy, Rampart, opacity);
                                }
                            }
                        }
                    }
                });
            });
        }

        sleep(1).await;
    }
}
