use std::cmp::{max, min};
use screeps::{TOWER_FALLOFF, TOWER_FALLOFF_RANGE, TOWER_OPTIMAL_RANGE, TOWER_POWER_ATTACK};

pub fn tower_attack_power(dist: u8) -> u32 {
    let effective_dist = max(TOWER_OPTIMAL_RANGE, min(TOWER_FALLOFF_RANGE, dist as u32));
    TOWER_POWER_ATTACK - ((TOWER_POWER_ATTACK as f32 * TOWER_FALLOFF) as u32) * (effective_dist - TOWER_OPTIMAL_RANGE) / (TOWER_FALLOFF_RANGE - TOWER_OPTIMAL_RANGE)
}