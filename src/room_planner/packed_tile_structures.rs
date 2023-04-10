use std::fmt::{Display, Formatter};
use rustc_hash::FxHashMap;
use screeps::StructureType;
use thiserror::Error;
use crate::algorithms::matrix_common::MatrixCommon;
use crate::algorithms::room_matrix::RoomMatrix;
use crate::room_state::StructuresMap;

#[derive(Copy, Clone, Debug, Eq, PartialEq, Default)]
/// Information about structure present in a tile and whether the tile is reserved for something.
pub struct PackedTileStructures(u8);

#[derive(Error, Debug)]
pub enum PackedTileStructuresError {
    #[error("invalid structure code")]
    InvalidStructureCode,
}

impl Display for PackedTileStructures {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        let main_structure = self.main_structure();
        let main_str = match main_structure.is_empty() {
            true => ' ',
            false => match StructureType::try_from(main_structure) {
                Ok(StructureType::Spawn) => 'S',
                Ok(StructureType::Extension) => 'E',
                Ok(StructureType::Wall) => 'W',
                Ok(StructureType::Controller) => '*',
                Ok(StructureType::Link) => 'K',
                Ok(StructureType::Storage) => 'R',
                Ok(StructureType::Tower) => 'T',
                Ok(StructureType::Observer) => 'O',
                Ok(StructureType::PowerSpawn) => 'P',
                Ok(StructureType::Extractor) => 'X',
                Ok(StructureType::Lab) => 'L',
                Ok(StructureType::Terminal) => 'M',
                Ok(StructureType::Container) => 'C',
                Ok(StructureType::Nuker) => 'N',
                Ok(StructureType::Factory) => 'F',
                Ok(StructureType::KeeperLair) => '$',
                Ok(StructureType::Portal) => '!',
                Ok(StructureType::PowerBank) => '&',
                Ok(StructureType::InvaderCore) => '@',
                _ => '?',
            }
        };
        let (left_str, right_str) = match (self.road().is_empty(), self.rampart().is_empty()) {
            (true, true) => (' ', ' '),
            (false, true) => ('|', '|'),
            (true, false) => ('(', ')'),
            (false, false) => ('[', ']'),
        };
        write!(f, "{}{}{}", left_str, main_str, right_str)
    }
}

impl From<StructureType> for PackedTileStructures {
    fn from(value: StructureType) -> Self {
        match value {
            StructureType::Spawn => PackedTileStructures(1),
            StructureType::Extension => PackedTileStructures(2),
            StructureType::Wall => PackedTileStructures(3),
            StructureType::Controller => PackedTileStructures(4),
            StructureType::Link => PackedTileStructures(5),
            StructureType::Storage => PackedTileStructures(6),
            StructureType::Tower => PackedTileStructures(7),
            StructureType::Observer => PackedTileStructures(8),
            StructureType::PowerSpawn => PackedTileStructures(9),
            StructureType::Extractor => PackedTileStructures(10),
            StructureType::Lab => PackedTileStructures(11),
            StructureType::Terminal => PackedTileStructures(12),
            StructureType::Container => PackedTileStructures(13),
            StructureType::Nuker => PackedTileStructures(14),
            StructureType::Factory => PackedTileStructures(15),
            StructureType::KeeperLair => PackedTileStructures(16),
            StructureType::Portal => PackedTileStructures(17),
            StructureType::PowerBank => PackedTileStructures(18),
            StructureType::InvaderCore => PackedTileStructures(19),
            StructureType::Road => PackedTileStructures(32),
            StructureType::Rampart => PackedTileStructures(64),
            _ => panic!("unsupported structure type"),
        }
    }
}

impl TryFrom<PackedTileStructures> for StructureType {
    type Error = PackedTileStructuresError;

    fn try_from(value: PackedTileStructures) -> Result<Self, Self::Error> {
        match value {
            PackedTileStructures(1) => Ok(StructureType::Spawn),
            PackedTileStructures(2) => Ok(StructureType::Extension),
            PackedTileStructures(3) => Ok(StructureType::Wall),
            PackedTileStructures(4) => Ok(StructureType::Controller),
            PackedTileStructures(5) => Ok(StructureType::Link),
            PackedTileStructures(6) => Ok(StructureType::Storage),
            PackedTileStructures(7) => Ok(StructureType::Tower),
            PackedTileStructures(8) => Ok(StructureType::Observer),
            PackedTileStructures(9) => Ok(StructureType::PowerSpawn),
            PackedTileStructures(10) => Ok(StructureType::Extractor),
            PackedTileStructures(11) => Ok(StructureType::Lab),
            PackedTileStructures(12) => Ok(StructureType::Terminal),
            PackedTileStructures(13) => Ok(StructureType::Container),
            PackedTileStructures(14) => Ok(StructureType::Nuker),
            PackedTileStructures(15) => Ok(StructureType::Factory),
            PackedTileStructures(16) => Ok(StructureType::KeeperLair),
            PackedTileStructures(17) => Ok(StructureType::Portal),
            PackedTileStructures(18) => Ok(StructureType::PowerBank),
            PackedTileStructures(19) => Ok(StructureType::InvaderCore),
            PackedTileStructures(32) => Ok(StructureType::Road),
            PackedTileStructures(64) => Ok(StructureType::Rampart),
            _ => Err(PackedTileStructuresError::InvalidStructureCode),
        }
    }
}

const ROAD: u8 = 32;
const RAMPART: u8 = 64;
const RESERVATION: u8 = 128;

impl PackedTileStructures {
    #[inline]
    pub fn main_structure(self) -> Self {
        PackedTileStructures(self.0 & !(ROAD | RAMPART | RESERVATION))
    }

    #[inline]
    pub fn impassable_structure(self) -> Self {
        let structure = self.main_structure();
        if structure == StructureType::Container.into() {
            PackedTileStructures::default()
        } else {
            structure
        }
    }

    #[inline]
    pub fn road(self) -> Self {
        PackedTileStructures(self.0 & ROAD)
    }

    #[inline]
    pub fn rampart(self) -> Self {
        PackedTileStructures(self.0 & RAMPART)
    }

    #[inline]
    pub fn is_reserved(self) -> bool {
        self.0 & RESERVATION != 0
    }

    #[inline]
    pub fn is_empty(self) -> bool {
        self.0 == 0
    }

    #[inline]
    pub fn is_passable(self, friendly: bool) -> bool {
        self.impassable_structure().is_empty() && !self.is_reserved() && (friendly || self.rampart().is_empty())
    }

    pub fn with_reservation(self) -> Self {
        PackedTileStructures(self.0 | RESERVATION)
    }

    pub fn with(self, structure_type: StructureType) -> Self {
        let code: PackedTileStructures = structure_type.into();
        if code.0 & RAMPART != 0 {
            PackedTileStructures(self.0 | code.0)
        } else if code.0 & ROAD != 0 {
            let main_structure = self.main_structure();
            if main_structure.is_empty() || main_structure == StructureType::Container.into() {
                PackedTileStructures(self.0 | code.0)
            } else {
                PackedTileStructures(self.0 & (ROAD | RAMPART | RESERVATION) | code.0)
            }
        } else if structure_type == StructureType::Container {
            PackedTileStructures((self.0 & (ROAD | RAMPART | RESERVATION)) | code.0)
        } else {
            PackedTileStructures((self.0 & (RAMPART | RESERVATION)) | code.0)
        }
    }

    #[inline]
    pub fn has(self, structure_type: StructureType) -> bool {
        let code: PackedTileStructures = structure_type.into();
        if code.0 & (ROAD | RAMPART) != 0 {
            self.0 & code.0 != 0
        } else {
            self.0 & !(ROAD | RAMPART | RESERVATION) == code.0
        }
    }

    pub fn iter(self) -> impl Iterator<Item = StructureType> {
        let mut structure_types = Vec::new();
        for packed_structure in [self.main_structure(), self.road(), self.rampart()] {
            packed_structure
                .try_into()
                .into_iter()
                .for_each(|structure_type| {
                    structure_types.push(structure_type);
                });
        }
        structure_types.into_iter()
    }
}

impl RoomMatrix<PackedTileStructures> {
    pub fn to_structures_map(self) -> StructuresMap {
        let mut result = FxHashMap::default();
        for (xy, packed_structures) in self.iter() {
            for structure_type in packed_structures.iter() {
                result.entry(structure_type).or_insert(Vec::new()).push(xy);
            }
        }
        result
    }
}

#[cfg(test)]
mod tests {
    use screeps::StructureType::{Container, Rampart, Road, Spawn};
    use crate::room_planner::packed_tile_structures::PackedTileStructures;

    #[test]
    fn test_is_passable() {
        assert!(PackedTileStructures::default().is_passable(true));
        assert!(PackedTileStructures::from(Road).is_passable(false));
        assert!(PackedTileStructures::from(Rampart).is_passable(true));
        assert!(!PackedTileStructures::from(Rampart).is_passable(false));
        assert!(PackedTileStructures::from(Container).is_passable(false));
        assert!(!PackedTileStructures::from(Container).with(Rampart).is_passable(false));
        assert!(!PackedTileStructures::from(Spawn).is_passable(true));
        assert!(!PackedTileStructures::from(Spawn).with(Rampart).is_passable(true));
        assert!(PackedTileStructures::from(Container).with(Road).with(Rampart).is_passable(true));
        assert!(!PackedTileStructures::from(Container).with(Road).with(Rampart).is_passable(false));
        assert!(PackedTileStructures::from(Container).with(Road).is_passable(false));
    }
}