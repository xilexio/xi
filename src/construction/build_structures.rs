use log::trace;
use screeps::RoomName;
use crate::kernel::sleep::sleep;
use crate::room_state::room_states::with_room_state;
use crate::u;

pub async fn build_structures(room_name: RoomName) {
    let mut builder = Some(42);
    
    loop {
        // TODO pick construction site with highest priority
        // TODO spawn a builder
        // TODO send a builder to build it
        
        let construction_site = u!(with_room_state(room_name, |room_state| {
            if room_state.construction_site_queue.is_empty() {
                trace!("Nothing to build in {}.", room_name);
                None
            } else {
                trace!(
                    "Building the following structures in {}: {:?}.",
                    room_name, room_state.construction_site_queue
                );
                room_state.construction_site_queue.first().cloned()
            }
        }));
        
        if let Some(construction_site) = construction_site {
            // TODO spawn pool that has a builder but is droppable
            // TODO After spawning the builder, making it pick up the energy from storage if there
            //      is one.
            // TODO Sending the builder to the construction site.
            // TODO Building the construction site.
            // TODO Requesting haul of energy when about to run of out energy.
            sleep(1).await;
        } else {
            builder.take();
            
            sleep(10).await;
        }
    }
}