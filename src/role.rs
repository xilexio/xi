use screeps::Part;
use crate::creep::CreepRole;
use crate::resources::RoomResources;

pub trait Role {
    fn body(resources: &RoomResources) -> Vec<Part>;
}

// impl From<CreepRole> for Box<dyn Role> {
//     fn from(value: CreepRole) -> Self {
//         match value {
//             CreepRole::Craftsman => {
//             }
//             CreepRole::Scout => {}
//         }
//     }
// }