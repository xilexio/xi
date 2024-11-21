use std::future::Future;
use log::trace;
use screeps::RoomName;
use crate::kernel::kernel::{current_priority, kill, schedule};
use crate::kernel::sleep::sleep;
use crate::room_states::room_states::with_room_state;
use crate::u;

pub async fn loop_until_structures_change<F>(room_name: RoomName, interval: u32, mut f: F)
where
    F: FnMut() -> bool,
{
    let mut structures_broadcast = u!(with_room_state(room_name, |room_state| {
        room_state.structures_broadcast.clone_not_primed()
    }));

    trace!("Beginning a loop until structures change.");

    // TODO when the check is true, it will always be true this tick.
    while structures_broadcast.check().is_none() {
        if !f() {
            break;
        }

        sleep(interval).await;
    }

    trace!("Structures changed. Finishing the loop.");
}

pub async fn run_future_until_structures_change<F>(room_name: RoomName, f: F)
where
    F: Future<Output = ()> + 'static,
{
    let mut structures_broadcast = u!(with_room_state(room_name, |room_state| {
        room_state.structures_broadcast.clone_not_primed()
    }));

    let handle = schedule("loop_until_structures_change", current_priority() + 1, f);
    trace!("Starting process {} until structures change.", handle.pid);

    // TODO Not active waiting.
    while structures_broadcast.check().is_none() {
        // TODO The ability to check if the process has already ended. Maybe just select()?
        sleep(1).await;
    }
    
    trace!("Structures changed. Killing the process {}.", handle.pid);
    kill(handle, ());
}