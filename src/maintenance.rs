use log::debug;
use screeps::{game, RoomName};
use crate::kernel::sleep::sleep;

pub async fn maintain_rooms() {
    loop {
        for room_name in game::rooms().keys() {
            maintain_room(room_name);
        }

        sleep(1).await;
    }
}

fn maintain_room(room_name: RoomName) {
    debug!("Maintaining room {}.", room_name);
}