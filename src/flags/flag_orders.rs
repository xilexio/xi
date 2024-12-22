use std::collections::hash_map::Entry;
use log::info;
use rustc_hash::FxHashMap;
use screeps::game::flags;
use screeps::HasPosition;
use crate::flags::claim_room::claim_room;
use crate::flags::forced_build::forced_build;
use crate::geometry::position_utils::PositionUtils;
use crate::kernel::kernel::{current_priority, schedule};
use crate::kernel::sleep::sleep;

pub async fn execute_flag_orders() {
    let mut active_flags = FxHashMap::default();
    
    // TODO Remove.
    sleep(4).await;
    
    loop {
        for (flag_name, flag) in flags().entries() {
            // TODO Removing the processes after they are done.
            if let Entry::Vacant(e) = active_flags.entry(flag_name) {
                let flag_name = e.key();
                info!("Found unprocessed flag {}.", flag_name);
                if flag_name.starts_with("claim") {
                    let flag_pos = flag.pos();
                    let room_name = flag_pos.room_name();
                    let process_handle = schedule(
                        &format!("claim_room_{}", room_name),
                        current_priority() - 1,
                        claim_room(flag_pos)
                    );
                    e.insert(process_handle);
                } else if flag_name.starts_with("build") {
                    let flag_pos = flag.pos();
                    let process_handle = schedule(
                        &format!("forced_build_{}", flag_pos.f()),
                        current_priority() - 1,
                        forced_build(flag_pos)
                    );
                    e.insert(process_handle);
                }
            }
        }
        
        sleep(4).await;
    }
}