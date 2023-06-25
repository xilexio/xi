#[derive(Debug, Copy, Clone, Hash, Eq, PartialEq)]
pub enum CreepRole {
    Craftsman,
    Scout
}

impl CreepRole {
    pub fn creep_name_prefix(self) -> &'static str {
        match self {
            CreepRole::Craftsman => "craftsman",
            CreepRole::Scout => "scout",
        }
    }
}

#[derive(Debug)]
pub struct Creep {
    pub name: String
}