use screeps::game;
use crate::log_err;
use crate::kernel::sleep::sleep;
use crate::room_state::scan_room::scan_room;

/// Scans visible rooms.
pub async fn scan_rooms() {
    loop {
        for room in game::rooms().values() {
            log_err!(scan_room(room.name()));
        }

        // TODO A proper scan only once per few ticks or when it is somehow requested (e.g., by a scout). However, some
        //      preliminary scan should always happen to detect ownership change.
        sleep(1).await;
    }
}