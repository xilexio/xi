use screeps::Part::{Carry, Claim, Move, Work};
use screeps::{
    BodyPart,
    Part,
    BUILD_POWER,
    CARRY_CAPACITY,
    CREEP_CLAIM_LIFE_TIME,
    CREEP_LIFE_TIME,
    CREEP_SPAWN_TIME,
    HARVEST_POWER,
    MOVE_POWER,
    REPAIR_POWER,
    UPGRADE_CONTROLLER_POWER,
};
use std::cmp::max;
use std::collections::hash_map::Entry;
use serde::{Deserialize, Serialize};
use std::fmt::{Display, Formatter};
use std::iter::repeat;
use rustc_hash::FxHashMap;
use enum_iterator::all;
use crate::consts::REPAIR_COST_PER_PART;
use crate::travel::surface::Surface;
use crate::utils::part_extras::PartExtras;

// TODO Should cache of ticks_per_tile and others be here or in creep? Probably better here.
// TODO Serialize this in a string and then cache all stats.
// TODO Remove ordering from here and leave it to be computed automatically upon spawning?
#[derive(Debug, Clone, Eq, PartialEq, Serialize, Deserialize)]
pub struct CreepBody {
    /// A map from creep part types into their count and information how many are boosted.
    // TODO It does not suffice to say just boosted, the kind of boost is important for Work.
    pub parts: FxHashMap<Part, (u8, u8)>,
}

impl CreepBody {
    pub fn empty() -> CreepBody {
        CreepBody {
            parts: FxHashMap::default(),
        }
    }
    
    /// Returns a vector of all parts in the body, without information about boosts and in the order
    /// used in spawning.
    // TODO Make the order more efficient for spawning, i.e., non-military creeps should have one of
    //      each type at the end for faster API-side checks.
    pub fn parts_vec(&self) -> Vec<Part> {
        self.parts
            .iter()
            .flat_map(|(part, (count, _))| repeat(*part).take(*count as usize))
            .collect()
    }

    pub fn lifetime(&self) -> u32 {
        if self.parts.contains_key(&Claim) {
            CREEP_CLAIM_LIFE_TIME
        } else {
            CREEP_LIFE_TIME
        }
    }

    pub fn spawn_duration(&self) -> u32 {
        self.total_part_count() as u32 * CREEP_SPAWN_TIME
    }

    pub fn energy_cost(&self) -> u32 {
        self.parts.iter().map(|(part, (count, _))| part.cost() * (*count as u32)).sum()
    }

    pub fn count_parts(&self, part: Part) -> u8 {
        self.parts.get(&part).map(|(count, _)| *count).unwrap_or(0)
    }
    
    pub fn total_part_count(&self) -> u8 {
        self.parts.values().map(|(count, _)| *count).sum()
    }

    /// The amount of energy per tick required to keep a creep with this body spawned.
    pub fn body_energy_usage(&self) -> f32 {
        self.energy_cost() as f32 / self.lifetime() as f32
    }

    pub fn store_capacity(&self) -> u32 {
        self.count_parts(Carry) as u32 * CARRY_CAPACITY
    }

    pub fn ticks_per_tile(&self, surface: Surface) -> u8 {
        if surface == Surface::Obstacle {
            return u8::MAX;
        }
        let move_parts = self.count_parts(Move) as u16;
        if move_parts == 0 {
            return u8::MAX;
        }
        let move_cost_per_part = surface.move_cost() as u16;
        // The fatigue can be as high as 490, which exceeds u8::MAX.
        let fatigue = (self.total_part_count() as u16 - move_parts) * move_cost_per_part;
        let move_power = move_parts * MOVE_POWER as u16;
        max(1u8, fatigue.div_ceil(move_power) as u8)
    }
    
    pub fn fatigue_regen_ticks(&self, fatigue: u8) -> u8 {
        fatigue.div_ceil(self.count_parts(Move) * MOVE_POWER as u8)
    }

    /// Amount of resources per tick per tile that can be carried by the creep with this body.
    /// This assumes a one-way trip, so the throughput is half of that when going back empty.
    pub fn hauling_throughput(&self, surface: Surface) -> f32 {
        self.store_capacity() as f32 / self.ticks_per_tile(surface) as f32
    }

    pub fn build_energy_usage(&self) -> u32 {
        self.count_parts(Work) as u32 * BUILD_POWER
    }
    
    pub fn repair_energy_usage(&self) -> u32 {
        self.count_parts(Work) as u32 * REPAIR_COST_PER_PART
    }
    
    pub fn repair_power(&self) -> u32 {
        self.count_parts(Work) as u32 * REPAIR_POWER
    }

    pub fn upgrade_energy_usage(&self) -> u32 {
        self.count_parts(Work) as u32 * UPGRADE_CONTROLLER_POWER
    }

    /// How many energy units per tick can a creep with this body mine.
    pub fn energy_harvest_power(&self) -> u32 {
        self.count_parts(Work) as u32 * HARVEST_POWER
    }
}

impl Display for CreepBody {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        for part in all::<Part>() {
            // TODO Display boost.
            if let Some((count, _)) = self.parts.get(&part) {
                write!(f, "{}{}", count, part.single_char())?;
            }
        }
        Ok(())
    }
}

impl From<Vec<BodyPart>> for CreepBody {
    fn from(value: Vec<BodyPart>) -> Self {
        let mut parts = FxHashMap::default();
        for part in value {
            match parts.entry(part.part()) {
                Entry::Occupied(mut e) => {
                    let v: &mut (u8, u8) = e.get_mut();
                    (*v).0 += 1;
                    (*v).1 += part.boost().is_some() as u8;
                }
                Entry::Vacant(e) => {
                    e.insert((1, part.boost().is_some() as u8));
                }
            }
        }
        CreepBody {
            parts,
        }
    }
}

impl From<Vec<Part>> for CreepBody {
    fn from(value: Vec<Part>) -> Self {
        let mut parts = FxHashMap::default();
        for part in value {
            match parts.entry(part) {
                Entry::Occupied(mut e) => {
                    let v: &mut (u8, u8) = e.get_mut();
                    (*v).0 += 1;
                }
                Entry::Vacant(e) => {
                    e.insert((1, 0));
                }
            }
        }
        CreepBody {
            parts,
        }
    }
}

impl From<Vec<(Part, u8)>> for CreepBody {
    fn from(value: Vec<(Part, u8)>) -> Self {
        let mut parts = FxHashMap::default();
        for (part, count) in value {
            parts.insert(part, (count, 0));
        }
        Self {
            parts
        }
    }
}

#[cfg(test)]
mod tests {
    use num_traits::abs;
    use screeps::Part::{Move, Work};
    use screeps::{REPAIR_COST, REPAIR_POWER};
    use crate::creeps::creep_body::REPAIR_COST_PER_PART;
    use crate::travel::surface::Surface;

    #[test]
    fn test_ticks_per_tile() {
        assert_eq!(crate::creeps::creep_body::CreepBody::from(vec![(Move, 1)]).ticks_per_tile(Surface::Road), 1u8);
        assert_eq!(crate::creeps::creep_body::CreepBody::from(vec![(Move, 1)]).ticks_per_tile(Surface::Plain), 1u8);
        assert_eq!(crate::creeps::creep_body::CreepBody::from(vec![(Move, 1)]).ticks_per_tile(Surface::Swamp), 1u8);
        assert_eq!(crate::creeps::creep_body::CreepBody::from(vec![(Move, 1)]).ticks_per_tile(Surface::Obstacle), u8::MAX);
        
        assert_eq!(crate::creeps::creep_body::CreepBody::from(vec![(Move, 1), (Work, 3)]).ticks_per_tile(Surface::Road), 2u8);
        assert_eq!(crate::creeps::creep_body::CreepBody::from(vec![(Move, 1), (Work, 3)]).ticks_per_tile(Surface::Plain), 3u8);
        assert_eq!(crate::creeps::creep_body::CreepBody::from(vec![(Move, 1), (Work, 3)]).ticks_per_tile(Surface::Swamp), 15u8);
        assert_eq!(crate::creeps::creep_body::CreepBody::from(vec![(Move, 1), (Work, 3)]).ticks_per_tile(Surface::Obstacle), u8::MAX);
        
        assert_eq!(crate::creeps::creep_body::CreepBody::from(vec![(Move, 1), (Work, 49)]).ticks_per_tile(Surface::Road), 25u8);
        assert_eq!(crate::creeps::creep_body::CreepBody::from(vec![(Move, 1), (Work, 49)]).ticks_per_tile(Surface::Plain), 49u8);
        assert_eq!(crate::creeps::creep_body::CreepBody::from(vec![(Move, 1), (Work, 49)]).ticks_per_tile(Surface::Swamp), 245u8);
        assert_eq!(crate::creeps::creep_body::CreepBody::from(vec![(Move, 1), (Work, 49)]).ticks_per_tile(Surface::Obstacle), u8::MAX);
        
        assert_eq!(crate::creeps::creep_body::CreepBody::from(vec![(Work, 1)]).ticks_per_tile(Surface::Plain), u8::MAX);
        assert_eq!(crate::creeps::creep_body::CreepBody::from(vec![(Work, 1)]).ticks_per_tile(Surface::Swamp), u8::MAX);
    }
    
    #[test]
    fn test_constants_consistency() {
        assert!(abs(REPAIR_COST_PER_PART as f32 - (REPAIR_POWER as f32 * REPAIR_COST)) < 1e-6);
    }
}