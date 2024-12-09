use std::cmp::max;
use log::{info, trace, warn};
use screeps::{ENERGY_REGEN_TIME, SOURCE_ENERGY_CAPACITY};
use screeps::Part::{Carry, Move, Work};
use screeps::StructureType::{Spawn, Storage};
use serde::{Deserialize, Serialize};
use crate::creeps::creep_body::CreepBody;
use crate::creeps::creep_role::CreepRole::{Hauler, Miner};
use crate::geometry::room_xy::RoomXYUtils;
use crate::room_planning::room_planner::SOURCE_AND_CONTROLLER_ROAD_RCL;
use crate::room_states::room_state::RoomState;
use crate::travel::surface::Surface;
use crate::u;

const MIN_SAFE_LAST_CREEP_TTL: u32 = 300;

/// Structure containing parameters for the room economy that decide the distribution of resources
/// as well as composition of creeps.
#[derive(Debug, Deserialize, Serialize)]
pub struct RoomEcoConfig {
    /// The number of haulers that should be currently spawned.
    pub haulers_required: u32,
    /// The body of a hauler.
    pub hauler_body: CreepBody,

    /// The number of miners that should be currently spawned.
    /// Miners are shared by all room sources.
    pub miners_required: u32,
    /// The body of a miner to spawn for each room source.
    pub miner_body: CreepBody,

    /// The number of upgraders to spawn.
    pub upgraders_required: u32,
    /// The body of an upgrader.
    pub upgrader_body: CreepBody,

    /// The number of builders to spawn.
    pub builders_required: u32,
    /// The body of a builder.
    pub builder_body: CreepBody,
}

impl RoomEcoConfig {
    // TODO Take previous room eco and other stats and adjust the values according to actual usage.
    // TODO Also include spawn occupation in the calculation, particularly at low RCL.
    pub fn new(room_state: &RoomState) -> Self {
        let miner_stats = u!(room_state.eco_stats.as_ref()).creep_stats(Miner);
        let hauler_stats = u!(room_state.eco_stats.as_ref()).creep_stats(Hauler);

        let spawn_energy = room_state.resources.spawn_energy;
        let spawn_energy_capacity = room_state.resources.spawn_energy_capacity;

        let number_of_sources = room_state.sources.len() as u32;

        let single_source_energy_income = SOURCE_ENERGY_CAPACITY as f32 / ENERGY_REGEN_TIME as f32;
        let energy_income = number_of_sources as f32 * single_source_energy_income;

        // Hauler uses energy only on its body.
        let mut hauler_body = Self::hauler_body(spawn_energy_capacity);
        let base_hauler_body_energy_usage = hauler_body.body_energy_usage();

        let roads_used = room_state.rcl >= SOURCE_AND_CONTROLLER_ROAD_RCL;

        let hauler_throughput = hauler_body
            .hauling_throughput(if roads_used { Surface::Road } else { Surface::Plain }) / 2.0;

        let controller_work_pos = u!(u!(room_state.controller).work_xy).to_pos(room_state.room_name);

        let mut body_cost_multiplier = 1f32;
        // Average distance from storage or sources to the controller work_xy.
        let avg_storage_controller_dist ;
        // Average distance from sources to the spawn.
        let mut avg_source_spawn_dist = 0f32;

        if let Some(storage_pos) = room_state.structure_pos(Storage) {
            avg_storage_controller_dist = controller_work_pos.get_range_to(storage_pos) as f32;
        } else {
            // The usual case when there is no storage is that there is a single spawn.
            // If, for any reason, there are more, the calculations will still be a decent
            // approximation as they will be nearby.
            if let Some(spawn_pos) = room_state.structure_pos(Spawn) {
                let spawn_distance_sum = room_state
                    .sources
                    .iter()
                    .map(|source_data| {
                        let source_work_pos = u!(source_data.work_xy).to_pos(room_state.room_name);
                        source_work_pos.get_range_to(spawn_pos) as f32
                    })
                    .sum::<f32>();
                avg_source_spawn_dist = spawn_distance_sum / room_state.sources.len() as f32;
            }
            // Multiplier of how much does a creep body cost including the cost of haulers.
            // Note that hauler energy usage should also be multiplied.
            body_cost_multiplier = 1.0 / (1.0 - avg_source_spawn_dist / hauler_throughput * base_hauler_body_energy_usage);
            if !(0.0..=2.0).contains(&body_cost_multiplier) {
                warn!("Improbable body cost multiplier computed: {:.2}.", body_cost_multiplier);
                body_cost_multiplier = 2.0;
            }

            let controller_distance_sum = room_state
                .sources
                .iter()
                .map(|source_data| {
                    let source_work_pos = u!(source_data.work_xy).to_pos(room_state.room_name);
                    source_work_pos.get_range_to(controller_work_pos)
                })
                .sum::<u32>();
            avg_storage_controller_dist = controller_distance_sum as f32 / room_state.sources.len() as f32;
        }

        let mut energy_balance = energy_income;
        let mut total_hauling_throughput = 0f32;

        // Miner uses energy only on its body.
        // TODO Link mining.
        let mut miner_body = Self::miner_body(spawn_energy_capacity, true);
        // When the room is out of essential creeps needed to sustain spawning other creeps, it
        // cannot just spawn any creep of any size.
        // Note that there is no need to prevent spawning of other creeps since they have smaller
        // priority anyway.
        // TODO Do something when all miners are low on TTL.
        let small_miner_required = miner_stats.number_of_creeps == 0
            && hauler_stats.number_of_creeps == 0
            && spawn_energy < miner_body.energy_cost() + hauler_body.energy_cost();
        if small_miner_required {
            // TODO Link mining.
            miner_body = Self::miner_body(0, true);
        }
        let miners_required = if small_miner_required {
            1
        } else {
            (single_source_energy_income / miner_body.energy_harvest_power() as f32).ceil() as u32 * number_of_sources
        };
        let total_miner_body_energy_usage = miners_required as f32 * miner_body.body_energy_usage() * body_cost_multiplier;
        let total_mining_energy_usage = total_miner_body_energy_usage;
        let mining_hauling_throughput = total_miner_body_energy_usage * avg_source_spawn_dist;
        total_hauling_throughput += mining_hauling_throughput;
        energy_balance -= total_mining_energy_usage;

        // Builder uses energy on its body and building.
        // Ignoring the time spent travelling for the purpose of energy usage.
        // This is only a one-time cost, so when storage is available, there is no real limit on
        // the number of builders as long as all are used and there is enough spawn capacity.
        let builder_body = Self::builder_body(spawn_energy_capacity);
        let mut total_building_energy_usage = 0f32;
        let mut building_creep_energy_usage = 0f32;
        let mut building_hauling_throughput = 0f32;
        let mut builders_required = 0;
        let mut total_construction_site_energy_needed = 0;
        if !room_state.construction_site_queue.is_empty() {
            // The planned amount.
            let target_building_energy_usage = energy_balance * 0.7;
            // TODO This is an approximate version assuming most stuff is around spawn and only
            //      works before storage.
            let avg_storage_construction_site_dist = avg_source_spawn_dist + 10.0;

            let builder_body_energy_usage = builder_body.body_energy_usage() * body_cost_multiplier;
            let builder_build_energy_usage = builder_body.build_energy_usage() as f32;
            let builder_energy_usage = builder_body_energy_usage + builder_build_energy_usage;
            builders_required = max(1, (target_building_energy_usage / builder_energy_usage) as u32);
            // The actual amount considering rounding of the number of creeps.
            building_creep_energy_usage = builders_required as f32 * builder_body_energy_usage;
            total_building_energy_usage = builders_required as f32 * builder_energy_usage;
            energy_balance -= total_building_energy_usage;

            building_hauling_throughput = builders_required as f32 * (builder_body_energy_usage * avg_source_spawn_dist + builder_build_energy_usage * avg_storage_construction_site_dist);
            total_hauling_throughput += building_hauling_throughput;

            total_construction_site_energy_needed = room_state
                .construction_site_queue
                .iter().map(|cs| {
                    u!(cs.structure_type.construction_cost())
                })
                .sum();
        }

        // Upgrader uses energy on its body and upgrading.
        // Ignoring the time spent travelling for the purpose of energy usage.
        let upgrader_body = Self::upgrader_body(spawn_energy_capacity);
        let upgrader_body_energy_usage = upgrader_body.body_energy_usage() * body_cost_multiplier;
        let upgrader_upgrade_energy_usage = upgrader_body.upgrade_energy_usage() as f32;
        let upgrader_energy_usage = upgrader_body_energy_usage + upgrader_upgrade_energy_usage;
        let upgraders_required = max(0, (energy_balance / upgrader_energy_usage) as u32);
        let upgrading_creep_energy_usage = upgraders_required as f32 * upgrader_body_energy_usage;
        let total_upgrading_energy_usage = upgraders_required as f32 * upgrader_energy_usage;
        energy_balance -= total_upgrading_energy_usage;

        let upgrading_hauling_throughput = upgraders_required as f32 * (upgrader_body_energy_usage * avg_source_spawn_dist + upgrader_upgrade_energy_usage * avg_storage_controller_dist);
        total_hauling_throughput += upgrading_hauling_throughput;

        // TODO
        // The leftover energy is hauled to the storage.
        // This is, in particular, the case in a fully built RCL8 room in which there is no infinite
        // sink of energy in a room.
        let storage_energy_usage = 0f32;
        let storage_hauling_throughput = 0f32;
        energy_balance -= storage_energy_usage;

        // When the room is out of essential creeps needed to sustain spawning other creeps, it
        // cannot just spawn any creep of any size.
        let small_hauler_required = hauler_stats.number_of_creeps == 0
            && spawn_energy < hauler_body.energy_cost();
        if small_hauler_required {
            hauler_body = Self::hauler_body(0);
        }
        // When there are no miners, do not spawn any new haulers.
        // TODO Unless there is energy to spare in the storage.
        let no_haulers_required = miner_stats.number_of_creeps == 0 && spawn_energy < miner_body.energy_cost();
        // Computing the fraction of a hauler that remains unused. Also, adding some extra fraction
        // for safety buffer.
        let haulers_required_exact = total_hauling_throughput / hauler_throughput;
        let extra_haulers_exact = 0.5;
        let haulers_required = if no_haulers_required {
            0
        } else if small_hauler_required {
            1
        } else {
            (haulers_required_exact + extra_haulers_exact).ceil() as u32
        };
        let extra_haulers = haulers_required as f32 - haulers_required_exact;
        let hauling_extra_energy_usage = extra_haulers * base_hauler_body_energy_usage * body_cost_multiplier;
        let extra_haulers_hauling_throughput = hauling_extra_energy_usage * avg_source_spawn_dist;

        if let Some(eco_stats) = room_state.eco_stats.as_ref() {
            info!("Current creeps:");
            for &role in eco_stats.creep_stats_by_role.keys() {
                let role_stats = eco_stats.creep_stats(role);
                info!(
                    "* {}: {} creeps, {} with prespawned, max TTL {}, {} with prespawned",
                    role,
                    role_stats.number_of_active_creeps,
                    role_stats.number_of_creeps,
                    role_stats.max_active_creep_ttl,
                    role_stats.max_creep_ttl
                );
            }
        }
        info!("Spawn energy: {}/{}", spawn_energy, spawn_energy_capacity);
        info!("Energy income: {:.2}E/t", energy_income);
        info!("Energy usage allocation, hauling throughput required (incl. hauling costs) and body:");
        info!("* Mining:    {:.2}E/t (on {} creeps), {:.2}R/t, {}", total_mining_energy_usage, miners_required, mining_hauling_throughput, miner_body);
        info!("* Building:  {:.2}E/t ({:.2}E/t on {} creeps), {:.2}R/t, {}", total_building_energy_usage, building_creep_energy_usage, builders_required, building_hauling_throughput, builder_body);
        info!("* Upgrading: {:.2}E/t ({:.2}E/t on {} creeps), {:.2}R/t, {}", total_upgrading_energy_usage, upgrading_creep_energy_usage, upgraders_required, upgrading_hauling_throughput, upgrader_body);
        info!("* Hauling:   {:.2}E/t (on avg. {:.2} idle creeps), {:.2}R/t, {}", hauling_extra_energy_usage, extra_haulers, extra_haulers_hauling_throughput, hauler_body);
        info!("* Storage:   {:.2}E/t (on haulers), {:.2}R/t", storage_energy_usage, storage_hauling_throughput);
        info!("Haulers: {}", haulers_required);
        info!("Construction sites: {} (total {}E needed)", room_state.construction_site_queue.len(), total_construction_site_energy_needed);
        info!("Energy balance: {:.2}E/t", energy_balance);
        trace!("Body cost multiplier: {:.2}", body_cost_multiplier);

        Self {
            haulers_required,
            hauler_body,
            miners_required,
            miner_body,
            upgraders_required,
            upgrader_body,
            builders_required,
            builder_body,
        }
    }

    pub fn hauler_body(spawn_energy: u32) -> CreepBody {
        if spawn_energy == 0 {
            // Smallest possible hauler.
            vec![(Move, 1), (Carry, 1)].into()
        } else if spawn_energy >= 550 {
            vec![(Move, 5), (Carry, 5)].into()
        } else {
            vec![(Move, 3), (Carry, 3)].into()
        }
    }

    pub fn miner_body(spawn_energy: u32, drop_mining: bool) -> CreepBody {
        if spawn_energy == 0 {
            if drop_mining {
                // Smallest possible drop miner.
                vec![(Move, 1), (Work, 1)].into()
            } else {
                // Smallest possible link miner.
                vec![(Move, 1), (Work, 1), (Carry, 1)].into()
            }
        } else if spawn_energy >= 550 && drop_mining {
            vec![(Move, 1), (Work, 5)].into()
        } else if drop_mining {
            vec![(Move, 2), (Work, 2)].into()
        } else {
            vec![(Move, 1), (Work, 2), (Carry, 1)].into()
        }
    }

    pub fn upgrader_body(spawn_energy: u32) -> CreepBody {
        if spawn_energy >= 550 {
            vec![(Move, 2), (Work, 2), (Carry, 4)].into()
        } else {
            vec![(Move, 1), (Work, 1), (Carry, 2)].into()
        }
    }

    pub fn builder_body(spawn_energy: u32) -> CreepBody {
        if spawn_energy >= 550 {
            vec![(Move, 2), (Work, 3), (Carry, 1)].into()
        } else {
            vec![(Move, 1), (Work, 2), (Carry, 1)].into()
        }
    }
}