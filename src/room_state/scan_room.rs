use log::debug;
use crate::room_state::room_states::replace_room_state;
use crate::room_state::{ControllerData, MineralData, RoomDesignation, RoomState, SourceData, StructureData};
use crate::u;
use rustc_hash::{FxHashMap, FxHashSet};
use screeps::StructureType::{Extension, Spawn};
use screeps::{
    find, game, HasPosition, HasTypedId, Mineral, ObjectId, OwnedStructureProperties, Position, ResourceType, RoomName,
    Source, StructureController,
};
use thiserror::Error;

#[derive(Error, Debug)]
pub enum ScanError {
    #[error("failed to scan the room due to lack of visibility")]
    RoomVisibilityError,
}

/// Updates the state of given room, i.e., records the terrain, structures, resources and other data.
/// Fails if the room is not visible.
pub fn scan_room(room_name: RoomName) -> Result<(), ScanError> {
    replace_room_state(room_name, |state| update_room_state_from_scan(room_name, state))
}

pub fn update_room_state_from_scan(room_name: RoomName, state: &mut RoomState) -> Result<(), ScanError> {
    let room = match game::rooms().get(room_name) {
        Some(room) => room,
        None => Err(ScanError::RoomVisibilityError)?,
    };
    if let Some(controller) = room.controller() {
        state.rcl = controller.level();
        let id: ObjectId<StructureController> = controller.id();
        let pos: Position = controller.pos();
        state.controller = Some(ControllerData {
            id,
            xy: pos.xy(),
            work_xy: None,
            link_xy: None,
        });
        if let Some(owner) = controller.owner() {
            state.owner = owner.username();
            if controller.my() {
                state.designation = RoomDesignation::Owned;
            } else {
                state.designation = RoomDesignation::NotOwned;
            }
        }
    };
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
    state.terrain = game::map::get_room_terrain(room_name).into();
    let mut structures = FxHashMap::default();
    let mut structures_changed = false;
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
        // TODO Definitely not changed but this branch is taken.
        // TODO "New spawn" is being registered as many ticks as there was in the game.
        debug!("Structures in room {room_name} changed.");
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
    Ok(())
}
