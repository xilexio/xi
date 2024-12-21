use std::cmp::{max, min};
use log::info;
use screeps::{controller_downgrade, CREEP_LIFE_TIME, ENERGY_REGEN_TIME, SOURCE_ENERGY_CAPACITY};
use screeps::Part::{Carry, Move, Work};
use serde::{Deserialize, Serialize};
use crate::creeps::creep_body::CreepBody;
use crate::creeps::creep_role::CreepRole::{Builder, Hauler, Miner, Upgrader};
use crate::room_states::room_state::RoomState;
use crate::u;
use crate::utils::game_tick::game_tick;
use crate::utils::priority::Priority;

const DEBUG: bool = true;

const MIN_AVG_ENERGY_TO_SPARE: u32 = 200;

const MIN_SAFE_LAST_CREEP_TTL: u32 = 300;

// TODO Measure it instead.
const REPAIRER_EFFICIENCY: f32 = 0.75;

/// Structure containing parameters for the room economy that decide the distribution of resources
/// as well as composition of creeps.
#[derive(Debug, Deserialize, Serialize)]
pub struct RoomEcoConfig {
    /// The number of haulers that should be currently spawned.
    pub haulers_required: u32,
    /// The body of a hauler.
    pub hauler_body: CreepBody,
    pub hauler_spawn_priority: Priority,

    /// The number of miners that should be currently spawned.
    /// Miners are shared by all room sources.
    pub miners_required: u32,
    /// The body of a miner to spawn for each room source.
    pub miner_body: CreepBody,
    pub miner_spawn_priority: Priority,

    /// The number of upgraders to spawn.
    pub upgraders_required: u32,
    /// The body of an upgrader.
    pub upgrader_body: CreepBody,

    /// The number of builders to spawn.
    pub builders_required: u32,
    /// The body of a builder.
    pub builder_body: CreepBody,
    
    /// The number of repairers to spawn.
    pub repairers_required: u32,
    /// The body of a repairer.
    pub repairer_body: CreepBody,
}

pub fn update_or_create_eco_config(room_state: &mut RoomState) {
    let eco_stats = u!(room_state.eco_stats.as_ref());
    let miner_stats = eco_stats.creep_stats(Miner);
    let hauler_stats = eco_stats.creep_stats(Hauler);

    let spawn_energy = room_state.resources.spawn_energy;
    let spawn_energy_capacity = room_state.resources.spawn_energy_capacity;
    let haulable_energy = eco_stats.haul_stats.withdrawable_storage_amount.last() + eco_stats.haul_stats.unfulfilled_withdraw_amount.last();

    // TODO Handle link mining.
    // Computing minimal and preferred miner and hauler bodies.
    let min_miner_body = preferred_miner_body(0, true);
    let min_hauler_body = preferred_hauler_body(0);

    let hauler_body = preferred_hauler_body(spawn_energy_capacity);
    let miner_body = preferred_miner_body(spawn_energy_capacity, true);

    if room_state.eco_config.is_none() {
        // TODO Handle memory wipe from an already built up state better.
        room_state.eco_config = Some(RoomEcoConfig {
            haulers_required: 1,
            hauler_body: hauler_body.clone(),
            hauler_spawn_priority: Priority(200),
            miners_required: 1,
            miner_body: miner_body.clone(),
            miner_spawn_priority: Priority(200),
            upgraders_required: 0,
            upgrader_body: preferred_upgrader_body(spawn_energy),
            builders_required: 0,
            builder_body: preferred_builder_body(spawn_energy),
            repairers_required: 0,
            repairer_body: preferred_repairer_body(spawn_energy),
        });
    }

    let eco_config = u!(room_state.eco_config.as_mut());
    eco_config.hauler_body = hauler_body;
    eco_config.miner_body = miner_body;

    let number_of_sources = room_state.sources.len() as u32;
    let single_source_energy_income = SOURCE_ENERGY_CAPACITY / ENERGY_REGEN_TIME;

    // Checking if the room is in a condition where it cannot sustain itself.
    // The minimum is a single miner and a single hauler.
    // The hauler is a priority except for when the room has no energy income or storage,
    // then a miner needs to be spawned first.
    // TODO If there are unassigned miners available, try them first.
    let mut bootstrapping = true;
    if hauler_stats.number_of_creeps.last() == 0 {
        // Don't spawn anything else until the issue is resolved.
        eco_config.clear_non_miner_or_hauler();
        if miner_stats.number_of_creeps.last() == 0 {
            // TODO It might be not a good idea to include every single withdrawable energy, only
            //      one that can be transported to the spawn fast.
            if spawn_energy >= eco_config.hauler_body.energy_cost() && spawn_energy + haulable_energy >= eco_config.hauler_body.energy_cost() + min_miner_body.energy_cost() {
                // There is enough energy for a full hauler and then a miner (maybe after
                // transporting the energy). Spawn the hauler first. A smaller miner body may be
                // selected in the next iteration.
                eco_config.haulers_required = 1;
                eco_config.hauler_spawn_priority = Priority(250);

                eco_config.miners_required = 1;
                eco_config.miner_spawn_priority = Priority(200);
            } else {
                // We are bootstrapping from scratch. Spawn a miner first to start mining some
                // energy while the hauler is spawning. The miner should be as big as possible.
                eco_config.miners_required = 1;
                eco_config.miner_spawn_priority = Priority(250);
                // TODO Link mining.
                eco_config.miner_body = preferred_miner_body(spawn_energy - min_hauler_body.energy_cost(), true);

                eco_config.haulers_required = 1;
                eco_config.hauler_spawn_priority = Priority(200);
                eco_config.hauler_body = min_hauler_body;
            }
        } else {
            // There are miners available, so try to spawn a hauler using whatever energy is
            // currently available.
            eco_config.haulers_required = 1;
            eco_config.hauler_spawn_priority = Priority(250);
            eco_config.hauler_body = preferred_hauler_body(spawn_energy);

            eco_config.miners_required = 1;
            eco_config.miner_spawn_priority = Priority(200);
        }
    } else if miner_stats.number_of_creeps.last() == 0 {
        // Don't spawn anything else until the issue is resolved.
        eco_config.clear_non_miner_or_hauler();
        // There are haulers available, but not enough energy to spawn a preferred miner, so try
        // to spawn a miner using whatever energy is currently available in spawns and to haul
        // to them.
        eco_config.miners_required = 1;
        eco_config.miner_spawn_priority = Priority(250);
        eco_config.miner_body = preferred_miner_body(min(spawn_energy_capacity, spawn_energy + haulable_energy), true);
    } else {
        bootstrapping = false;

        // Setting the spawn priorities to normal.
        eco_config.hauler_spawn_priority = Priority(200);
        eco_config.miner_spawn_priority = Priority(200);

        // Setting the number of miners to optimal.
        eco_config.miners_required = single_source_energy_income.div_ceil(eco_config.miner_body.energy_harvest_power()) * number_of_sources;

        // There should always be at least two haulers.
        eco_config.haulers_required = max(2, eco_config.haulers_required);
        // TODO Remove.
        eco_config.haulers_required = max(4, eco_config.haulers_required);
    }

    // Energy to spare is decided by the amount in storage as well as the average unfulfilled
    // withdraw requests.
    let unfulfilled_haul_amount_balance = eco_stats.haul_stats.unfulfilled_withdraw_amount.small_sample_avg::<i32>()
        - eco_stats.haul_stats.unfulfilled_deposit_amount.small_sample_avg::<i32>();
    // TODO Check just energy, not everything.
    let has_energy_to_spare = unfulfilled_haul_amount_balance > MIN_AVG_ENERGY_TO_SPARE as i32;

    let ticks_to_downgrade = u!(room_state.controller).downgrade_tick - game_tick();
    let max_ticks_to_downgrade = u!(controller_downgrade(room_state.rcl));
    // TODO Once everything is built, it should be kept close to fully upgraded.
    //      On RCL 5-7, it should be kept rather high, but building should also take place.
    //      On RCL 4 and lower, it's sufficient to just barely keep it from downgrading.
    let controller_downgrade_level_critical = ticks_to_downgrade < max_ticks_to_downgrade / 4;

    if !bootstrapping {
        // Increasing or decreasing the required number of haulers depending on the average amount of
        // resources to carry in unfulfilled requests as well as the number if idle haulers.
        // TODO Handle large sample or preliminary calculations if needed.
        let unfulfilled_fulfillable_haul_amount = max(
            min(
                eco_stats.haul_stats.unfulfilled_withdraw_amount.small_sample_avg::<u32>(),
                eco_stats.haul_stats.unfulfilled_deposit_amount.small_sample_avg::<u32>()
                    + eco_stats.haul_stats.depositable_storage_amount.small_sample_avg::<u32>()
            ),
            min(
                eco_stats.haul_stats.unfulfilled_deposit_amount.small_sample_avg::<u32>(),
                eco_stats.haul_stats.unfulfilled_withdraw_amount.small_sample_avg::<u32>()
                    + eco_stats.haul_stats.withdrawable_storage_amount.small_sample_avg::<u32>()
            )
        );

        // TODO Possibly also check if there is energy to spare.
        // TODO If there is a lot of energy in decaying piles, but also no storage, spawn more
        //      haulers to contain it.
        if eco_config.haulers_required > 2 && hauler_stats.number_of_idle_creeps.small_sample_avg::<f32>() >= 1.5 {
            // If there is at least 1.5 idle hauler on average, decrease the number of required haulers.
            eco_config.haulers_required -= 1;
        } else if hauler_stats.number_of_idle_creeps.small_sample_avg::<f32>() < 0.5 && unfulfilled_fulfillable_haul_amount > eco_config.hauler_body.store_capacity() / 2 {
            // If there are usually no idle haulers and there is on average more to carry than half
            // of a hauler capacity, increase the number of haulers.
            eco_config.haulers_required += 1;
        }

        // If there are construction sites, spawn builders.
        // TODO Also make the calculations based on various storage, especially when the main storage
        //      is built.
        if controller_downgrade_level_critical || room_state.construction_site_queue.is_empty() {
            // No need for builders if there are no construction sites.
            eco_config.builders_required = 0;
        } else {
            let builder_stats = eco_stats.creep_stats(Builder);
            if eco_config.builders_required > 1 && builder_stats.number_of_idle_creeps.small_sample_avg::<f32>() >= 1.5 {
                // If at least 1.5 builders are idle on average, decrease their number.
                eco_config.builders_required -= 1;
            } else if has_energy_to_spare {
                // If there are construction sites and energy to spare, spawn more builders.
                // However, don't spawn more builders if some of them are idle (i.e., starved for
                // energy).
                if eco_config.builders_required == 0 || eco_config.builders_required == builder_stats.number_of_active_creeps.last() && builder_stats.number_of_idle_creeps.small_sample_avg::<f32>() < 0.5 {
                    eco_config.builders_required += 1;
                }
            }

            if eco_config.builders_required > 0 {
                eco_config.builder_body = preferred_builder_body(spawn_energy);
            }
        }

        // If there is enough energy to spare, spawn upgraders. They have smaller priority than
        // builders. However, if the controller is close to downgrading, prioritize the upgrader.
        // TODO Spawning a single upgrader should have higher priority when the controller is
        //      critical.
        if !room_state.construction_site_queue.is_empty() && !controller_downgrade_level_critical {
            eco_config.upgraders_required = 0;
        } else {
            let upgrader_stats = eco_stats.creep_stats(Upgrader);
            if eco_config.upgraders_required > 1 && upgrader_stats.number_of_idle_creeps.small_sample_avg::<f32>() >= 1.5 {
                // If at least 1.5 upgraders are idle on average, decrease their number.
                eco_config.upgraders_required -= 1;
            } else if has_energy_to_spare || controller_downgrade_level_critical {
                // If there is energy to spare, spawn more upgraders.
                // However, don't spawn more builders if some of them are idle (i.e., starved for
                // energy).
                if eco_config.upgraders_required == 0 || eco_config.upgraders_required == upgrader_stats.number_of_active_creeps.last() &&upgrader_stats.number_of_idle_creeps.small_sample_avg::<f32>() < 0.5 {
                    eco_config.upgraders_required += 1;
                }
            }

            if eco_config.upgraders_required > 0 {
                eco_config.upgrader_body = preferred_upgrader_body(spawn_energy);
            }
        }
        
        // TODO Include in energy calculations. Prioritize over building. Prioritize over upgrading if critical unless controller also critical.
        let single_repairer_total_repairer_hits = ((eco_config.repairer_body.repair_power() * CREEP_LIFE_TIME) as f32 * REPAIRER_EFFICIENCY) as u32;
        let repairer_required = !room_state.triaged_repair_sites.critical.is_empty() || room_state.triaged_repair_sites.total_hits_to_repair >= single_repairer_total_repairer_hits;
        eco_config.repairers_required = repairer_required as u32;
    }

    if DEBUG {
        info!("Average haul stats / small sample haul stats / current haul stats:");
        info!(
            "Unfulfilled withdraw amount: {:.2}R, {:.2}R, {}R",
            eco_stats.haul_stats.unfulfilled_withdraw_amount.avg::<f32>(),
            eco_stats.haul_stats.unfulfilled_withdraw_amount.small_sample_avg::<f32>(),
            eco_stats.haul_stats.unfulfilled_withdraw_amount.last()
        );
        info!(
            "Unfulfilled deposit amount: {:.2}R, {:.2}R, {}R",
            eco_stats.haul_stats.unfulfilled_deposit_amount.avg::<f32>(),
            eco_stats.haul_stats.unfulfilled_deposit_amount.small_sample_avg::<f32>(),
            eco_stats.haul_stats.unfulfilled_deposit_amount.last()
        );
        info!(
            "Withdrawable storage amount: {:.2}R, {:.2}R, {}R",
            eco_stats.haul_stats.withdrawable_storage_amount.avg::<f32>(),
            eco_stats.haul_stats.withdrawable_storage_amount.small_sample_avg::<f32>(),
            eco_stats.haul_stats.withdrawable_storage_amount.last()
        );
        info!(
            "Depositable storage amount: {:.2}R, {:.2}R, {}R",
            eco_stats.haul_stats.depositable_storage_amount.avg::<f32>(),
            eco_stats.haul_stats.depositable_storage_amount.small_sample_avg::<f32>(),
            eco_stats.haul_stats.depositable_storage_amount.last()
        );
        info!("Creep stats:");
        for (role, role_stats) in eco_stats.creep_stats_by_role.iter() {
            info!(
                "* {}: {} creeps, {} with prespawned, max TTL {}, {} with prespawned, {:.2} idle ({:.2} avg.)",
                role,
                role_stats.number_of_active_creeps.last(),
                role_stats.number_of_creeps.last(),
                role_stats.max_active_creep_ttl.last(),
                role_stats.max_creep_ttl.last(),
                role_stats.number_of_idle_creeps.last(),
                role_stats.number_of_idle_creeps.small_sample_avg::<f32>()
            );
        }

        let energy_income = number_of_sources as f32 * single_source_energy_income as f32;

        let hauling_body_energy_usage = eco_config.haulers_required as f32 * eco_config.hauler_body.body_energy_usage();
        let mining_body_energy_usage = eco_config.miners_required as f32 * eco_config.miner_body.body_energy_usage();
        let building_body_energy_usage = eco_config.builders_required as f32 * eco_config.builder_body.body_energy_usage();
        let upgrading_body_energy_usage = eco_config.upgraders_required as f32 * eco_config.upgrader_body.body_energy_usage();

        let body_energy_usage = hauling_body_energy_usage + mining_body_energy_usage + building_body_energy_usage + upgrading_body_energy_usage;
        let building_work_energy_usage = eco_config.builders_required as f32 * eco_config.builder_body.build_energy_usage() as f32;
        let upgrading_work_energy_usage = eco_config.upgraders_required as f32 * eco_config.upgrader_body.upgrade_energy_usage() as f32;
        let work_energy_usage = building_work_energy_usage + upgrading_work_energy_usage;
        let energy_usage = body_energy_usage + work_energy_usage;

        let total_construction_site_energy_needed: u32 = room_state
            .construction_site_queue
            .iter().map(|cs| u!(cs.structure_type.construction_cost()))
            .sum();

        info!("Bootstrapping: {}, Energy to spare: {}, Controller critical: {} ({}/{})", bootstrapping, has_energy_to_spare, controller_downgrade_level_critical, ticks_to_downgrade, max_ticks_to_downgrade);
        info!("Spawn energy: {}/{}", spawn_energy, spawn_energy_capacity);
        info!("Energy income: {:.2}E/t", energy_income);
        info!("Predicted energy usage and other stats:");
        info!("* Hauling:   {:.2}E/t on {} creeps, {}", hauling_body_energy_usage, eco_config.haulers_required, eco_config.hauler_body);
        info!("* Mining:    {:.2}E/t on {} creeps, {}", mining_body_energy_usage, eco_config.miners_required, eco_config.miner_body);
        info!("* Building:  {:.2}E/t on {} creeps + {:.2}E/t on work, {}", building_body_energy_usage, eco_config.builders_required, building_work_energy_usage, eco_config.builder_body);
        info!("* Upgrading: {:.2}E/t on {} creeps + {:.2}E/t on work, {}", upgrading_body_energy_usage, eco_config.upgraders_required, upgrading_work_energy_usage, eco_config.upgrader_body);
        info!("Construction sites: {} (total {}E needed)", room_state.construction_site_queue.len(), total_construction_site_energy_needed);
        info!("Energy usage: {:.2}E/t + {:.2}E/t = {:.2}E/t", body_energy_usage, work_energy_usage, energy_usage);
        info!("Energy balance: {:.2}E/t", energy_income - energy_usage)
    }
}

impl RoomEcoConfig {
    pub fn clear_non_miner_or_hauler(&mut self) {
        self.upgraders_required = 0;
        self.builders_required = 0;
    }

    /*
    pub fn new(room_state: &RoomState) -> Self {
        let eco_stats = u!(room_state.eco_stats.as_ref());
        let miner_stats = eco_stats.creep_stats(Miner);
        let hauler_stats = eco_stats.creep_stats(Hauler);

        let spawn_energy = room_state.resources.spawn_energy;
        let spawn_energy_capacity = room_state.resources.spawn_energy_capacity;

        let number_of_sources = room_state.sources.len() as u32;

        let single_source_energy_income = SOURCE_ENERGY_CAPACITY as f32 / ENERGY_REGEN_TIME as f32;
        let energy_income = number_of_sources as f32 * single_source_energy_income;

        // Hauler uses energy only on its body.
        let mut hauler_body = preferred_hauler_body(spawn_energy_capacity);
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
        let mut miner_body = preferred_miner_body(spawn_energy_capacity, true);
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
            miner_body = preferred_miner_body(0, true);
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

        let mut miner_spawn_priority = Priority(200);
        let mut hauler_spawn_priority = Priority(150);
        if miner_stats.number_of_creeps == 0 {
            miner_spawn_priority = Priority(250);
        } else if hauler_stats.number_of_creeps == 0 {
            hauler_spawn_priority = Priority(250);
        }

        // Builder uses energy on its body and building.
        // Ignoring the time spent travelling for the purpose of energy usage.
        // This is only a one-time cost, so when storage is available, there is no real limit on
        // the number of builders as long as all are used and there is enough spawn capacity.
        let builder_body = preferred_builder_body(spawn_energy_capacity);
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
        let upgrader_body = preferred_upgrader_body(spawn_energy_capacity);
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
            hauler_body = preferred_hauler_body(0);
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
            info!("Average haul stats / small sample haul stats / current haul stats:");
            info!(
                "Unfulfilled withdraw amount: {:.2}R, {:.2}R, {}R",
                eco_stats.haul_stats.unfulfilled_withdraw_amount.avg::<f32>(),
                eco_stats.haul_stats.unfulfilled_withdraw_amount.small_sample_avg::<f32>(),
                eco_stats.haul_stats.unfulfilled_withdraw_amount.last()
            );
            info!(
                "Unfulfilled deposit amount: {:.2}R, {:.2}R, {}R",
                eco_stats.haul_stats.unfulfilled_deposit_amount.avg::<f32>(),
                eco_stats.haul_stats.unfulfilled_deposit_amount.small_sample_avg::<f32>(),
                eco_stats.haul_stats.unfulfilled_deposit_amount.last()
            );
            info!(
                "Withdrawable storage amount: {:.2}R, {:.2}R, {}R",
                eco_stats.haul_stats.withdrawable_storage_amount.avg::<f32>(),
                eco_stats.haul_stats.withdrawable_storage_amount.small_sample_avg::<f32>(),
                eco_stats.haul_stats.withdrawable_storage_amount.last()
            );
            info!(
                "Depositable storage amount: {:.2}R, {:.2}R, {}R",
                eco_stats.haul_stats.depositable_storage_amount.avg::<f32>(),
                eco_stats.haul_stats.depositable_storage_amount.small_sample_avg::<f32>(),
                eco_stats.haul_stats.depositable_storage_amount.last()
            );
            info!(
                "Idle haulers: {:.2}, {:.2}, {}",
                eco_stats.haul_stats.idle_haulers.avg::<f32>(),
                eco_stats.haul_stats.idle_haulers.small_sample_avg::<f32>(),
                eco_stats.haul_stats.idle_haulers.last()
            );
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
            hauler_spawn_priority,
            miners_required,
            miner_body,
            miner_spawn_priority,
            upgraders_required,
            upgrader_body,
            builders_required,
            builder_body,
        }
    }
     */
}

pub fn preferred_hauler_body(spawn_energy: u32) -> CreepBody {
    if spawn_energy >= 550 {
        vec![(Move, 5), (Carry, 5)].into()
    } else if spawn_energy >= 300 {
        vec![(Move, 3), (Carry, 3)].into()
    } else {
        // Smallest possible hauler.
        vec![(Move, 1), (Carry, 1)].into()
    }
}

pub fn preferred_miner_body(spawn_energy: u32, drop_mining: bool) -> CreepBody {
    if drop_mining {
        preferred_drop_miner_body(spawn_energy)
    } else {
        preferred_link_miner_body(spawn_energy)
    }
}

pub fn preferred_drop_miner_body(spawn_energy: u32) -> CreepBody {
    if spawn_energy >= 550 {
        vec![(Move, 1), (Work, 5)].into()
    } else if spawn_energy >= 400 {
        vec![(Move, 2), (Work, 3)].into()
    } else if spawn_energy >= 250 {
        vec![(Move, 1), (Work, 2)].into()
    } else {
        // Smallest possible drop miner.
        vec![(Move, 1), (Work, 1)].into()
    }
}

pub fn preferred_link_miner_body(spawn_energy: u32) -> CreepBody {
    if spawn_energy >= 300 {
        vec![(Move, 1), (Work, 2), (Carry, 1)].into()
    } else {
        // Smallest possible link miner.
        vec![(Move, 1), (Work, 1), (Carry, 1)].into()
    }
}

pub fn preferred_upgrader_body(spawn_energy: u32) -> CreepBody {
    if spawn_energy >= 550 {
        vec![(Move, 2), (Work, 2), (Carry, 4)].into()
    } else {
        vec![(Move, 1), (Work, 1), (Carry, 2)].into()
    }
}

pub fn preferred_builder_body(spawn_energy: u32) -> CreepBody {
    if spawn_energy >= 450 {
        vec![(Move, 1), (Work, 2), (Carry, 4)].into()
    } else if spawn_energy >= 400 {
        vec![(Move, 1), (Work, 2), (Carry, 3)].into()
    } else {
        vec![(Move, 1), (Work, 1), (Carry, 3)].into()
    }
}

pub fn preferred_repairer_body(spawn_energy: u32) -> CreepBody {
    if spawn_energy >= 450 {
        vec![(Move, 3), (Work, 2), (Carry, 4)].into()
    } else if spawn_energy >= 400 {
        vec![(Move, 2), (Work, 2), (Carry, 2)].into()
    } else {
        vec![(Move, 1), (Work, 1), (Carry, 1)].into()
    }
}