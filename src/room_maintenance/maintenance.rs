use crate::kernel::sleep::sleep;
use crate::kernel::kernel::{current_priority, kill_tree, schedule};
use crate::priorities::SPAWNING_CREEPS_PRIORITY;
use crate::room_states::room_states::with_room_state;
use log::{debug, info};
use rustc_hash::{FxHashMap, FxHashSet};
use screeps::{game, RoomName};
use crate::construction::build_structures::build_structures;
use crate::construction::repair_structures::repair_structures;
use crate::construction::triage_repair_sites::triage_repair_sites;
use crate::consts::FAR_FUTURE;
use crate::economy::gather_eco_samples::gather_eco_samples;
use crate::economy::update_eco_config::update_eco_config;
use crate::room_maintenance::fill_structures_with_energy::fill_structures_with_energy;
use crate::hauling::haul_resources::haul_resources;
use crate::room_maintenance::mine_sources::mine_sources;
use crate::spawning::spawn_room_creeps::{spawn_room_creeps, update_spawn_list};
use crate::u;
use crate::room_maintenance::upgrade_controller::upgrade_controller;

/// Each tick, schedule or kill processes to maintain a room.
pub async fn maintain_rooms() {
    let mut room_processes = FxHashMap::default();

    loop {
        // Checking which rooms were lost by comparing them with the current information contained
        // keys of `room_processes`.
        let mut lost_rooms = room_processes.keys().cloned().collect::<FxHashSet<_>>();

        for room_name in game::rooms().keys() {
            lost_rooms.remove(&room_name);

            // Only maintaining rooms that have a plan are maintained.
            // Finding out if the room has a plan.
            let has_plan = with_room_state(room_name, |room_state| {
                room_state.plan.is_some()
            }).unwrap_or(false);
            
            if has_plan {
                room_processes.entry(room_name).or_insert_with(|| {
                    // Schedule the room maintenance process to run later so that it can be killed
                    // before it runs in the tick the room is lost.
                    schedule(
                        &format!("maintain_room_{}", room_name),
                        current_priority() - 1,
                        maintain_room(room_name),
                    )
                });
            }
        }

        for room_name in lost_rooms.into_iter() {
            let room_process = u!(room_processes.remove(&room_name));
            info!("Lost room {}.", room_name);
            kill_tree(room_process, ());
            // TODO Release other room resources, reallocate creeps.
        }

        sleep(1).await;
    }
}

async fn maintain_room(room_name: RoomName) {
    with_room_state(room_name, |room_state| {
        let structures_broadcast = room_state.structures_broadcast.clone_primed();
    
        // Reacting to changes in structures in the room.
        // This and subsequent processes are scheduled with a lower priority so that they run
        // later than this process.
        schedule(
            &format!("update_structures_{}", room_name),
            current_priority() - 1,
            async move {
                loop {
                    update_spawn_list(room_name);
    
                    structures_broadcast.clone_not_primed().await;
                    debug!("Structures have changed in maintain rooms.");
                }
            },
        );

        // Schedule filling the spawns and extensions.
        schedule(
            &format!("fill_structures_with_energy_{}", room_name),
            current_priority() - 1,
            fill_structures_with_energy(room_name)
        );

        // Schedule mining sources inside the room.
        schedule(
            &format!("mine_sources_{}", room_name),
            current_priority() - 1,
            mine_sources(room_name)
        );

        // Handle scheduled hauls and control haulers.
        schedule(
            &format!("haul_resources_{}", room_name),
            current_priority() - 1,
            haul_resources(room_name)
        );
        
        schedule(
            &format!("gather_eco_samples_{}", room_name),
            current_priority() - 10,
            gather_eco_samples(room_name)
        );

        // Update stats and decide on resource distribution within the room.
        schedule(
            &format!("update_eco_config_{}", room_name),
            current_priority() - 11,
            update_eco_config(room_name)
        );

        // Spawning creeps is scheduled to run later to react to spawning requests.
        schedule(
            &format!("spawn_creeps_{}", room_name),
            SPAWNING_CREEPS_PRIORITY,
            async move {
                loop {
                    spawn_room_creeps(room_name);
                    sleep(1).await;
                }
            },
        );

        // Upgrade the controller, spawn upgraders and schedule hauling of the energy.
        schedule(
            &format!("upgrade_controller_{}", room_name),
            current_priority() - 1,
            upgrade_controller(room_name)
        );

        // Build structures in the room and spawn builders.
        schedule(
            &format!("build_structures_{}", room_name),
            current_priority() - 1,
            build_structures(room_name)
        );
        
        // Order structures to be repaired in the room.
        // TODO Shouldn't this be more global?
        schedule(
            &format!("select_repair_sites_{}", room_name),
            current_priority() - 1,
            triage_repair_sites(room_name)
        );
        
        // Repair structures in the room and spawn repairers.
        // Should run after selecting the repair sites.
        schedule(
            &format!("repair_structures_{}", room_name),
            current_priority() - 2,
            repair_structures(room_name)
        );
    });

    debug!("Finished setting up maintenance of room {}.", room_name);
    // The process has done its job, now it is waiting for the whole tree to be killed when
    // the room is lost.
    sleep(FAR_FUTURE).await;
}