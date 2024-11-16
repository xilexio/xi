use screeps::Part::{Carry, Claim, Move, Work};
use screeps::{BodyPart, Part, BUILD_POWER, CARRY_CAPACITY, CREEP_CLAIM_LIFE_TIME, CREEP_LIFE_TIME, CREEP_SPAWN_TIME, HARVEST_POWER, MOVE_COST_PLAIN, MOVE_COST_ROAD, MOVE_POWER, UPGRADE_CONTROLLER_POWER};
use std::cmp::max;
use derive_more::Constructor;
use serde::{Deserialize, Serialize};
use std::fmt::{Display, Formatter};
use rustc_hash::FxHashMap;
use enum_iterator::all;
use crate::utils::part_extras::PartExtras;

#[derive(Debug, Clone, Constructor, Eq, PartialEq, Serialize, Deserialize)]
pub struct CreepBody {
    pub parts: Vec<Part>,
}

impl CreepBody {
    pub fn empty() -> CreepBody {
        CreepBody {
            parts: Vec::new(),
        }
    }

    pub fn lifetime(&self) -> u32 {
        if self.parts.contains(&Claim) {
            CREEP_CLAIM_LIFE_TIME
        } else {
            CREEP_LIFE_TIME
        }
    }

    pub fn spawn_duration(&self) -> u32 {
        self.parts.len() as u32 * CREEP_SPAWN_TIME
    }

    pub fn energy_cost(&self) -> u32 {
        self.parts.iter().map(|part| part.cost()).sum()
    }

    pub fn count_parts(&self, part: Part) -> u32 {
        self.parts.iter().filter(|&&p| p == part).count() as u32
    }

    /// The amount of energy per tick required to keep a creep with this body spawned.
    pub fn body_energy_usage(&self) -> f32 {
        self.energy_cost() as f32 / self.lifetime() as f32
    }

    pub fn store_capacity(&self) -> u32 {
        self.count_parts(Carry) * CARRY_CAPACITY
    }

    pub fn ticks_per_tile(&self, road: bool) -> u32 {
        let move_parts = self.count_parts(Move);
        if move_parts == 0 {
            return 10000;
        }
        let move_cost_per_part = if road { MOVE_COST_ROAD } else { MOVE_COST_PLAIN };
        let fatigue = (self.parts.len() as u32 - move_parts) * move_cost_per_part;
        let move_power = move_parts * MOVE_POWER;
        max(1, fatigue.div_ceil(move_power))
    }

    /// Amount of resources per tick per tile that can be carried by the creep with this body.
    /// This assumes a one-way trip, so the throughput is half of that when going back empty.
    pub fn hauling_throughput(&self, road: bool) -> f32 {
        self.store_capacity() as f32 / self.ticks_per_tile(road) as f32
    }

    pub fn build_energy_usage(&self) -> u32 {
        self.count_parts(Work) * BUILD_POWER
    }

    pub fn upgrade_energy_usage(&self) -> u32 {
        self.count_parts(Work) * UPGRADE_CONTROLLER_POWER
    }

    pub fn energy_harvest_power(&self) -> u32 {
        self.count_parts(Work) * HARVEST_POWER
    }
}

impl Display for CreepBody {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        let mut part_counts = FxHashMap::default();
        for &part in self.parts.iter() {
            *part_counts.entry(part).or_insert(0u32) += 1;
        }
        for part in all::<Part>() {
            if let Some(count) = part_counts.get(&part) {
                write!(f, "{}{}", count, part.single_char())?;
            }
        }
        Ok(())
    }
}

impl From<Vec<BodyPart>> for CreepBody {
    fn from(value: Vec<BodyPart>) -> Self {
        CreepBody {
            parts: value.into_iter().map(|part| part.part()).collect()
        }
    }
}