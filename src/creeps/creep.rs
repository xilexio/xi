use std::cmp::max;
use std::fmt::{Display, Formatter};
use crate::travel::TravelState;
use crate::u;
use screeps::{game, Part, BodyPart, Position, ResourceType, SharedCreepProperties, Source, Withdrawable, Resource, Transferable, RoomObject, Store, HasPosition, ObjectId, MaybeHasId, StructureController, ConstructionSite, CREEP_CLAIM_LIFE_TIME, CREEP_LIFE_TIME, CREEP_SPAWN_TIME, HARVEST_POWER, UPGRADE_CONTROLLER_POWER, BUILD_POWER, CARRY_CAPACITY, MOVE_COST_PLAIN, MOVE_COST_ROAD, MOVE_POWER};
use screeps::Part::{Carry, Claim, Move, Work};
use derive_more::Constructor;
use serde::{Deserialize, Serialize};
use crate::errors::XiError;
use crate::errors::XiError::*;
use crate::utils::single_tick_cache::SingleTickCache;
use crate::utils::unchecked_transferable::UncheckedTransferable;
use crate::utils::unchecked_withdrawable::UncheckedWithdrawable;

#[derive(Debug, Copy, Clone, Hash, Eq, PartialEq)]
pub enum CreepRole {
    Scout,
    Miner,
    Hauler,
    Upgrader,
    Builder,
}

impl Default for CreepRole {
    fn default() -> Self {
        CreepRole::Scout
    }
}

impl Display for CreepRole {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:?}", self)
    }
}

impl CreepRole {
    pub fn creep_name_prefix(self) -> &'static str {
        match self {
            CreepRole::Miner => "miner",
            CreepRole::Hauler => "hauler",
            CreepRole::Scout => "scout",
            CreepRole::Upgrader => "upgrader",
            CreepRole::Builder => "builder",
        }
    }

    pub fn from_creep_name_prefix(creep_name_prefix: &str) -> Option<Self> {
        match creep_name_prefix {
            "miner" => Some(CreepRole::Miner),
            "hauler" => Some(CreepRole::Hauler),
            "scout" => Some(CreepRole::Scout),
            "upgrader" => Some(CreepRole::Upgrader),
            "builder" => Some(CreepRole::Builder),
            _ => None
        }
    }
}

#[derive(Debug, Default)]
pub struct Creep {
    /// Globally unique creep name.
    pub name: String,
    /// Creep role. May not change.
    pub role: CreepRole,
    /// Unique creep identifier, separate for each role.
    pub number: u32,
    /// State of travel of the creep with information about location where it is supposed to be
    /// and temporary state to be managed by the travel module.
    pub travel_state: TravelState,
    pub last_transfer_tick: u32,
    pub dead: bool,
    pub cached_creep: SingleTickCache<screeps::Creep>,
}

impl Creep {
    // Utility

    pub fn screeps_obj(&mut self) -> Result<&mut screeps::Creep, XiError> {
        if !self.dead {
            Ok(self.cached_creep.get_or_insert_with(|| u!(game::creeps().get(self.name.clone()))))
        } else {
            Err(CreepDead)
        }
    }

    pub fn screeps_id(&mut self) -> Result<ObjectId<screeps::Creep>, XiError> {
        // If the creep is alive, it must have an ID.
        Ok(u!(self.screeps_obj()?.try_id()))
    }

    // API wrappers
    
    pub fn body(&mut self) -> Result<CreepBody, XiError> {
        Ok(self.screeps_obj()?.body().into())
    }

    pub fn harvest(&mut self, source: &Source) -> Result<(), XiError> {
        self.screeps_obj()?.harvest(source).or(Err(CreepHarvestFailed))
    }

    pub fn move_to(&mut self, pos: Position) -> Result<(), XiError> {
        self.screeps_obj()?.move_to(pos).or(Err(CreepMoveToFailed))
    }

    pub fn pos(&mut self) -> Result<Position, XiError> {
        Ok(self.screeps_obj()?.pos().into())
    }

    pub fn public_say(&mut self, message: &str) -> Result<(), XiError> {
        self.screeps_obj()?.say(message, true).or(Err(CreepSayFailed))
    }

    pub fn suicide(&mut self) -> Result<(), XiError> {
        self.screeps_obj()?.suicide().or(Err(CreepSuicideFailed))
    }

    /// Zero indicates a dead creep.
    pub fn ticks_to_live(&mut self) -> u32 {
        match self.screeps_obj() {
            Ok(creep) => creep.ticks_to_live().unwrap_or(0),
            Err(_) => 0,
        }
    }

    pub fn withdraw<T>(&mut self, target: &T, resource_type: ResourceType, amount: Option<u32>) -> Result<(), XiError>
    where
        T: Withdrawable,
    {
        self.screeps_obj()?.withdraw(target, resource_type, amount).or(Err(CreepWithdrawFailed))
    }

    pub fn unchecked_withdraw(&mut self, target: &RoomObject, resource_type: ResourceType, amount: Option<u32>) -> Result<(), XiError> {
        let unchecked_target = UncheckedWithdrawable(target);
        self.withdraw(&unchecked_target, resource_type, amount)
    }

    pub fn pickup(&mut self, target: &Resource) -> Result<(), XiError> {
        self.screeps_obj()?.pickup(target).or(Err(CreepPickupFailed))
    }

    pub fn transfer<T>(&mut self, target: &T, resource_type: ResourceType, amount: Option<u32>) -> Result<(), XiError>
    where
        T: Transferable
    {
        self.screeps_obj()?.transfer(target, resource_type, amount).or(Err(CreepTransferFailed))
    }

    pub fn unchecked_transfer(&mut self, target: &RoomObject, resource_type: ResourceType, amount: Option<u32>) -> Result<(), XiError> {
        let unchecked_target = UncheckedTransferable(target);
        self.transfer(&unchecked_target, resource_type, amount)
    }

    pub fn drop(&mut self, resource_type: ResourceType, amount: Option<u32>) -> Result<(), XiError> {
        self.screeps_obj()?.drop(resource_type, amount).or(Err(CreepDropFailed))
    }

    pub fn upgrade_controller(&mut self, controller: &StructureController) -> Result<(), XiError> {
        self.screeps_obj()?.upgrade_controller(controller).or(Err(CreepUpgradeControllerFailed))
    }

    pub fn build(&mut self, construction_site: &ConstructionSite) -> Result<(), XiError> {
        self.screeps_obj()?.build(construction_site).or(Err(CreepBuildFailed))
    }

    pub fn store(&mut self) -> Result<Store, XiError> {
        Ok(self.screeps_obj()?.store())
    }

    // Statistics

    pub fn energy_harvest_power(&mut self) -> Result<u32, XiError> {
        Ok(self.screeps_obj()?
            .body()
            .iter()
            .filter_map(|body_part| (body_part.part() == Work).then_some(HARVEST_POWER))
            .sum())
    }

    pub fn upgrade_energy_consumption(&mut self) -> Result<u32, XiError> {
        Ok(self.screeps_obj()?
            .body()
            .iter()
            .filter_map(|body_part| (body_part.part() == Work).then_some(UPGRADE_CONTROLLER_POWER))
            .sum())
    }

    pub fn build_energy_consumption(&mut self) -> Result<u32, XiError> {
        Ok(self.screeps_obj()?
            .body()
            .iter()
            .filter_map(|body_part| (body_part.part() == Work).then_some(BUILD_POWER))
            .sum())
    }
}

#[derive(Debug, Clone, Constructor, Eq, PartialEq, Serialize, Deserialize)]
pub struct CreepBody {
    pub parts: Vec<Part>,
}

impl CreepBody {
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
}

impl From<Vec<BodyPart>> for CreepBody {
    fn from(value: Vec<BodyPart>) -> Self {
        CreepBody {
            parts: value.into_iter().map(|part| part.part()).collect()
        }
    }
}