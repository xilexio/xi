use crate::room_state::room_states::replace_room_state;
use crate::room_state::{ControllerData, MineralData, RoomDesignation, RoomState, SourceData};
use screeps::{
    find, game, HasTypedId, Mineral, ObjectId, OwnedStructureProperties, Position, ResourceType, RoomName, Source,
    StructureController,
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
        let pos: Position = controller.pos().into();
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
        let pos: Position = source.pos().into();
        state.sources.push(SourceData {
            id,
            xy: pos.xy(),
            work_xy: None,
            link_xy: None,
        });
    }
    for mineral in room.find(find::MINERALS, None) {
        let id: ObjectId<Mineral> = mineral.id();
        let pos: Position = mineral.pos().into();
        let mineral_type: ResourceType = mineral.mineral_type();
        state.mineral = Some(MineralData {
            id,
            xy: pos.xy(),
            mineral_type,
        });
    }
    state.terrain = game::map::get_room_terrain(room_name).into();
    // TODO structures
    Ok(())
}
