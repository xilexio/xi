use std::fmt::{Display, Formatter};
use crate::travel::TravelState;
use crate::u;
use screeps::{
    game,
    Part,
    BodyPart,
    Position,
    ResourceType,
    ReturnCode,
    SharedCreepProperties,
    Source,
    Withdrawable,
    Resource,
    Transferable,
    RoomObject,
    CREEP_CLAIM_LIFE_TIME,
    CREEP_LIFE_TIME,
    CREEP_SPAWN_TIME,
    HARVEST_POWER,
};
use screeps::Part::{Claim, Work};
use derive_more::Constructor;
use crate::unchecked_transferable::UncheckedTransferable;

#[derive(Debug, Copy, Clone, Hash, Eq, PartialEq)]
pub enum CreepRole {
    Miner,
    Hauler,
    Scout,
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

#[derive(Debug)]
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
}

impl Creep {
    // Utility

    pub fn exists(&self) -> bool {
        self.screeps_obj().is_some()
    }

    fn screeps_obj(&self) -> Option<screeps::Creep> {
        game::creeps().get(self.name.clone())
    }

    // API wrappers
    
    pub fn body(&self) -> CreepBody {
        u!(self.screeps_obj()).body().into()
    }

    pub fn harvest(&self, source: &Source) -> ReturnCode {
        u!(self.screeps_obj()).harvest(source)
    }

    pub fn move_to(&self, pos: Position) -> ReturnCode {
        u!(self.screeps_obj()).move_to(pos)
    }

    pub fn pos(&self) -> Position {
        u!(self.screeps_obj()).pos().into()
    }

    pub fn public_say(&self, message: &str) {
        // Ignoring any error from this function.
        u!(self.screeps_obj()).say(message, true);
    }

    pub fn suicide(&self) -> ReturnCode {
        self.screeps_obj()
            .map(|creep| creep.suicide())
            .unwrap_or(ReturnCode::Ok)
    }

    /// Zero indicates a dead creep.
    pub fn ticks_to_live(&self) -> u32 {
        self.screeps_obj()
            .and_then(|creep| creep.ticks_to_live())
            .unwrap_or(0)
    }

    pub fn withdraw<T>(&self, target: &T, resource_type: ResourceType, amount: Option<u32>) -> ReturnCode
    where
        T: Withdrawable,
    {
        u!(self.screeps_obj()).withdraw(target, resource_type, amount)
    }

    pub fn pickup(&self, target: &Resource) -> ReturnCode {
        u!(self.screeps_obj()).pickup(target)
    }

    pub fn transfer<T>(&self, target: &T, resource_type: ResourceType, amount: Option<u32>) -> ReturnCode
    where
        T: Transferable
    {
        u!(self.screeps_obj()).transfer(target, resource_type, amount)
    }
    
    pub fn unchecked_transfer(&self, target: &RoomObject, resource_type: ResourceType, amount: Option<u32>) -> ReturnCode {
        let transferable_target = UncheckedTransferable(target);
        self.transfer(&transferable_target, resource_type, amount)
    }

    // Statistics

    pub fn energy_harvest_power(&self) -> u32 {
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
}

impl From<Vec<BodyPart>> for CreepBody {
    fn from(value: Vec<BodyPart>) -> Self {
        CreepBody {
            parts: value.into_iter().map(|part| part.part()).collect()
        }
    }
}