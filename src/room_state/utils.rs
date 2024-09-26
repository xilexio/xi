use screeps::RoomName;
use crate::kernel::sleep::sleep;
use crate::room_state::room_states::with_room_state;
use crate::u;

pub async fn loop_until_structures_change<F>(room_name: RoomName, interval: u32, mut f: F)
where
    F: FnMut() -> bool,
{
    let mut structures_broadcast = u!(with_room_state(room_name, |room_state| {
        room_state.structures_broadcast.clone_inactive()
    }));

    // TODO when the check is true, it will always be true this tick.
    while structures_broadcast.check().is_none() {
        if !f() {
            break;
        }

        sleep(interval).await;
    }
}