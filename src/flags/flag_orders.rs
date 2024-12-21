use log::info;
use rustc_hash::FxHashMap;
use screeps::game::flags;
use screeps::HasPosition;
use crate::flags::claim_room::claim_room;
use crate::kernel::kernel::{current_priority, schedule};
use crate::kernel::sleep::sleep;

pub async fn execute_flag_orders() {
    let mut active_flags = FxHashMap::default();
    
    // TODO Remove.
    sleep(4).await;
    
    loop {
        for (flag_name, flag) in flags().entries() {
            // TODO Removing the processes after they are done.
            if !active_flags.contains_key(&flag_name) {
                info!("Found unprocessed flag {}.", flag_name);
                if flag_name.starts_with("claim") {
                    let flag_pos = flag.pos();
                    let room_name = flag_pos.room_name();
                    let process_handle = schedule(
                        &format!("claim_room_{}", room_name),
                        current_priority() - 1,
                        claim_room(flag_pos)
                    );
                    active_flags.insert(flag_name, process_handle);
                }
                
            }
        }
        
        sleep(4).await;
    }
}