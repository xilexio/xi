use screeps::game;

#[derive(Debug, Copy, Clone, Hash, Eq, PartialEq)]
pub enum CreepRole {
    Miner,
    Hauler,
    Scout
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
}

impl Creep {
    pub fn exists(&self) -> bool {
        game::creeps().get(self.name.clone()).is_some()
    }
}