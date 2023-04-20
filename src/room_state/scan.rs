use screeps::{game, RoomName, OwnedStructureProperties, find, Position, ResourceType, ObjectId, Mineral, HasTypedId, Source, StructureController};
use crate::room_state::{ControllerInfo, MineralInfo, RoomDesignation, RoomState, SourceInfo};
use crate::room_state::room_states::replace_room_state;

#[derive(Debug, Clone)]
pub struct RoomVisibilityError;

pub fn scan(room_name: RoomName) -> Result<(), RoomVisibilityError> {
    replace_room_state(room_name, |state| {
        update_room_state_from_scan(room_name, state)
    })
}

pub fn update_room_state_from_scan(room_name: RoomName, state: &mut RoomState) -> Result<(), RoomVisibilityError> {
    let room = match game::rooms().get(room_name) {
        Some(room) => room,
        None => return Err(RoomVisibilityError),
    };
    if let Some(controller) = room.controller() {
        state.rcl = controller.level();
        let id: ObjectId<StructureController> = controller.id();
        let pos: Position = controller.pos().into();
        state.controller = Some(ControllerInfo {
            id,
            xy: pos.xy(),
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
        state.sources.push(SourceInfo {
            id,
            xy: pos.xy(),
        });
    };
    for mineral in room.find(find::MINERALS, None) {
        let id: ObjectId<Mineral> = mineral.id();
        let pos: Position = mineral.pos().into();
        let mineral_type: ResourceType = mineral.mineral_type();
        state.mineral = Some(MineralInfo {
            id,
            xy: pos.xy(),
            mineral_type,
        });
    };
    state.terrain = game::map::get_room_terrain(room_name).into();
    // TODO structures
    Ok(())
}