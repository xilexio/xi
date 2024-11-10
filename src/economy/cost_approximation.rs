use log::debug;
use screeps::Part::{Carry, Move, Work};
use screeps::{
    RoomName, CARRY_CAPACITY, CONSTRUCTION_COST_ROAD_SWAMP_RATIO, CONSTRUCTION_COST_ROAD_WALL_RATIO, CONTAINER_DECAY,
    CONTAINER_DECAY_TIME_OWNED, CREEP_LIFE_TIME, ENERGY_REGEN_TIME, EXTRACTOR_COOLDOWN, HARVEST_MINERAL_POWER,
    HARVEST_POWER, LAB_REACTION_AMOUNT, LINK_CAPACITY, LINK_LOSS_RATIO, MINERAL_REGEN_TIME,
    RAMPART_DECAY_AMOUNT, REPAIR_POWER, ROAD_DECAY_AMOUNT, ROAD_DECAY_TIME, SOURCE_ENERGY_CAPACITY, INTENT_CPU_COST,
};

const FAST_FILLER_CARRY: [u32; 4] = [18, 4, 4, 6];
// Includes intents for withdraw from storage, putting into container and each of 4 creeps filling some
// extensions or spawns.
const FAST_FILLER_INTENTS_PER_ENERGY: f32 = (8 + 3 + 8 + 8 + 10) as f32 / ((12 * 200 + 3 * 300) as f32);

const SOURCE_ENERGY_PER_TICK: f32 = SOURCE_ENERGY_CAPACITY as f32 / ENERGY_REGEN_TIME as f32;

const AVERAGE_MINERAL_DENSITY: f32 = 15_000.0 * 0.1 + 35_000.0 * 0.4 + 70_000.0 * 0.4 + 100_000.0 * 0.1;

pub fn energy_balance_and_cpu_cost(
    room_name: RoomName,
    source_distances: Vec<u8>,
    mineral_distance: u8,
    controller_distance: u8,
    plain_roads_count: u32,
    plain_roads_avg_dist: f32,
    swamp_roads_count: u32,
    swamp_roads_avg_dist: f32,
    wall_roads_count: u32,
    wall_roads_avg_dist: f32,
    rampart_count: u32,
    container_count: u32,
) -> (f32, f32) {
    let source_energy_per_tick = SOURCE_ENERGY_PER_TICK * (source_distances.len() as f32);

    // Source mining.
    let miner_work = 12;
    let miner_carry = 4;
    let miner_move = 3;
    let miner_energy_cost = miner_work * Work.cost() + miner_carry * Carry.cost() + miner_move + Move.cost();
    let miner_speed = miner_work / (2 * miner_move);
    let link_energy_cost_per_tick = source_energy_per_tick * LINK_LOSS_RATIO;

    let mut miners_energy_cost_per_tick = 0.0;
    let mut miners_travel_intents_per_tick = 0.0;
    let mut miners_spawn_intents_per_tick = 0.0;
    for &dist in source_distances.iter() {
        let miner_mining_ticks = CREEP_LIFE_TIME as f32 - miner_speed as f32 * dist as f32;
        miners_energy_cost_per_tick += miner_energy_cost as f32 / miner_mining_ticks;
        miners_travel_intents_per_tick += dist as f32 / miner_mining_ticks;
        miners_spawn_intents_per_tick += spawn_intent_cost(miner_energy_cost) / miner_mining_ticks;
    }
    let miners_harvest_intents_per_tick = source_distances.len() as f32
        * (SOURCE_ENERGY_CAPACITY as f32 / (miner_work * HARVEST_POWER) as f32).ceil()
        / ENERGY_REGEN_TIME as f32;
    let miner_capacity = miner_carry * CARRY_CAPACITY;
    let link_store_intents_per_tick =
        miners_harvest_intents_per_tick * ((miner_work * HARVEST_POWER) as f32 / miner_capacity as f32).ceil();
    // This is an approximation.
    let link_send_intents_per_tick = source_distances.len() as f32 * SOURCE_ENERGY_PER_TICK / LINK_CAPACITY as f32;

    let mining_energy_cost_per_tick = miners_energy_cost_per_tick + link_energy_cost_per_tick;
    let mining_intents_per_tick = miners_travel_intents_per_tick
        + miners_spawn_intents_per_tick
        + miners_harvest_intents_per_tick
        + link_store_intents_per_tick
        + link_send_intents_per_tick;

    // Mineral mining.
    // These computations are approximate as the specifics (such as number of required miner creeps) vary depending on density.
    let average_mineral_amount = AVERAGE_MINERAL_DENSITY;
    let mineral_miner_work = 40;
    let mineral_miner_move = 10;
    let mineral_miner_energy_cost = mineral_miner_work * Work.cost() + mineral_miner_move * Move.cost();
    let mineral_miner_speed = mineral_miner_work / (2 * mineral_miner_move);
    let mining_time = CREEP_LIFE_TIME as f32 - mineral_miner_speed as f32 * mineral_distance as f32;
    let total_extractions = mining_time as f32 / (1 + EXTRACTOR_COOLDOWN) as f32;
    let extractions_required = AVERAGE_MINERAL_DENSITY / (mineral_miner_work as f32 * HARVEST_MINERAL_POWER as f32);
    // This equals 1 unless the mineral is very far away.
    let number_of_miners_per_regen = (extractions_required / total_extractions).ceil();
    let mineral_miner_energy_cost_per_tick =
        mineral_miner_energy_cost as f32 * number_of_miners_per_regen / MINERAL_REGEN_TIME as f32;
    // TODO CPU

    // Fast filler creeps.
    let ff_energy_cost_per_tick =
        FAST_FILLER_CARRY.iter().sum::<u32>() as f32 * Carry.cost() as f32 / CREEP_LIFE_TIME as f32;
    let ff_intents_per_tick = FAST_FILLER_CARRY
        .iter()
        .map(|&carry| spawn_intent_cost(carry * Carry.cost()))
        .sum::<f32>()
        / CREEP_LIFE_TIME as f32;

    // Controller upgrade.
    // We only take into account energy used on the upgrader itself, as upgrading an RCL8 room goes into a shared
    // resource, GCL, not economy of this room.
    // TODO energy + CPU cost, assume 15/tick at RCL8 with link
    let upgrader_work = 15;
    let upgrader_carry = 4;
    let upgrader_move = 4;
    let upgrader_speed = upgrader_work / (2 * upgrader_move);
    let upgrading_time = CREEP_LIFE_TIME as f32 - upgrader_speed as f32 * controller_distance as f32;
    let upgrader_energy_cost =
        upgrader_work * Work.cost() + upgrader_carry * Carry.cost() + upgrader_move * Move.cost();
    let upgrader_energy_cost_per_tick = upgrader_energy_cost as f32 / upgrading_time;

    // A single hauler.
    let hauler_carry = 16;
    let hauler_move = 8;
    let hauler_energy_cost = hauler_carry * Carry.cost() + hauler_move * Move.cost();
    let hauler_energy_cost_per_tick = hauler_energy_cost as f32 / CREEP_LIFE_TIME as f32;
    // TODO intents - assume at least one round to all extensions for the mineral miner and then to labs and to collect mineral, maybe separately

    // Labs and hauling to them.
    let lab_cooldown = 10;
    let output_labs_count = 8;
    let reactions_per_tick = 1.0 / (1 + lab_cooldown) as f32;
    let lab_reacted_amount_per_tick = output_labs_count as f32 * LAB_REACTION_AMOUNT as f32 * reactions_per_tick;
    // TODO hauling intents

    // Factory.
    // TODO only intents

    // Power processing.
    // TODO only intents.

    // Maintainer creep.
    let maintainer_work = 16;
    let maintainer_carry = 16;
    let maintainer_move = 8;
    let maintainer_energy_cost =
        maintainer_work * Work.cost() + maintainer_carry * Carry.cost() + maintainer_move * Move.cost();
    let maintainer_speed = maintainer_work / (2 * maintainer_move);
    let maintainer_energy_cost_per_tick = (maintainer_energy_cost * maintainer_speed) as f32 / CREEP_LIFE_TIME as f32;
    // We assume that there is always something unrepaired in range, so the maintainer does not waste any life time.
    let repair_cost =
        (maintainer_energy_cost_per_tick + maintainer_work as f32) / (maintainer_work * REPAIR_POWER) as f32;

    // Road maintenance.
    let roads_count = [plain_roads_count, swamp_roads_count, wall_roads_count];
    let multipliers = [1, CONSTRUCTION_COST_ROAD_SWAMP_RATIO, CONSTRUCTION_COST_ROAD_WALL_RATIO];
    let avg_distances = [plain_roads_avg_dist, swamp_roads_avg_dist, wall_roads_avg_dist];
    let mut road_maintenance_energy_cost_per_tick = 0.0;
    for (&count, &mul) in roads_count
        .iter()
        .zip(multipliers.iter())
    {
        road_maintenance_energy_cost_per_tick += (count * mul * ROAD_DECAY_AMOUNT) as f32 / ROAD_DECAY_TIME as f32 * repair_cost;
    }
    // TODO CPU

    // Rampart maintenance.
    let rampart_maintenance_energy_cost_per_tick =
        (rampart_count * RAMPART_DECAY_AMOUNT) as f32 / RAMPART_DECAY_AMOUNT as f32 * repair_cost;
    // TODO CPU

    // Container maintenance.
    let container_maintenance_energy_cost_per_tick =
        (container_count * CONTAINER_DECAY) as f32 / CONTAINER_DECAY_TIME_OWNED as f32 * repair_cost;
    // TODO CPU

    let total_energy_balance = source_energy_per_tick
        - mining_energy_cost_per_tick
        - ff_energy_cost_per_tick
        - road_maintenance_energy_cost_per_tick
        - rampart_maintenance_energy_cost_per_tick
        - container_maintenance_energy_cost_per_tick
        - mineral_miner_energy_cost_per_tick
        - hauler_energy_cost_per_tick
        - upgrader_energy_cost_per_tick;
    let total_intents_per_tick = mining_intents_per_tick + ff_intents_per_tick;

    debug!(
        "Approximate energy balance and CPU cost for room {}:\n\
    \n\
    Energy balance:\n\
    * income: +{}\n\
    * source mining: -{}\n\
    * mineral mining: -{}\n\
    * fast filler creeps: -{}\n\
    * hauler: -{}\n\
    * upgrader: -{}\n\
    * rampart maintenance: -{}\n\
    * road maintenance: -{}\n\
    * container maintenance: -{}\n\
    Total: {}\n\
    \n\
    CPU cost:\n\
    * mining: {}I / {}CPU\n\
    * fast filler creeps: {}I / {}CPU\n\
    Total: {}I / {}CPU\
    \n\
    Energy efficiency: {}E/CPU",
        room_name,
        source_energy_per_tick,
        mining_energy_cost_per_tick,
        mineral_miner_energy_cost_per_tick,
        ff_energy_cost_per_tick,
        hauler_energy_cost_per_tick,
        upgrader_energy_cost_per_tick,
        rampart_maintenance_energy_cost_per_tick,
        road_maintenance_energy_cost_per_tick,
        container_maintenance_energy_cost_per_tick,
        total_energy_balance,
        mining_intents_per_tick,
        mining_intents_per_tick * INTENT_CPU_COST as f32,
        ff_intents_per_tick,
        ff_intents_per_tick * INTENT_CPU_COST as f32,
        total_intents_per_tick,
        total_intents_per_tick * INTENT_CPU_COST as f32,
        total_energy_balance / (total_intents_per_tick * INTENT_CPU_COST as f32)
    );

    (total_energy_balance, total_intents_per_tick * INTENT_CPU_COST as f32)
}

#[inline]
pub fn spawn_intent_cost(energy_cost: u32) -> f32 {
    1.0 + energy_cost as f32 * FAST_FILLER_INTENTS_PER_ENERGY
}