use std::cmp::max;
use rustc_hash::FxHashMap;
use crate::creeps::creep_role::CreepRole;
use crate::hauling::haul_stats::HaulStats;
use crate::spawning::spawn_pool::WId;

/// A structure gathering energy, transportation throughput and other statistics to decide on
/// the distribution of resources in the room, e.g., on the number of haulers, upgraders, etc.
#[derive(Debug, Default)]
pub struct RoomEcoStats {
    /// Statistics for creeps in the room, separately for each creep role and spawn pool.
    pub creep_stats_by_role: FxHashMap<CreepRole, FxHashMap<WId, RoomCreepStats>>,
    
    /// Statistics about amount of resources in haul requests in the room.
    pub haul_stats: HaulStats,
}

#[derive(Debug, Default)]
pub struct RoomCreepStats {
    /// Number of active creeps, i.e., creeps that are spawned and executing their future.
    pub number_of_active_creeps: u32,
    /// Number of creeps, including already spawned prespawned ones.
    pub number_of_creeps: u32,
    /// Maximum TTL of active creeps.
    pub max_active_creep_ttl: u32,
    /// Maximum TTL of active creeps and already spawned prespawned creeps.
    pub max_creep_ttl: u32,
}

impl RoomCreepStats {
    pub fn add(&mut self, other: &RoomCreepStats) {
        self.number_of_active_creeps += other.number_of_active_creeps;
        self.number_of_creeps += other.number_of_creeps;
        self.max_active_creep_ttl = max(self.max_active_creep_ttl, other.max_active_creep_ttl);
        self.max_creep_ttl = max(self.max_creep_ttl, other.max_creep_ttl);
    }
}

impl RoomEcoStats {
    pub fn creep_stats(&self, role: CreepRole) -> RoomCreepStats {
        let mut total_stats = RoomCreepStats::default();
        
        if let Some(role_stats) = self.creep_stats_by_role.get(&role) {
            for creep_stats in role_stats.values() {
                total_stats.add(creep_stats);
            }
        }
        
        total_stats
    }
}