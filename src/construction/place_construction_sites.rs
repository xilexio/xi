use std::cmp::max;
use crate::utils::game_tick::first_tick;
use crate::kernel::sleep::{sleep, sleep_until};
use crate::room_states::room_states::for_each_owned_room;
use crate::u;
use crate::utils::find::get_structure;
use crate::utils::multi_map_utils::MultiMapUtils;
use crate::utils::result_utils::ResultUtils;
use js_sys::JsString;
use log::{debug, error, trace, warn};
use rustc_hash::{FxHashMap, FxHashSet};
use screeps::game::{construction_sites, rooms};
use screeps::StructureType::{
    Container, Extension, Extractor, Factory, Lab, Link, Nuker, Observer, PowerSpawn, Rampart, Road, Spawn, Storage,
    Terminal, Tower, Wall,
};
use screeps::{game, HasPosition, MaybeHasId, RoomName, RoomXY, StructureType};
use crate::room_states::room_state::{ConstructionSiteData, StructuresMap};

const DEBUG: bool = true;

const MAX_CONSTRUCTION_SITES_PER_ROOM: u32 = 4;

const PRIORITY_OF_STRUCTURES: [StructureType; 16] = [
    Spawn,
    Extension,
    Storage,
    Terminal,
    Tower,
    Link,
    Container,
    Road,
    Lab,
    Extractor,
    Factory,
    PowerSpawn,
    Observer,
    Nuker,
    Rampart,
    Wall,
];

// Places construction sites in a room and removes incorrect ones. Removes incorrect buildings.
// Sets the construction site queue in the room state.
// TODO As it is not using the global construction site limit, it should just be ran independently
//      for each room and moved to room maintenance.
pub async fn place_construction_sites() {
    sleep_until(first_tick() + 10).await;

    loop {
        for_each_owned_room(|room_name, room_state| {
            let mut construction_sites_by_room = FxHashMap::default();

            // The construction sites may be removed by stomping on them so there is a need to
            // fetch them anew.
            for construction_site in construction_sites().values() {
                let room_name = u!(construction_site.room()).name();
                let id = u!(construction_site.try_id());
                let xy = construction_site.pos().xy();
                let structure_type = construction_site.structure_type();
                construction_sites_by_room.push_or_insert(room_name, ConstructionSiteData {
                    id,
                    structure_type,
                    xy
                });
            }

            if room_state.current_rcl_structures.is_empty() {
                trace!(
                    "No structures are planned in room {} for RCL {}.",
                    room_name, room_state.rcl
                );
            } else {
                trace!(
                    "Computing what construction sites to place in room {} at RCL {}.",
                    room_name, room_state.rcl
                );
                // Computing which structures are missing and which are not in the plan.
                let StructuresDiff {
                    extra_structures,
                    missing_structures_by_priority
                } = room_structures_diff_from_current_rcl_structures(
                    &room_state.current_rcl_structures,
                    &room_state.structures
                );

                // Cannot remove a structure that cannot be in the same place as the new one
                // and create a construction site in the same tick in the same place.
                // Cannot remove and create another construction site in the same
                // tick in the same place.
                // Cannot place two construction sites in the same place.
                // Gathering coordinates of these tiles.
                let mut xys_not_for_new_cs = extra_structures
                    .values()
                    .flatten()
                    .copied()
                    .collect::<FxHashSet<_>>();

                // Removing extra structures.
                let mut number_of_spawns = room_state
                    .structures
                    .get(&Spawn)
                    .map(|xys| xys.len())
                    .unwrap_or(0);
                for (structure_type, xys) in extra_structures {
                    for xy in xys {
                        // There is an extra structure in the room. It might happen upon claiming
                        // a room with structures present or when the room was downgraded.
                        if structure_type == Spawn && number_of_spawns == 1 {
                            warn!(
                                "The only {:?} in {} at {} is in an incorrect place. Not removing it.",
                                structure_type, room_name, xy,
                            );
                        } else {
                            // Destroying the structure.
                            if let Some(structure_obj) = get_structure(room_name, xy, structure_type) {
                                // TODO Do not destroy the structure if it is owned and supposed
                                //      to be built at RCL8 in that location unless it being
                                //      inactive breaks something (e.g., remote links being
                                //      active while the fast filler link is not).
                                // TODO This should be some API constant, not just zero.
                                if structure_obj.as_structure().destroy() != 0 {
                                    warn!(
                                        "Failed to remove a structure {:?} in {} at {}",
                                        structure_type, room_name, xy
                                    );
                                }

                                if structure_type == Spawn {
                                    number_of_spawns -= 1;
                                }
                            } else {
                                error!("Failed to find the structure {:?} in {} at {} that was about to be removed",
                                    structure_type, room_name, xy);
                            }
                        }
                    }
                }

                // Computing which construction sites are missing and which are not in the plan
                // or not top priority.
                let room_construction_sites = construction_sites_by_room
                    .remove(&room_name)
                    .unwrap_or_default();
                let room_construction_sites_count = room_construction_sites.len();

                let ConstructionSitesDiff {
                    extra_construction_sites,
                    correct_construction_sites,
                    missing_construction_sites
                } = construction_sites_diff_from_top_priority_missing_structures(
                    missing_structures_by_priority,
                    room_construction_sites
                );

                xys_not_for_new_cs.extend(
                    extra_construction_sites
                        .iter()
                        .map(|cs| cs.xy)
                );

                let construction_sites_left_to_limit = max(
                    MAX_CONSTRUCTION_SITES_PER_ROOM as i32 + extra_construction_sites.len() as i32 - room_construction_sites_count as i32,
                    0
                ) as usize;

                // Registering the correct construction sites in the room state.
                room_state.construction_site_queue = correct_construction_sites;

                // Removing invalid construction sites.
                // TODO Do not remove construction site with decent progress on them.
                for cs in extra_construction_sites {
                    let construction_site = u!(game::get_object_by_id_typed(&cs.id));
                    construction_site.remove().warn_if_err(&format!(
                        "Failed to remove a construction site of {:?} in {} at {}",
                        cs.structure_type, room_name, cs.xy
                    ));
                }

                // Placing construction sites with the top priority.
                // Taking only the `construction_sites_left_to_limit` because the next iteration
                // of this function every extra structure and construction site will be removed
                // (maybe except the sole incorrect spawn), so no point in starting work on
                // other construction sites only to remove
                let placed_construction_sites = missing_construction_sites
                    .iter()
                    .take(construction_sites_left_to_limit);
                for &(structure_type, xy) in placed_construction_sites {
                    if xys_not_for_new_cs.contains(&xy) {
                        debug!(
                            "Cannot place construction site for {:?} in {} at {} since something else is there.",
                            structure_type, room_name, xy
                        );
                    } else {
                        xys_not_for_new_cs.insert(xy);
                        debug!(
                            "Placing a new construction site for {:?} at {} in {}.",
                            structure_type, xy, room_name
                        );

                        let room = u!(rooms().get(room_name));

                        let js_name = structure_js_name(structure_type, room_name, xy);
                        let creation_result = room
                            .create_construction_site(
                                xy.x.u8(),
                                xy.y.u8(),
                                structure_type,
                                js_name.as_ref(),
                            );
                        creation_result.warn_if_err(&format!(
                            "Failed to create the construction site of {:?} in {} at {}",
                            structure_type, room_name, xy
                        ));
                    }
                }
            }
        });

        sleep(20).await;
    }
}

struct StructuresDiff {
    extra_structures: FxHashMap<StructureType, Vec<RoomXY>>,
    missing_structures_by_priority: Vec<(StructureType, RoomXY)>,
}

fn room_structures_diff_from_current_rcl_structures(
    planned_structures: &StructuresMap,
    existing_structures: &StructuresMap
) -> StructuresDiff {
    let mut extra_structures = FxHashMap::<_, Vec<_>>::default();
    let mut missing_structures_by_priority = Vec::new();

    for structure_type in PRIORITY_OF_STRUCTURES {
        let existing_structure_xys = existing_structures
            .get(&structure_type)
            .cloned()
            .unwrap_or_default();
        let planned_structure_xys = planned_structures
            .get(&structure_type)
            .cloned()
            .unwrap_or_default();

        // Computing extra structures that should be removed.
        for &xy in existing_structure_xys.iter() {
            if !planned_structure_xys.contains(&xy) {
                extra_structures.entry(structure_type).or_default().push(xy);
            }
        }

        // Computing missing structures that should be placed.
        for &xy in planned_structure_xys.iter() {
            if !existing_structure_xys.contains(&xy) {
                missing_structures_by_priority.push((structure_type, xy));
            }
        }
    }

    StructuresDiff {
        extra_structures,
        missing_structures_by_priority,
    }
}

struct ConstructionSitesDiff {
    correct_construction_sites: Vec<ConstructionSiteData>,
    extra_construction_sites: Vec<ConstructionSiteData>,
    missing_construction_sites: Vec<(StructureType, RoomXY)>,
}

fn construction_sites_diff_from_top_priority_missing_structures(
    planned_construction_sites: Vec<(StructureType, RoomXY)>,
    existing_construction_sites: Vec<ConstructionSiteData>
) -> ConstructionSitesDiff {
    let mut existing_cs_map = existing_construction_sites
        .into_iter()
        .map(|cs| (cs.xy, cs))
        .collect::<FxHashMap<_, _>>();

    let mut correct_construction_sites = Vec::new();
    let mut extra_construction_sites = Vec::new();
    let mut missing_construction_sites = Vec::new();

    for (structure_type, xy) in planned_construction_sites {
        let maybe_existing_cs = existing_cs_map.remove(&xy);
        if let Some(existing_construction_site) = maybe_existing_cs {
            if existing_construction_site.structure_type == structure_type {
                correct_construction_sites.push(existing_construction_site);
            } else {
                extra_construction_sites.push(existing_construction_site);
                missing_construction_sites.push((structure_type, xy));
            }
        } else {
            missing_construction_sites.push((structure_type, xy));
        }
    }

    extra_construction_sites.extend(
        existing_cs_map
            .drain()
            .map(|(_, cs)| cs)
    );

    ConstructionSitesDiff {
        correct_construction_sites,
        extra_construction_sites,
        missing_construction_sites
    }
}

fn structure_js_name(structure_type: StructureType, room_name: RoomName, xy: RoomXY) -> Option<JsString> {
    if structure_type == Spawn {
        let name = room_name.to_string() + &*xy.to_string();
        let js_name: JsString = name.into();
        Some(js_name)
    } else {
        None
    }
}
