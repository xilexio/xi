use std::cmp::max;
use enum_iterator::all;
use rustc_hash::FxHashMap;
use crate::utils::avg_vector::AvgVector;
use crate::creeps::creep_role::CreepRole;
use crate::hauling::haul_stats::HaulStats;
use crate::spawning::spawn_pool::WId;
use crate::{local_debug, u};
use crate::creeps::creeps::CreepRef;
use crate::utils::game_tick::{first_tick, game_tick};

const DEBUG: bool = true;

/// A structure gathering energy, transportation throughput and other statistics to decide on
/// the distribution of resources in the room, e.g., on the number of haulers, upgraders, etc.
// TODO Custom default that creates empty creep_stats_by_role instead of relying on timings.
#[derive(Debug, Default)]
pub struct RoomEcoStats {
    /// Statistics for spawn pools in the room. Used as a base for creep stats by role and without
    /// history.
    pub spawn_pool_stats: FxHashMap<WId, SpawnPoolStats>,

    /// The number of idle creeps since last sampling by role.
    number_of_idle_creeps: FxHashMap<CreepRole, u32>,

    /// Statistics for creeps by role.
    pub creep_stats_by_role: FxHashMap<CreepRole, RoomCreepStats>,

    /// Tick in which last sampling took place.
    creep_stats_by_role_sample_tick: u32,
    
    /// Statistics about amount of resources in haul requests in the room.
    pub haul_stats: HaulStats,
}

#[derive(Debug, Default)]
pub struct SpawnPoolStats {
    pub creep_role: CreepRole,
    /// Number of active creeps, i.e., creeps that are spawned and executing their future.
    pub number_of_active_creeps: u32,
    /// Number of creeps, including already spawned prespawned ones.
    pub number_of_creeps: u32,
    /// Maximum TTL of active creeps.
    pub max_active_creep_ttl: u32,
    /// Maximum TTL of active creeps and already spawned prespawned creeps.
    pub max_creep_ttl: u32,
}

impl SpawnPoolStats {
    pub fn new(creep_role: CreepRole) -> Self {
        Self {
            creep_role,
            ..SpawnPoolStats::default()
        }
    }

    pub fn add_assign(&mut self, other: &SpawnPoolStats) {
        self.number_of_active_creeps += other.number_of_active_creeps;
        self.number_of_creeps += other.number_of_creeps;
        self.max_active_creep_ttl = max(self.max_active_creep_ttl, other.max_active_creep_ttl);
        self.max_creep_ttl = max(self.max_creep_ttl, other.max_creep_ttl);
    }
}

#[derive(Debug, Default)]
pub struct RoomCreepStats {
    /// Number of active creeps, i.e., creeps that are spawned and executing their future.
    pub number_of_active_creeps: AvgVector<u32>,
    /// Number of creeps, including already spawned prespawned ones.
    pub number_of_creeps: AvgVector<u32>,
    /// Maximum TTL of active creeps.
    pub max_active_creep_ttl: AvgVector<u32>,
    /// Maximum TTL of active creeps and already spawned prespawned creeps.
    pub max_creep_ttl: AvgVector<u32>,
    /// Number of idle creeps.
    pub number_of_idle_creeps: AvgVector<u32>,
    // TODO Also unassigned creeps.
}

impl RoomEcoStats {
    pub fn register_idle_creep(&mut self, role: CreepRole, creep_ref: &CreepRef) {
        local_debug!("Creep {} is idle.", creep_ref.borrow().name);
        *self.number_of_idle_creeps.entry(role).or_default() += 1;
    }

    pub fn push_creep_stats_samples(&mut self) {
        let mut creep_stats: FxHashMap<CreepRole, SpawnPoolStats> = FxHashMap::default();

        for spawn_pool_stats in self.spawn_pool_stats.values() {
            creep_stats.entry(spawn_pool_stats.creep_role).or_default().add_assign(spawn_pool_stats);
        }

        let ticks_since_last_sample = game_tick() - max(self.creep_stats_by_role_sample_tick, first_tick());

        for role in all::<CreepRole>() {
            let creep_role_stats = self
                .creep_stats_by_role
                .entry(role)
                .or_default();

            creep_role_stats.number_of_active_creeps.push(creep_stats
                .get(&role)
                .map_or(0, |stats| stats.number_of_active_creeps));
            creep_role_stats.number_of_creeps.push(creep_stats
                .get(&role)
                .map_or(0, |stats| stats.number_of_creeps));
            creep_role_stats.number_of_active_creeps.push(creep_stats
                .get(&role)
                .map_or(0, |stats| stats.number_of_active_creeps));
            creep_role_stats.max_active_creep_ttl.push(creep_stats
                .get(&role)
                .map_or(0, |stats| stats.max_active_creep_ttl));
            creep_role_stats.max_creep_ttl.push(creep_stats
                .get(&role)
                .map_or(0, |stats| stats.max_creep_ttl));

            // This division makes it rather inaccurate, but it is later averaged anyway.
            creep_role_stats.number_of_idle_creeps.push(
                self.number_of_idle_creeps
                    .get(&role)
                    .map_or(0, |&count| count / ticks_since_last_sample)
            );
        }

        self.number_of_idle_creeps.clear();
        self.creep_stats_by_role_sample_tick = game_tick()
    }

    pub fn creep_stats(&self, role: CreepRole) -> &RoomCreepStats {
        // TODO Ensure some stats exist before calling this.
        u!(self.creep_stats_by_role.get(&role))
    }
}