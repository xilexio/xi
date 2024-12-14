use log::trace;
use screeps::RoomName;
use crate::kernel::sleep::sleep;
use crate::room_states::room_states::with_room_state;
use crate::utils::sampling::ticks_until_sample_tick;

pub async fn gather_eco_samples(room_name: RoomName) {
    sleep(ticks_until_sample_tick(0)).await;
    
    loop {
        trace!("Gathering eco samples.");
        
        with_room_state(room_name, |room_state| {
            if let Some(eco_stats) = room_state.eco_stats.as_mut() {
                eco_stats.push_creep_stats_samples();
            }
        });

        sleep(ticks_until_sample_tick(1)).await;
    }
}