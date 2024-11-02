use std::fmt::{Display, Formatter};
use crate::travel::TravelState;
use crate::u;
use screeps::{game, Part, BodyPart, Position, ResourceType, SharedCreepProperties, Source, Withdrawable, Resource, Transferable, RoomObject, Store, CREEP_CLAIM_LIFE_TIME, CREEP_LIFE_TIME, CREEP_SPAWN_TIME, HARVEST_POWER, HasPosition};
use screeps::Part::{Claim, Work};
use derive_more::Constructor;
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
        }
    }

    pub fn from_creep_name_prefix(creep_name_prefix: &str) -> Option<Self> {
        match creep_name_prefix {
            "miner" => Some(CreepRole::Miner),
            "hauler" => Some(CreepRole::Hauler),
            "scout" => Some(CreepRole::Scout),
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

    pub fn store(&mut self) -> Result<Store, XiError> {
        Ok(self.screeps_obj()?.store())
    }

    // Statistics

    pub fn energy_harvest_power(&mut self) -> u32 {
        u!(self.screeps_obj()).body().iter().filter_map(|body_part| (body_part.part() == Work).then_some(HARVEST_POWER)).sum()
    }
}

#[derive(Debug, Clone, Constructor, Eq, PartialEq)]
pub struct CreepBody {
    pub parts: Vec<Part>,
}

impl CreepBody {
    pub(crate) fn lifetime(&self) -> u32 {
        if self.parts.contains(&Claim) {
            CREEP_CLAIM_LIFE_TIME
        } else {
            CREEP_LIFE_TIME
        }
    }
}

impl CreepBody {
    pub fn spawn_duration(&self) -> u32 {
        self.parts.len() as u32 * CREEP_SPAWN_TIME
    }

    pub fn energy_cost(&self) -> u32 {
        self.parts.iter().map(|part| part.cost()).sum()
    }
}

impl From<Vec<BodyPart>> for CreepBody {
    fn from(value: Vec<BodyPart>) -> Self {
        CreepBody {
            parts: value.into_iter().map(|part| part.part()).collect()
        }
    }
}