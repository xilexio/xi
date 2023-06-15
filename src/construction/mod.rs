use crate::game_time::first_tick;
use crate::kernel::sleep::{sleep, sleep_until};
use crate::room_state::room_states::for_each_owned_room;
use crate::u;
use crate::utils::map_utils::MultiMapUtils;
use derive_more::Constructor;
use js_sys::JsString;
use log::{debug, error, trace};
use rustc_hash::{FxHashMap, FxHashSet};
use screeps::game::{construction_sites, rooms};
use screeps::StructureType::{
    Container, Extension, Extractor, Factory, Lab, Link, Nuker, Observer, PowerSpawn, Rampart, Road, Spawn, Storage,
    Terminal, Tower, Wall,
};
use screeps::{
    ConstructionSite, MaybeHasTypedId, ObjectId, Position, RoomName, RoomXY, StructureType, MAX_CONSTRUCTION_SITES,
};

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
                let mut room_construction_sites_count = room_construction_sites.len();

                // TODO Removing invalid construction sites.
                if room_construction_sites_count < MAX_CONSTRUCTION_SITES_PER_ROOM as usize {
                    // Check what structures are missing in the order of priority and place their construction sites,
                    // obeying MAX_CONSTRUCTION_SITES_PER_ROOM and global MAX_CONSTRUCTION_SITES.
                    if let Some(current_rcl_structures) = room_state.current_rcl_structures.as_ref() {
                        if let Some(room) = rooms().get(room_name) {
                            for structure_type in PRIORITY_OF_STRUCTURES {
                                if room_state.structures.get(&structure_type).map(|xys| xys.len())
                                    != current_rcl_structures.get(&structure_type).map(|xys| xys.len())
                                {
                                    let structure_xys = room_state
                                        .structures
                                        .get(&structure_type)
                                        .map(Vec::as_slice)
                                        .unwrap_or(&[])
                                        .iter()
                                        .copied()
                                        .collect::<FxHashSet<_>>();
                                    let structure_current_rcl_xys = current_rcl_structures
                                        .get(&structure_type)
                                        .map(Vec::as_slice)
                                        .unwrap_or(&[])
                                        .iter()
                                        .copied()
                                        .collect::<FxHashSet<_>>();

                                    for xy in structure_current_rcl_xys.iter() {
                                        if !structure_xys.contains(xy) {
                                            debug!(
                                                "Placing a new construction site for {:?} at {} in {}.",
                                                structure_type, xy, room_name
                                            );

                                            // There is a structure yet to be built. Placing the construction site.
                                            let js_name = structure_js_name(structure_type, room_name, *xy);
                                            room.create_construction_site(
                                                xy.x.u8(),
                                                xy.y.u8(),
                                                structure_type,
                                                js_name.as_ref(),
                                            );

                                            room_construction_sites_count += 1;
                                            total_construction_sites_count += 1;

                                            // Checking if further construction sites may be created.
                                            if room_construction_sites_count >= MAX_CONSTRUCTION_SITES_PER_ROOM as usize
                                            {
                                                break;
                                            }

                                            if total_construction_sites_count >= MAX_CONSTRUCTION_SITES as usize {
                                                return;
                                            }
                                        }
                                    }

                                    for xy in structure_xys.iter() {
                                        if !structure_current_rcl_xys.contains(xy) {
                                            // There is an extra structure in the room. It might happen upon claiming
                                            // a room with structures present or when the room was downgraded.
                                            // TODO Destroy it unless it is on the RCL8 plan or maybe an extension.
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
            }
        });

        sleep(10).await;
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
