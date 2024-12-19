use screeps::{Direction, ObjectId, Position, RawObjectId, RoomXY, StructureSpawn};
use screeps::StructureType::Spawn;
use crate::geometry::room_xy::RoomXYUtils;
use crate::room_states::room_state::RoomState;

#[derive(Debug, Clone)]
pub struct PreferredSpawn {
    /// ID of the spawn to spawn from.
    pub id: ObjectId<StructureSpawn>,
    /// Allowed directions in which the creep should move from the spawn upon spawning.
    pub directions: Vec<Direction>,
    /// Extra energy cost incurred by selecting this spawn.
    pub extra_cost: u32,
    /// Position of the spawn.
    pub pos: Position,
}

pub fn best_spawns(room_state: &RoomState, target_xy: Option<RoomXY>) -> Vec<PreferredSpawn> {
    if let Some(target_xy) = target_xy {
        let mut spawns = room_state
            .structures
            .get(&Spawn)
            .iter()
            .flat_map(|xys| {
                xys.iter().map(|(&xy, &id)| (
                    target_xy.get_range_to(xy),
                    PreferredSpawn {
                        id: RawObjectId::from(id).into(),
                        directions: Vec::new(),
                        extra_cost: 0,
                        pos: xy.to_pos(room_state.room_name),
                    },
                ))
            })
            .collect::<Vec<_>>();

        spawns.sort_by_key(|(dist, _)| *dist);

        spawns.into_iter().map(|(_, spawn)| spawn).collect()
    } else {
        room_state
            .structures
            .get(&Spawn)
            .iter()
            .flat_map(|xys| {
                xys.iter().map(|(&xy, &id)| PreferredSpawn {
                    id: RawObjectId::from(id).into(),
                    directions: Vec::new(),
                    extra_cost: 0,
                    pos: xy.to_pos(room_state.room_name),
                })
            })
            .collect()
    }
}