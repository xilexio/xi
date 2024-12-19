use log::debug;
use crate::room_states::room_states::map_and_replace_room_state;
use crate::{local_debug, u};
use rustc_hash::FxHashMap;
use screeps::{find, game, HasId, HasPosition, Mineral, ObjectId, OwnedStructureProperties, Position, ResourceType, RoomName, Source, StructureController};
use screeps::ResourceType::Energy;
use screeps::Terrain::Wall;
use crate::construction::triage_repair_sites::StructureToRepair;
use crate::economy::room_eco_stats::RoomEcoStats;
use crate::errors::XiError;
use crate::geometry::room_xy::RoomXYUtils;
use crate::room_states::room_state::{ControllerData, MineralData, RoomDesignation, RoomResources, RoomState, SourceData};
use crate::utils::game_tick::game_tick;
use crate::utils::multi_map_utils::MultiMapUtils;

const DEBUG: bool = true;

/// Updates the state of given room, i.e., records the terrain, structures, resources and other
/// data. Fails if the room is not visible.
pub fn scan_room(room_name: RoomName, force_update: bool) -> Result<(), XiError> {
    map_and_replace_room_state(room_name, |state| update_room_state_from_scan(room_name, force_update, state))
}

/// Updates the state of a given room, given the room state to update.
// TODO double borrow at xi::room_states::scan_rooms::scan_rooms::{{closure}}::hd7cffa08165cb64e (wasm-function[1002]:206)
pub fn update_room_state_from_scan(room_name: RoomName, force_update: bool, state: &mut RoomState) -> Result<(), XiError> {
    local_debug!("Scanning room {} with force_update={}.", room_name, force_update);
    let room = match game::rooms().get(room_name) {
        Some(room) => room,
        None => Err(XiError::RoomVisibilityError)?,
    };
    if let Some(controller) = room.controller() {
        state.rcl = controller.level();
        let id: ObjectId<StructureController> = controller.id();
        let pos: Position = controller.pos();
        let mut work_xy = None;
        let link_xy = None; // TODO This requires information if the link and core have been constructed.
        if let Some(owner) = controller.owner() {
            state.owner = owner.username();
            if controller.my() {
                state.designation = RoomDesignation::Owned;
                
                if let Some(plan) = state.plan.as_ref() {
                    // TODO How about not at RCL8? Is it the same work_xy?
                    work_xy = Some(plan.controller.work_xy);
                }
            } else {
                state.designation = RoomDesignation::NotOwned;
            }
        }
        state.controller = Some(ControllerData {
            id,
            xy: pos.xy(),
            work_xy,
            link_xy,
            downgrade_tick: game_tick() + controller.ticks_to_downgrade().unwrap_or(0)
        });
    };
    local_debug!("Room designation: {:?}", state.designation);
    // TODO Only needed the first time.
    state.terrain = u!(game::map::get_room_terrain(room_name)).into();
    state.sources = Vec::new();
    for source in room.find(find::SOURCES, None) {
        let id: ObjectId<Source> = source.id();
        let xy = source.pos().xy();
        let work_xy = (state.designation == RoomDesignation::Owned).then(|| {
            state
                .plan
                .as_ref()
                .map(|plan| u!(plan.sources.iter().find(|source_data| source_data.source_xy == xy)).work_xy)
        }).flatten();
        let drop_mining_xys = (state.designation == RoomDesignation::Owned).then(|| {
            xy.around().filter(|&xy| state.terrain.get(xy) != Wall).collect()
        }).unwrap_or_default();
        // TODO container_id, link_xy, link_id
        state.sources.push(SourceData {
            id,
            xy,
            work_xy,
            drop_mining_xys,
            container_id: None,
            link_xy: None,
            link_id: None,
        });
    }
    for mineral in room.find(find::MINERALS, None) {
        let id: ObjectId<Mineral> = mineral.id();
        let pos: Position = mineral.pos();
        let mineral_type: ResourceType = mineral.mineral_type();
        state.mineral = Some(MineralData {
            id,
            xy: pos.xy(),
            mineral_type,
        });
    }
    let mut structures = FxHashMap::default();
    state.structures_to_repair.clear();
    let mut structures_changed = force_update;
    // Note that it also finds the controller and other such structures.
    for structure in room.find(find::STRUCTURES, None) {
        let structure = structure.as_structure();
        let structure_type = structure.structure_type();
        let xy = structure.pos().xy();
        let id = structure.id();
        structures
            .entry(structure_type)
            .or_insert_with(FxHashMap::default)
            .insert(xy, id);
        
        let hits = structure.hits();
        let hits_max = structure.hits_max();
        
        if hits < hits_max {
            state.structures_to_repair.push_or_insert(structure_type, StructureToRepair {
                id,
                xy,
                hits,
                hits_max,
            });
        }

        let is_in_state = state
            .structures
            .get(&structure_type)
            .map_or(false, |state_xys| {
                state_xys.get(&xy).map_or(false, |&state_id| state_id == id)
            });

        if !is_in_state {
            structures_changed = true;
        }
    }
    if !structures_changed {
        for (structure_type, state_xys) in state.structures.iter() {
            if let Some(xys) = structures.get(structure_type) {
                if xys.len() != state_xys.len() {
                    structures_changed = true;
                    break;
                }
            } else {
                structures_changed = true;
                break;
            }
        }
    }
    if structures_changed {
        debug!("Structures in room {room_name} changed.");
        state.structures = structures;

        // TODO Fast filler data.

        state.update_structures_matrix();

        // Informing waiting processes that the structure changed.
        state.structures_broadcast.broadcast(());
    }
    
    if state.designation == RoomDesignation::Owned {
        state.resources = RoomResources {
            spawn_energy: room.energy_available(),
            spawn_energy_capacity: room.energy_capacity_available(),
            storage_energy: room.storage().map_or(0, |storage| storage.store().get(Energy).unwrap_or(0)),
        };
        
        if state.eco_stats.is_none() {
            state.eco_stats = Some(RoomEcoStats::default());
        }
    } else {
        state.eco_stats.take();
        state.eco_config.take();
    }
    
    Ok(())
}
