use crate::kernel::sleep::sleep;
use crate::kernel::{kill_tree, schedule};
use crate::priorities::ROOM_MAINTENANCE_PRIORITY;
use crate::u;
use log::debug;
use rustc_hash::{FxHashMap, FxHashSet};
use screeps::{game, RoomName};

pub async fn maintain_rooms() {
    let mut room_processes = FxHashMap::default();

    loop {
        let mut lost_rooms = room_processes.keys().copied().collect::<FxHashSet<_>>();

        for room_name in game::rooms().keys() {
            lost_rooms.remove(&room_name);

            room_processes.entry(room_name).or_insert_with(|| {
                schedule(
                    &format!("room_process_{}", room_name),
                    ROOM_MAINTENANCE_PRIORITY - 1,
                    maintain_room(room_name),
                )
            });
        }

        for room_name in lost_rooms.into_iter() {
            let room_process = u!(room_processes.remove(&room_name));
            kill_tree(room_process, ());
        }

        sleep(1).await;
    }
}

async fn maintain_room(room_name: RoomName) {
    loop {
        debug!("Maintaining room {}.", room_name);

        // with_room_state(room_name, |room_state| {
        //     // TODO keep a task spawned and working asynchronously for each source
        //     for source_data in room_state.sources.iter() {
        //         let miner = spawn(MINER).await;
        //         // TODO in background
        //         miner.mine().then(|res| {
        //             match res with {
        //                 Dead => ;
        //                 ...
        //             }
        //         })
        //         if dropped_resource > 100 {
        //             haul(resource).();
        //         }
        //     }
        // });

        sleep(1).await;
    }
}
