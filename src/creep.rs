use crate::travel::TravelState;
use crate::u;
use screeps::{game, Position, ReturnCode, SharedCreepProperties};

#[derive(Debug, Copy, Clone, Hash, Eq, PartialEq)]
pub enum CreepRole {
    Miner,
    Hauler,
    Scout,
}

impl CreepRole {
    pub fn creep_name_prefix(self) -> &'static str {
        match self {
            CreepRole::Miner => "miner",
            CreepRole::Hauler => "hauler",
            CreepRole::Scout => "scout",
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
    pub fn move_to(&self, pos: Position) -> ReturnCode {
        u!(self.screeps_obj()).move_to(pos)
    }

    pub fn pos(&self) -> Position {
        u!(self.screeps_obj()).pos().into()
    }

    pub fn exists(&self) -> bool {
        self.screeps_obj().is_some()
    }

    fn screeps_obj(&self) -> Option<screeps::Creep> {
        game::creeps().get(self.name.clone())
    }
}
