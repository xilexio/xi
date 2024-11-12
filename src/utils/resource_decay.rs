use screeps::ENERGY_DECAY;

pub fn decay_per_tick(amount: u32) -> u32 {
    amount.div_ceil(ENERGY_DECAY)
}