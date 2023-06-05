use screeps::{game, RoomName};
use screeps::ResourceType::Energy;
use crate::unwrap;

pub struct RoomResources {
    pub spawn_energy: u32,
    pub spawn_energy_capacity: u32,
    pub storage_energy: u32,
}

pub fn room_resources(room_name: RoomName) -> RoomResources {
    let room = unwrap!(game::rooms().get(room_name));
    RoomResources {
        spawn_energy: room.energy_available(),
        spawn_energy_capacity: room.energy_capacity_available(),
        storage_energy: room.storage().map_or(0, |storage| storage.store().get(Energy).unwrap_or(0))
    }
}