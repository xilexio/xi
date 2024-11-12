use std::cmp::max;
use std::iter::repeat;
use log::{info, trace};
use screeps::{ENERGY_REGEN_TIME, SOURCE_ENERGY_CAPACITY};
use screeps::Part::{Carry, Move, Work};
use screeps::StructureType::{Spawn, Storage};
use serde::{Deserialize, Serialize};
use crate::creeps::creep::CreepBody;
use crate::room_planning::room_planner::SOURCE_AND_CONTROLLER_ROAD_RCL;
use crate::room_states::room_state::RoomState;
use crate::u;

/// Structure containing parameters for the room economy that decide the distribution of resources
/// as well as composition of creeps.
#[derive(Debug, Deserialize, Serialize)]
pub struct RoomEcoConfig {
    /// The number of haulers that should be spawned at the moment.
    pub haulers_required: u32,
    /// The body of a hauler.
    pub hauler_body: CreepBody,

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

    /// Whether the room is out of essential creeps needed to sustain spawning other creeps and
    /// needs to be cold booted.
    pub cold_boot: bool,
}

impl RoomEcoConfig {
    // TODO Take previous room eco and other stats and adjust the values according to actual usage.
    // TODO Also include spawn occupation in the calculation, particularly at low RCL.
    pub fn new(room_state: &RoomState) -> Self {
        let spawn_energy = room_state.resources.spawn_energy_capacity;
        let spawn_energy_capacity = room_state.resources.spawn_energy_capacity;

        let number_of_sources = room_state.sources.len() as u32;

        let single_source_energy_income = SOURCE_ENERGY_CAPACITY as f32 / ENERGY_REGEN_TIME as f32;
        let energy_income = number_of_sources as f32 * single_source_energy_income;

        // Hauler uses energy only on its body.
        let hauler_body = Self::hauler_body(spawn_energy_capacity);
        let base_hauler_body_energy_usage = hauler_body.body_energy_usage();

        let roads_used = room_state.rcl >= SOURCE_AND_CONTROLLER_ROAD_RCL;

        let hauler_throughput = hauler_body.hauling_throughput(roads_used) / 2.0;

        let controller_work_pos = room_state.xy_to_pos(u!(u!(room_state.controller).work_xy));

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
                        let source_work_pos = room_state.xy_to_pos(u!(source_data.work_xy));
                        source_work_pos.get_range_to(spawn_pos) as f32
                    })
                    .sum::<f32>();
                avg_source_spawn_dist = spawn_distance_sum / room_state.sources.len() as f32;
            }
            // Multiplier of how much does a creep body cost including the cost of haulers.
            // Note that hauler energy usage should also be multiplied.
            body_cost_multiplier = 1.0 / (1.0 - avg_source_spawn_dist / hauler_throughput * base_hauler_body_energy_usage);

            let controller_distance_sum = room_state
                .sources
                .iter()
                .map(|source_data| {
                    let source_work_pos = room_state.xy_to_pos(u!(source_data.work_xy));
                    source_work_pos.get_range_to(controller_work_pos)
                })
                .sum::<u32>();
            avg_storage_controller_dist = controller_distance_sum as f32 / room_state.sources.len() as f32;
        }

        let mut energy_balance = energy_income;
        let mut total_hauling_throughput = 0f32;

        // Miner uses energy only on its body.
        let miner_body = Self::miner_body(spawn_energy_capacity);
        let miners_required = number_of_sources;
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

        // Computing the fraction of a hauler that remains unused. Also, adding some extra fraction
        // for safety buffer.
        let haulers_required_exact = total_hauling_throughput / hauler_throughput;
        let extra_haulers_exact = 0.5;
        let haulers_required = (haulers_required_exact + extra_haulers_exact).ceil() as u32;
        let extra_haulers = haulers_required as f32 - haulers_required_exact;
        let hauling_extra_energy_usage = extra_haulers * base_hauler_body_energy_usage * body_cost_multiplier;
        let extra_haulers_hauling_throughput = hauling_extra_energy_usage * avg_source_spawn_dist;

        info!("Energy income: {:.2}E/t", energy_income);
        info!("Energy usage allocation and hauling throughput required (incl. hauling costs):");
        info!("* Mining:    {:.2}E/t (on {} creeps), {:.2}R/t", total_mining_energy_usage, miners_required, mining_hauling_throughput);
        info!("* Building:  {:.2}E/t ({:.2}E/t on {} creeps), {:.2}R/t", total_building_energy_usage, building_creep_energy_usage, builders_required, building_hauling_throughput);
        info!("* Upgrading: {:.2}E/t ({:.2}E/t on {} creeps), {:.2}R/t", total_upgrading_energy_usage, upgrading_creep_energy_usage, upgraders_required, upgrading_hauling_throughput);
        info!("* Hauling:   {:.2}E/t (on avg. {:.2} idle creeps), {:.2}R/t", hauling_extra_energy_usage, extra_haulers, extra_haulers_hauling_throughput);
        info!("* Storage:   {:.2}E/t (on haulers), {:.2}R/t", storage_energy_usage, storage_hauling_throughput);
        info!("Haulers: {}", haulers_required);
        info!("Construction sites: {} (total {}E needed)", room_state.construction_site_queue.len(), total_construction_site_energy_needed);
        info!("Energy balance: {:.2}E/t", energy_balance);
        trace!("Body cost multiplier: {:.2}", body_cost_multiplier);

        Self {
            haulers_required,
            hauler_body,
            miner_body,
            upgraders_required,
            upgrader_body,
            builders_required,
            builder_body,
            cold_boot: false,
        }
    }

    pub fn hauler_body(spawn_energy: u32) -> CreepBody {
        let parts = if spawn_energy >= 550 {
            repeat([Carry, Move]).take(5).flatten().collect::<Vec<_>>()
        } else {
            vec![Carry, Move, Carry, Move, Carry, Move]
        };

        CreepBody::new(parts)
    }

    pub fn miner_body(spawn_energy: u32) -> CreepBody {
        let parts = if spawn_energy >= 550 {
            vec![Work, Work, Work, Work, Work, Move]
        } else {
            vec![Work, Work, Move, Move]
        };

        CreepBody::new(parts)
    }

    pub fn upgrader_body(spawn_energy: u32) -> CreepBody {
        let parts = if spawn_energy >= 550 {
            vec![Move, Carry, Carry, Carry, Work, Move, Carry, Work]
        } else {
            vec![Carry, Move, Carry, Work]
        };

        CreepBody::new(parts)
    }

    pub fn builder_body(spawn_energy: u32) -> CreepBody {
        let parts = if spawn_energy >= 550 {
            vec![Move, Move, Carry, Work, Work, Work]
        } else {
            vec![Move, Carry, Work, Work]
        };

        CreepBody::new(parts)
    }
}