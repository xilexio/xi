use screeps::look::STRUCTURES;
use screeps::{Position, RoomName, RoomXY, StructureObject, StructureType};

pub fn get_structure(room_name: RoomName, xy: RoomXY, structure_type: StructureType) -> Option<StructureObject> {
    let pos = Position::new(xy.x, xy.y, room_name);
    pos.look_for(STRUCTURES)
        .into_iter()
        .find(|structure_obj| structure_obj.as_structure().structure_type() == structure_type)
}
