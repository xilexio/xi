use log::{debug, trace};
use crate::room_states::room_states::map_and_replace_room_state;
use crate::{local_trace, u};
use rustc_hash::{FxHashMap, FxHashSet};
use screeps::StructureType::{Extension, Spawn};
use screeps::{find, game, HasId, HasPosition, Mineral, ObjectId, OwnedStructureProperties, Position, ResourceType, RoomName, Source, StructureController};
use screeps::ResourceType::Energy;
use crate::economy::room_eco_stats::RoomEcoStats;
use crate::errors::XiError;
use crate::room_states::room_state::{ControllerData, MineralData, RoomDesignation, RoomResources, RoomState, SourceData, StructureData};

const DEBUG: bool = true;

/// Updates the state of given room, i.e., records the terrain, structures, resources and other data.
/// Fails if the room is not visible.
pub fn scan_room(room_name: RoomName, force_update: bool) -> Result<(), XiError> {
    map_and_replace_room_state(room_name, |state| update_room_state_from_scan(room_name, force_update, state))
}

pub fn update_room_state_from_scan(room_name: RoomName, force_update: bool, state: &mut RoomState) -> Result<(), XiError> {
    local_trace!("Scanning room {} with force_update={}.", room_name, force_update);
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
        });
    };
    local_trace!("Room designation: {:?}", state.designation);
    state.sources = Vec::new();
    for source in room.find(find::SOURCES, None) {
        let id: ObjectId<Source> = source.id();
        let xy = source.pos().xy();
        let mut work_xy = None;
        if state.designation == RoomDesignation::Owned {
            work_xy = state
                .plan
                .as_ref()
                .map(|plan| u!(plan.sources.iter().find(|source_data| source_data.source_xy == xy)).work_xy);
        }
        // TODO container_id, link_xy, link_id
        state.sources.push(SourceData {
            id,
            xy,
            work_xy,
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
    // TODO Only needed the first time.
    state.terrain = u!(game::map::get_room_terrain(room_name)).into();
    let mut structures = FxHashMap::default();
    let mut structures_changed = force_update;
    for structure in room.find(find::STRUCTURES, None) {
        let structure_type = structure.as_structure().structure_type();
        let xy = structure.pos().xy();
        structures
            .entry(structure_type)
            .or_insert_with(FxHashSet::default)
            .insert(xy);

        if let Some(xys) = state.structures.get(&structure_type) {
            if !xys.contains(&xy) {
                structures_changed = true;
            }
        } else {
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
        state.spawns.clear();
        state.extensions.clear();

        // Updating sorted lists of structures.
        for structure in room.find(find::STRUCTURES, None) {
            let structure_type = structure.as_structure().structure_type();
            let xy = structure.pos().xy();
            if state.designation == RoomDesignation::Owned {
                if structure_type == Spawn {
                    // TODO Something is wrong as it ends up being 17 same spawns.
                    state
                        .spawns
                        .push(StructureData::new(structure.as_structure().id().into_type(), xy));
                }
                if structure_type == Extension {
                    state
                        .extensions
                        .push(StructureData::new(structure.as_structure().id().into_type(), xy));
                }
            }
        }
        // TODO sort lists of structures
        // TODO fast filler data
        
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
