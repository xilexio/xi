use std::cmp::Reverse;
use std::collections::BTreeMap;
use log::debug;
use rustc_hash::FxHashMap;
use screeps::{ObjectId, RoomName, Source};
use screeps::Part::Work;
use crate::consts::FAR_FUTURE;
use crate::creeps::creep_role::CreepRole::Miner;
use crate::geometry::position_utils::PositionUtils;
use crate::geometry::room_xy::RoomXYUtils;
use crate::kernel::kernel::{current_priority, schedule};
use crate::kernel::sleep::sleep;
use crate::room_maintenance::mine_source::mine_source;
use crate::room_states::room_states::with_room_state;
use crate::spawning::reserved_creep::find_unassigned_creep;
use crate::u;
use crate::utils::multi_map_utils::{MultiMapUtils, OrderedMultiMapUtils};

const SUFFICIENT_WORK_PARTS: u32 = 5;

pub async fn mine_sources(room_name: RoomName) {
    // Finding unassigned miners and assigning them to sources up to the limit of total work parts.
    // Finding unassigned miners and their distance from each source.
    // If solved optimally, this is similar to the knapsack problem, but we minimize the total
    // distance to sources while making sure that each source has at least
    // `SUFFICIENT_WORK_PARTS` available to it.
    let mut miners_and_dists = Vec::new();
    while let Some(reserved_creep) = find_unassigned_creep(
        room_name,
        Miner,
        None
    ) {
        let mut dists = u!(with_room_state(room_name, |room_state| {
            room_state
            .sources
            .iter()
            .map(|source_data| {
                (
                    source_data.id,
                    source_data.xy.to_pos(room_name).get_range_to(reserved_creep.borrow().travel_state.pos)
                )
            })
            .collect::<Vec<_>>()
        }));
        dists.sort_by_key(|(_, dist)| Reverse(*dist));
        let work_parts = reserved_creep.borrow().body.count_parts(Work);
        miners_and_dists.push((reserved_creep, work_parts, dists));
    }
    
    // Using a greedy approach.
    // Assigning miners ordered by the number of work parts and then the minimum distance to nearest
    // source. This is to ensure that the full-sized creeps are used first.
    let mut initial_miners = FxHashMap::default();
    let mut total_work_parts: FxHashMap<ObjectId<Source>, u32> = FxHashMap::default();
    let mut miners_by_min_dist = BTreeMap::default();
    for (reserved_creep, work_parts, dists) in miners_and_dists.into_iter() {
        let min_dist = u!(dists.last()).1;
        miners_by_min_dist.push_or_insert((Reverse(work_parts), min_dist), (reserved_creep, dists));
    }
    while let Some(((work_parts, _), (reserved_creep, mut dists))) = miners_by_min_dist.pop_from_first() {
        if let Some((closest_source_id, _)) = dists.pop() {
            let source_total_work_parts = total_work_parts.entry(closest_source_id).or_default();
            if *source_total_work_parts < SUFFICIENT_WORK_PARTS {
                // The source needs more miners.
                *source_total_work_parts += work_parts.0 as u32;
                initial_miners.push_or_insert(closest_source_id, reserved_creep);
            } else {
                // The source doesn't need any more miners. Re-adding the miner to the queue.
                if let Some((_, min_dist)) = dists.last() {
                    miners_by_min_dist.push_or_insert((work_parts, *min_dist), (reserved_creep, dists));
                } else {
                    // There are no more sources to assign to. Therefore, no miner can be assigned
                    // anymore. Stopping the assignment.
                    break;
                }
            }
        }
    }
    
    debug!("Assigning existing miners to sources:");
    for (source_id, reserved_creeps) in initial_miners.iter() {
        debug!(
            "* {}: {}",
            source_id,
            reserved_creeps
                .iter()
                .map(|reserved_creep| {
                    format!(
                        "{} at {}",
                        reserved_creep.borrow().name,
                        reserved_creep.borrow().travel_state.pos.f()
                    )
                })
                .collect::<Vec<_>>()
                .join(", ")
        );
    }

    with_room_state(room_name, |room_state| {
        for (source_ix, source_data) in room_state.sources.iter().enumerate() {
            let source_initial_miners = initial_miners.remove(&source_data.id).unwrap_or_else(Vec::new);
            debug!(
                "Setting up mining of {} in {} with {} initial miners.",
                source_data.xy,
                room_name,
                source_initial_miners.len()
            );
            schedule(
                &format!("mine_source_{}_X{}_Y{}", room_name, source_data.xy.x, source_data.xy.y),
                current_priority() - 1,
                mine_source(room_name, source_ix, source_initial_miners),
            );
        }
    });
    
    sleep(FAR_FUTURE).await;
}