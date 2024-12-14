use log::trace;
use screeps::RoomName;
use crate::economy::room_eco_config::update_or_create_eco_config;
use crate::kernel::sleep::sleep;
use crate::room_states::room_states::with_room_state;
use crate::utils::sampling::ticks_until_sample_tick;

pub async fn update_eco_config(room_name: RoomName) {
    loop {
        trace!("Updating eco config.");
        
        // This makes sure that eco stats are gathered (and thus exist) before eco config is
        // computed.
        sleep(ticks_until_sample_tick(0) + 1).await;

        with_room_state(room_name, |room_state| {
            update_or_create_eco_config(room_state);
        });

        sleep(ticks_until_sample_tick(0) + 1).await;
    }
}