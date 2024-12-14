use std::fmt::{Display, Formatter};
use enum_iterator::Sequence;

#[derive(Debug, Copy, Clone, Hash, Eq, PartialEq, Sequence)]
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