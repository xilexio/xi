use rustc_hash::FxHashMap;
use screeps::StructureType;
use thiserror::Error;
use crate::algorithms::matrix_common::MatrixCommon;
use crate::algorithms::room_matrix::RoomMatrix;
use crate::room_state::StructuresMap;

#[derive(Copy, Clone, Debug, Eq, PartialEq, Default)]
/// Information about structure present in a tile. Uses only 7 bits, though may be further compressed into 6 bits if
/// 4 structures would be removed (keeper lair, portal, power bank, invader core).
pub struct PackedTileStructures(u8);

#[derive(Error, Debug)]
pub enum PackedTileStructuresError {
    #[error("invalid structure code")]
    InvalidStructureCode,
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

const ROAD_AND_RAMPART: u8 = 32 + 64;

impl PackedTileStructures {
    pub fn main_structure(self) -> Self {
        PackedTileStructures(self.0 & !ROAD_AND_RAMPART)
    }

    pub fn road(self) -> Self {
        PackedTileStructures(self.0 & 32)
    }

    pub fn rampart(self) -> Self {
        PackedTileStructures(self.0 & 64)
    }

    pub fn empty(self) -> bool {
        self.0 == 0
    }

    pub fn with(self, structure_type: StructureType) -> PackedTileStructures {
        let code: PackedTileStructures = structure_type.try_into().unwrap();
        if code.0 & ROAD_AND_RAMPART != 0 {
            PackedTileStructures(self.0 | code.0)
        } else {
            PackedTileStructures((self.0 & ROAD_AND_RAMPART) | code.0)
        }
    }

    pub fn has(self, structure_type: StructureType) -> bool {
        let code: PackedTileStructures = structure_type.try_into().unwrap();
        if code.0 & ROAD_AND_RAMPART != 0 {
            self.0 & code.0 != 0
        } else {
            self.0 & !ROAD_AND_RAMPART == code.0
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
