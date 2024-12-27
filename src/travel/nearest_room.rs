use screeps::RoomName;
use crate::room_states::room_state::RoomDesignation;
use crate::room_states::room_states::{for_each_owned_room, with_room_state};

pub fn find_nearest_owned_room(target_room_name: RoomName, min_rcl: u8) -> Option<RoomName> {
    let target_room_is_good = with_room_state(target_room_name, |room_state| {
        room_state.designation == RoomDesignation::Owned && room_state.rcl >= min_rcl
    }).unwrap_or(false);
    if target_room_is_good {
        Some(target_room_name)
    } else {
        let mut closest_room_name = None;
        let mut closest_room_dist = i32::MAX;
        for_each_owned_room(|room_name, room_state| {
            if room_state.rcl >= min_rcl {
                let dx = room_name.x_coord() - target_room_name.x_coord();
                let dy = room_name.y_coord() - target_room_name.y_coord();
                let room_dist = dx + dy;

                if room_dist < closest_room_dist {
                    closest_room_name = Some(room_name);
                    closest_room_dist = room_dist;
                }
            }
        });

        closest_room_name
    }
}