use log::trace;
use screeps::RoomName;
use crate::kernel::sleep::sleep;
use crate::room_state::room_states::with_room_state;

pub async fn build_structures(room_name: RoomName) {
    loop {
        // TODO pick construction site with highest priority
        // TODO spawn a builder
        // TODO send a builder to build it
        
        with_room_state(room_name, |room_state| {
            if room_state.construction_site_queue.is_empty() {
                trace!("Nothing to build in {}.", room_name);
            } else {
                trace!(
                    "Building the following structures in {}: {:?}.",
                    room_name, room_state.construction_site_queue
                );
            }
        });
        
        // TODO Replace this with a condition instead.
        sleep(1).await;
    }
}