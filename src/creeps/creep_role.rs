use std::fmt::{Display, Formatter};
use enum_iterator::Sequence;
use screeps::Part;

#[derive(Debug, Default, Copy, Clone, Hash, Eq, PartialEq, Sequence)]
pub enum CreepRole {
    #[default]
    Scout,
    Miner,
    Hauler,
    Upgrader,
    Builder,
    Repairer,
    Claimer,
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
            CreepRole::Repairer => "repairer",
            CreepRole::Claimer => "claimer",
        }
    }

    pub fn from_creep_name_prefix(creep_name_prefix: &str) -> Option<Self> {
        match creep_name_prefix {
            "miner" => Some(CreepRole::Miner),
            "hauler" => Some(CreepRole::Hauler),
            "scout" => Some(CreepRole::Scout),
            "upgrader" => Some(CreepRole::Upgrader),
            "builder" => Some(CreepRole::Builder),
            "repairer" => Some(CreepRole::Repairer),
            "claimer" => Some(CreepRole::Claimer),
            _ => None
        }
    }
    
    pub fn primary_part(&self) -> Part {
        match self {
            CreepRole::Miner => Part::Work,
            CreepRole::Hauler => Part::Carry,
            CreepRole::Scout => Part::Move,
            CreepRole::Upgrader => Part::Work,
            CreepRole::Builder => Part::Work,
            CreepRole::Repairer => Part::Work,
            CreepRole::Claimer => Part::Claim,
        }
    }
}