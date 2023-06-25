use crate::kernel::sleep::sleep;
use crate::kernel::{kill_tree, schedule};
use crate::priorities::{MINING_PRIORITY, ROOM_MAINTENANCE_PRIORITY};
use crate::room_state::room_states::with_room_state;
use crate::u;
use log::debug;
use rustc_hash::{FxHashMap, FxHashSet};
use screeps::{game, RoomName};
use std::iter::once;

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

struct Miner {
    creep_name: Option<String>,
    creep_ticks_to_live: u32,
}

async fn maintain_room(room_name: RoomName) {
    // TODO the sources are constant, but link/container/drop is not

    let number_of_sources = loop {
        if let Some(number_of_sources) = with_room_state(room_name, |room_state| room_state.sources.len()) {
            break number_of_sources;
        } else {
            sleep(1).await;
        }
    };

    let miners: Vec<Option<u8>> = once(None).cycle().take(number_of_sources).collect();

    with_room_state(room_name, |room_state| {
        for (source_ix, source_data) in room_state.sources.iter().enumerate() {
            debug!("Setting up mining of {} in {}.", source_data.xy, room_name);
            drop(schedule(
                &format!("mine_source_{}_X{}_Y{}", room_name, source_data.xy.x, source_data.xy.y),
                MINING_PRIORITY,
                mine_source(room_name, source_ix),
            ));
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
        }
    });

    loop {
        debug!("Maintaining room {}.", room_name);

        sleep(1).await;
    }
}

async fn mine_source(room_name: RoomName, source_ix: usize) {
    loop {


        sleep(1).await;
    }
}
