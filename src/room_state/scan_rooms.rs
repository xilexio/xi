use screeps::game;
use crate::log_err;
use crate::kernel::sleep::sleep;
use crate::room_state::scan_room::scan_room;

/// Scans visible rooms.
/// It is guaranteed that the bot will scan all visible rooms each tick. 
pub async fn scan_rooms() {
    loop {
        let mut first_scan = true;
        
        for room in game::rooms().values() {
            log_err!(scan_room(room.name(), first_scan));
            first_scan = false;
        }

        // TODO A proper scan only once per few ticks or when it is somehow requested (e.g., by a scout). However, some
        //      preliminary scan should always happen to detect ownership change.
        sleep(1).await;
    }
}