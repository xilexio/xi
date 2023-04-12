use modular_bitfield::{bitfield, BitfieldSpecifier};
use screeps::StructureType;
use std::fmt::{Display, Formatter};
use thiserror::Error;

#[derive(BitfieldSpecifier, Eq, PartialEq, Debug)]
#[bits = 5]
pub enum MainStructureType {
    Empty,
    Spawn,
    Extension,
    Wall,
    Controller,
    Link,
    Storage,
    Tower,
    Observer,
    PowerSpawn,
    Extractor,
    Lab,
    Terminal,
    Container,
    Nuker,
    Factory,
    KeeperLair,
    Portal,
    PowerBank,
    InvaderCore,
}

/// Information about structure present in a tile and whether the tile is reserved for something.
#[bitfield(bits = 7)]
#[derive(Copy, Clone, BitfieldSpecifier, Eq, PartialEq, Debug)]
pub struct PackedTileStructures {
    pub main: MainStructureType,
    pub road: bool,
    pub rampart: bool,
}

impl Default for PackedTileStructures {
    fn default() -> Self {
        PackedTileStructures::new()
    }
}

#[derive(Error, Debug)]
pub enum PackedTileStructuresError {
    #[error("invalid main structure type")]
    InvalidMainStructureType,
    #[error("empty main structure type")]
    EmptyMainStructureType,
}

impl Display for PackedTileStructures {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        let main_str = match self.main() {
            MainStructureType::Empty => ' ',
            MainStructureType::Spawn => 'S',
            MainStructureType::Extension => 'E',
            MainStructureType::Wall => 'W',
            MainStructureType::Controller => '*',
            MainStructureType::Link => 'I',
            MainStructureType::Storage => 'R',
            MainStructureType::Tower => 'T',
            MainStructureType::Observer => 'O',
            MainStructureType::PowerSpawn => 'P',
            MainStructureType::Extractor => 'X',
            MainStructureType::Lab => 'L',
            MainStructureType::Terminal => 'Y',
            MainStructureType::Container => 'C',
            MainStructureType::Nuker => 'N',
            MainStructureType::Factory => 'F',
            MainStructureType::KeeperLair => '^',
            MainStructureType::Portal => '%',
            MainStructureType::PowerBank => '=',
            MainStructureType::InvaderCore => '@',
        };
        let (left_str, right_str) = match (self.road(), self.rampart()) {
            (false, true) => (' ', ' '),
            (true, false) => ('|', '|'),
            (false, false) => ('(', ')'),
            (true, true) => ('[', ']'),
        };
        write!(f, "{}{}{}", left_str, main_str, right_str)
    }
}

impl TryFrom<StructureType> for MainStructureType {
    type Error = PackedTileStructuresError;

    fn try_from(value: StructureType) -> Result<Self, Self::Error> {
        match value {
            StructureType::Spawn => Ok(MainStructureType::Spawn),
            StructureType::Extension => Ok(MainStructureType::Extension),
            StructureType::Wall => Ok(MainStructureType::Wall),
            StructureType::Controller => Ok(MainStructureType::Controller),
            StructureType::Link => Ok(MainStructureType::Link),
            StructureType::Storage => Ok(MainStructureType::Storage),
            StructureType::Tower => Ok(MainStructureType::Tower),
            StructureType::Observer => Ok(MainStructureType::Observer),
            StructureType::PowerSpawn => Ok(MainStructureType::PowerSpawn),
            StructureType::Extractor => Ok(MainStructureType::Extractor),
            StructureType::Lab => Ok(MainStructureType::Lab),
            StructureType::Terminal => Ok(MainStructureType::Terminal),
            StructureType::Container => Ok(MainStructureType::Container),
            StructureType::Nuker => Ok(MainStructureType::Nuker),
            StructureType::Factory => Ok(MainStructureType::Factory),
            StructureType::KeeperLair => Ok(MainStructureType::KeeperLair),
            StructureType::Portal => Ok(MainStructureType::Portal),
            StructureType::PowerBank => Ok(MainStructureType::PowerBank),
            StructureType::InvaderCore => Ok(MainStructureType::InvaderCore),
            _ => Err(PackedTileStructuresError::InvalidMainStructureType),
        }
    }
}


impl TryFrom<MainStructureType> for StructureType {
    type Error = PackedTileStructuresError;

    fn try_from(value: MainStructureType) -> Result<Self, Self::Error> {
        match value {
            MainStructureType::Empty => Err(PackedTileStructuresError::InvalidMainStructureType),
            MainStructureType::Spawn => Ok(StructureType::Spawn),
            MainStructureType::Extension => Ok(StructureType::Extension),
            MainStructureType::Wall => Ok(StructureType::Wall),
            MainStructureType::Controller => Ok(StructureType::Controller),
            MainStructureType::Link => Ok(StructureType::Link),
            MainStructureType::Storage => Ok(StructureType::Storage),
            MainStructureType::Tower => Ok(StructureType::Tower),
            MainStructureType::Observer => Ok(StructureType::Observer),
            MainStructureType::PowerSpawn => Ok(StructureType::PowerSpawn),
            MainStructureType::Extractor => Ok(StructureType::Extractor),
            MainStructureType::Lab => Ok(StructureType::Lab),
            MainStructureType::Terminal => Ok(StructureType::Terminal),
            MainStructureType::Container => Ok(StructureType::Container),
            MainStructureType::Nuker => Ok(StructureType::Nuker),
            MainStructureType::Factory => Ok(StructureType::Factory),
            MainStructureType::KeeperLair => Ok(StructureType::KeeperLair),
            MainStructureType::Portal => Ok(StructureType::Portal),
            MainStructureType::PowerBank => Ok(StructureType::PowerBank),
            MainStructureType::InvaderCore => Ok(StructureType::InvaderCore),
        }
    }
}

impl From<StructureType> for PackedTileStructures {
    fn from(value: StructureType) -> Self {
        match value {
            StructureType::Road => PackedTileStructures::new().with_road(true),
            StructureType::Rampart => PackedTileStructures::new().with_rampart(true),
            _ => match value.try_into() {
                Ok(main_structure_type) => PackedTileStructures::new().with_main(main_structure_type),
                Err(_) => panic!("unsupported structure type"),
            },
        }
    }
}

impl PackedTileStructures {
    #[inline]
    pub fn is_empty(self) -> bool {
        self == Self::new()
    }

    #[inline]
    pub fn is_passable(self, friendly: bool) -> bool {
        (self.main() == MainStructureType::Empty || self.main() == MainStructureType::Container)
            && (friendly || !self.rampart())
    }

    pub fn with(self, structure_type: StructureType) -> Self {
        if structure_type == StructureType::Rampart {
            self.with_rampart(true)
        } else if structure_type == StructureType::Road {
            if self.main() == MainStructureType::Empty || self.main() == MainStructureType::Container {
                self.with_road(true)
            } else {
                self.with_main(MainStructureType::Empty).with_road(true)
            }
        } else if structure_type == StructureType::Container {
            self.with_main(MainStructureType::Container)
        } else {
            self.with_main(structure_type.try_into().unwrap()).with_road(false)
        }
    }

    pub fn iter(self) -> impl Iterator<Item = StructureType> {
        let mut structure_types = Vec::new();
        if let Ok(structure_type) = self.main().try_into() { structure_types.push(structure_type) }
        if self.road() {
            structure_types.push(StructureType::Road);
        }
        if self.rampart() {
            structure_types.push(StructureType::Rampart);
        }
        structure_types.into_iter()
    }

    pub fn merge(self, other: Self) -> Self {
        let mut result = self;
        for structure_type in other.iter() {
            result = result.with(structure_type);
        }
        result
    }
}

#[cfg(test)]
mod tests {
    use crate::room_planner::packed_tile_structures::{MainStructureType, PackedTileStructures};
    use screeps::StructureType::{Container, Rampart, Road, Spawn};

    #[test]
    fn test_is_passable() {
        assert!(PackedTileStructures::default().is_passable(true));
        assert_eq!(PackedTileStructures::from(Road).main(), MainStructureType::Empty);
        assert!(PackedTileStructures::from(Road).is_passable(false));
        assert!(PackedTileStructures::from(Rampart).is_passable(true));
        assert!(!PackedTileStructures::from(Rampart).is_passable(false));
        assert!(PackedTileStructures::from(Container).is_passable(false));
        assert!(!PackedTileStructures::from(Container).with(Rampart).is_passable(false));
        assert!(!PackedTileStructures::from(Spawn).is_passable(true));
        assert!(!PackedTileStructures::from(Spawn).with(Rampart).is_passable(true));
        assert!(PackedTileStructures::from(Container)
            .with(Road)
            .with(Rampart)
            .is_passable(true));
        assert!(!PackedTileStructures::from(Container)
            .with(Road)
            .with(Rampart)
            .is_passable(false));
        assert!(PackedTileStructures::from(Container).with(Road).is_passable(false));
    }
}
