use rustc_hash::FxHashSet;
use screeps::game;
use crate::log_err;
use crate::kernel::sleep::sleep;
use crate::room_states::room_state::RoomDesignation;
use crate::room_states::room_states::for_each_room;
use crate::room_states::scan_room::scan_room;

/// Scans visible rooms.
/// It is guaranteed that the bot will scan all visible rooms each tick. 
pub async fn scan_rooms() {
    let mut first_scan = true;
    
    loop {
        let mut visible_room_names = FxHashSet::default();
        
        for room in game::rooms().values() {
            let room_name = room.name();
            visible_room_names.insert(room_name);
            log_err!(scan_room(room_name, first_scan));
            first_scan = false;
        }
        
        for_each_room(|room_name, room_state| {
            if !visible_room_names.contains(&room_name) {
                room_state.designation = RoomDesignation::NotOwned;
            }
        });

        // TODO A proper scan only once per few ticks or when it is somehow requested (e.g., by a scout). However, some
        //      preliminary scan should always happen to detect ownership change.
        sleep(1).await;
    }
}