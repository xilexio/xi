use log::trace;
use screeps::RoomName;
use crate::economy::room_eco_config::RoomEcoConfig;
use crate::kernel::sleep::sleep;
use crate::room_state::room_states::with_room_state;

pub async fn update_eco_config(room_name: RoomName) {
    loop {
        trace!("Updating eco config.");

        with_room_state(room_name, |room_state| {
            room_state.eco_config = Some(RoomEcoConfig::new(room_state));
        });

        sleep(20).await;
    }
}