use std::default::Default;
use screeps::{ObjectId, RoomName, RoomXY, Structure, StructureType};
use crate::kernel::sleep::{sleep, sleep_until};
use crate::room_planning::plan_rooms::MIN_CONTAINER_RCL;
use crate::room_states::room_states::with_room_state;
use crate::u;
use crate::utils::decay::DecayInfo;
use crate::utils::game_tick::first_tick;

/// The minimum number of ticks to expiration of a structure until it is deemed in critical state.
const CRITICAL_TICKS_TO_EXPIRATION: u32 = 7500;

#[derive(Clone, Debug)]
pub struct StructureToRepair {
    pub id: ObjectId<Structure>,
    pub xy: RoomXY,
    pub hits: u32,
    pub hits_max: u32,
}

#[derive(Default, Clone, Debug)]
pub struct TriagedRepairSites {
    /// Repair sites that need to be repaired immediately. Specifically ones that are decaying and
    /// sufficiently close to expiration.
    pub critical: Vec<RepairSiteData>,
    /// Other repair sites, including ones that are low on hits, but are not decaying.
    pub regular: Vec<RepairSiteData>,
    /// Total hits to repair.
    pub total_hits_to_repair: u32,
}

#[derive(Clone, Debug)]
pub struct RepairSiteData {
    pub id: ObjectId<Structure>,
    pub structure_type: StructureType,
    pub xy: RoomXY,
    pub hits_to_repair: u32,
    /// The number of hits to which the structure is supposed to be repaired.
    pub target_hits: u32,
}

pub async fn triage_repair_sites(room_name: RoomName) {
    // TODO Figure out which structures need repairing and how many repairers need to be spawned
    //      for that.
    // TODO Triage structures into critical and normal priority. Priority ones are the decaying
    //      ones that are close to death. Spawn if any is critical.
    // TODO Otherwise spawn only if a repairer can reasonably use its whole life on repair.
    // TODO This involves economy and energy usage too, so maybe it should be done in a process
    //      that then signals eco_config to include it in calculations.
    // TODO Do not fully repair ramparts all at once. Actually, beyond some point that should depend
    //      on economy.

    sleep_until(first_tick() + 10).await;

    loop {
        u!(with_room_state(room_name, |room_state| {
            let mut triaged_repair_sites = TriagedRepairSites::default();

            for (&structure_type, structures_to_repair) in room_state.structures_to_repair.iter() {
                let min_non_critical_hits;
                if let (Some(decay_amount), Some(decay_ticks)) = (structure_type.decay_amount(), structure_type.decay_ticks(true)) {
                    min_non_critical_hits = decay_amount * CRITICAL_TICKS_TO_EXPIRATION.div_ceil(decay_ticks) + 1;
                } else {
                    min_non_critical_hits = 1;
                }
                
                for structure_to_repair in structures_to_repair.iter() {
                    let target_hits = match structure_type {
                        StructureType::Wall | StructureType::Rampart => rampart_target_hits(room_state.rcl),
                        StructureType::Container if room_state.rcl <= MIN_CONTAINER_RCL => 0,
                        _ => structure_to_repair.hits_max
                    };
                    
                    if structure_to_repair.hits < target_hits {
                        let hits_to_repair = target_hits - structure_to_repair.hits;
                        
                        let repair_site_data = RepairSiteData {
                            id: structure_to_repair.id,
                            structure_type,
                            xy: structure_to_repair.xy,
                            hits_to_repair,
                            target_hits,
                        };
    
                        if structure_to_repair.hits < min_non_critical_hits {
                            triaged_repair_sites.critical.push(repair_site_data);
                        } else {
                            triaged_repair_sites.regular.push(repair_site_data);
                        }
    
                        triaged_repair_sites.total_hits_to_repair += hits_to_repair;
                    }
                }
            }
            
            room_state.triaged_repair_sites = triaged_repair_sites;
        }));

        // TODO It is not required to check it each tick, but no new JS calls have to be made anyway.
        sleep(3).await;
    }
}

// TODO More dynamic, especially for high RCL. Also different for walls.
pub fn rampart_target_hits(rcl: u8) -> u32 {
    match rcl {
        6 => 25_000,
        7 => 50_000,
        8 => 100_000,
        _ => 0
    }
}

impl TriagedRepairSites {
    /// Chooses the closest repair site to given position. Prioritizes critical ones over regular
    /// ones regardless of the distance.
    pub fn choose_repair_site(&self, xy: RoomXY) -> Option<RepairSiteData> {
        let source = if !self.critical.is_empty() {
            Some(&self.critical)
        } else if !self.regular.is_empty() {
            Some(&self.regular)
        } else {
            None
        };
        
        source.and_then(|repair_sites| {
            repair_sites
                .iter()
                .map(|repair_site| (repair_site.xy.get_range_to(xy), repair_site))
                .min_by_key(|(dist, _)| *dist)
                .map(|(_, repair_site)| repair_site.clone())
        })
    }
    
    pub fn remove_repair_site(&mut self, id: ObjectId<Structure>) {
        self.critical.retain(|repair_site| repair_site.id != id);
        self.regular.retain(|repair_site| repair_site.id != id);
    }
}

#[cfg(test)]
mod tests {
    use crate::construction::triage_repair_sites::rampart_target_hits;
    use crate::room_planning::room_planner::MIN_RAMPART_RCL;

    #[test]
    fn check_rampart_target_hits_consistency() {
        for rcl in 0u8..=8u8 {
            assert_eq!(rampart_target_hits(rcl) > 0, rcl >= MIN_RAMPART_RCL); 
        }
    }
}