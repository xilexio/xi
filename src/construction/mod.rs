use crate::game_tick::first_tick;
use crate::kernel::sleep::{sleep, sleep_until};
use crate::room_state::room_states::for_each_owned_room;
use crate::u;
use crate::utils::find::get_structure;
use crate::utils::multi_map_utils::MultiMapUtils;
use crate::utils::result_utils::ResultUtils;
use derive_more::Constructor;
use js_sys::JsString;
use log::{debug, error, trace, warn};
use rustc_hash::{FxHashMap, FxHashSet};
use screeps::game::{construction_sites, rooms};
use screeps::StructureType::{
    Container, Extension, Extractor, Factory, Lab, Link, Nuker, Observer, PowerSpawn, Rampart, Road, Spawn, Storage,
    Terminal, Tower, Wall,
};
use screeps::{game, ConstructionSite, HasPosition, MaybeHasId, ObjectId, Position, RoomName, RoomXY, StructureType, MAX_CONSTRUCTION_SITES};

const MAX_CONSTRUCTION_SITES_PER_ROOM: u32 = 10;

const PRIORITY_OF_STRUCTURES: [StructureType; 16] = [
    Spawn, Extension, Storage, Terminal, Tower, Link, Container, Road, Lab, Extractor, Factory, PowerSpawn, Observer,
    Nuker, Rampart, Wall,
];

#[derive(Debug, Copy, Clone, Constructor)]
struct ConstructionSiteData {
    id: ObjectId<ConstructionSite>,
    xy: RoomXY,
    structure_type: StructureType,
}

pub async fn construct_structures() {
    sleep_until(first_tick() + 10).await;

    loop {
        let mut total_construction_sites_count = 0;
        let mut construction_sites_by_room = FxHashMap::default();

        let initial_construction_sites = construction_sites();
        for construction_site in initial_construction_sites.values() {
            let room_name = u!(construction_site.room()).name();
            let id = u!(construction_site.try_id());
            let xy = Position::from(construction_site.pos()).xy();
            let structure_type = construction_site.structure_type();
            construction_sites_by_room.push_or_insert(room_name, ConstructionSiteData::new(id, xy, structure_type));
            total_construction_sites_count += 1;
        }

        for_each_owned_room(|room_name, room_state| {
            if !room_state.current_rcl_structures_built {
                trace!("Computing what construction sites to place in room {}.", room_name);

                let room_construction_sites = construction_sites_by_room.entry(room_name).or_insert_with(Vec::new);
                let existing_construction_sites_xys = room_construction_sites
                    .iter()
                    .map(|construction_site_data| construction_site_data.xy)
                    .collect::<FxHashSet<_>>();
                let mut room_construction_sites_count = room_construction_sites.len();

                if let Some(current_rcl_structures) = room_state.current_rcl_structures.as_ref() {
                    // Removing invalid construction sites.
                    for construction_site_data in room_construction_sites.iter() {
                        if !current_rcl_structures
                            .get(&construction_site_data.structure_type)
                            .map(|xys| xys.contains(&construction_site_data.xy))
                            .unwrap_or(false)
                        {
                            // TODO Remove them only if they are not present in the RCL8 plan, otherwise just ignore
                            //      until required RCL.
                            let construction_site = u!(game::get_object_by_id_typed(&construction_site_data.id));
                            construction_site.remove().warn_if_err(&format!(
                                "Failed to remove a construction site of {:?} in {} at {}",
                                construction_site_data.structure_type, room_name, construction_site_data.xy
                            ));
                        }
                    }

                    for structure_type in PRIORITY_OF_STRUCTURES {
                        let structure_xys = room_state
                            .structures
                            .get(&structure_type)
                            .cloned()
                            .unwrap_or_default();
                        let structure_current_rcl_xys = current_rcl_structures
                            .get(&structure_type)
                            .cloned()
                            .unwrap_or_default();

                        let mut has_incorrect_last_spawn = false;

                        // Removing extra structures.
                        for &xy in structure_xys.iter() {
                            if !structure_current_rcl_xys.contains(&xy) {
                                // There is an extra structure in the room. It might happen upon claiming
                                // a room with structures present or when the room was downgraded.
                                if structure_type == Spawn {
                                    has_incorrect_last_spawn = true;
                                }
                                
                                if structure_type == Spawn && structure_xys.len() == 1 {
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
                                        if structure_obj.as_structure().destroy() != 0 {
                                            warn!(
                                                "Failed to remove a structure {:?} in {} at {}",
                                                structure_type, room_name, xy
                                            );
                                        }
                                    } else {
                                        warn!("Failed to find the structure {:?} in {} at {} that was about to be removed", structure_type, room_name, xy);
                                    }
                                }
                            }
                        }

                        // Placing construction sites for missing structures.
                        if room_construction_sites_count < MAX_CONSTRUCTION_SITES_PER_ROOM as usize
                            && total_construction_sites_count < MAX_CONSTRUCTION_SITES as usize
                        {
                            // Check what structures are missing in the order of priority and place their construction sites,
                            // obeying MAX_CONSTRUCTION_SITES_PER_ROOM and global MAX_CONSTRUCTION_SITES.
                            if let Some(room) = rooms().get(room_name) {
                                for xy in structure_current_rcl_xys.iter() {
                                    if !structure_xys.contains(xy) && !existing_construction_sites_xys.contains(xy) {
                                        if structure_type == Spawn && has_incorrect_last_spawn && structure_current_rcl_xys.len() == 1 {
                                            warn!(
                                                "Not placing construction site for {:?} in {} at {} since there exists only one, though in an incorrect place.",
                                                structure_type, room_name, xy,
                                            );
                                            continue;
                                        }

                                        debug!(
                                            "Placing a new construction site for {:?} at {} in {}.",
                                            structure_type, xy, room_name
                                        );

                                        // There is a structure yet to be built. Placing the construction site.
                                        let js_name = structure_js_name(structure_type, room_name, *xy);
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
                                        
                                        if creation_result.is_ok() {
                                            room_construction_sites_count += 1;
                                            total_construction_sites_count += 1;

                                            // Checking if further construction sites may be created.
                                            if room_construction_sites_count >= MAX_CONSTRUCTION_SITES_PER_ROOM as usize
                                            {
                                                return;
                                            }

                                            if total_construction_sites_count >= MAX_CONSTRUCTION_SITES as usize {
                                                return;
                                            }
                                        } else {
                                            // Interrupt the work on this room. Try again later.
                                            return;
                                        }
                                    }
                                }
                            }
                        }
                    }
                } else {
                    error!(
                        "Expected Some in current_rcl_structures in room {} when they are not built yet.",
                        room_name
                    );
                }
            }
        });

        sleep(20).await;
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
