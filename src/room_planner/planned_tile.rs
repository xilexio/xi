use std::fmt::{Display, Formatter};
use crate::algorithms::matrix_common::MatrixCommon;
use crate::algorithms::room_matrix::RoomMatrix;
use crate::room_planner::packed_tile_structures::{MainStructureType, PackedTileStructures};
use crate::room_state::StructuresMap;
use modular_bitfield::bitfield;
use modular_bitfield::specifiers::B7;
use rustc_hash::FxHashMap;
use screeps::{ROOM_SIZE, StructureType};

#[bitfield(bits = 16)]
#[derive(Copy, Clone, Eq, PartialEq, Debug)]
pub struct PlannedTile {
    pub structures: PackedTileStructures,
    pub reserved: bool,
    pub interior: bool,
    pub build_priority: B7,
}

impl Default for PlannedTile {
    fn default() -> Self {
        PlannedTile::new()
    }
}

impl PlannedTile {
    pub fn is_empty(self) -> bool {
        self.structures().is_empty() && !self.reserved()
    }

    pub fn is_passable(self, friendly: bool) -> bool {
        !self.reserved() && self.structures().is_passable(friendly)
    }

    pub fn with(self, structure_type: StructureType) -> Self {
        self.with_structures(self.structures().with(structure_type))
    }

    pub fn iter(self) -> impl Iterator<Item = StructureType> {
        self.structures().iter()
    }

    pub fn merge(self, other: Self) -> Self {
        let result = if self.structures().is_empty() {
            other
        } else if other.structures().is_empty() {
            self
        } else if other.structures().main() != MainStructureType::Empty {
            other.with_structures(self.structures().merge(other.structures()))
        } else {
            self.with_structures(self.structures().merge(other.structures()))
        };
        if self.reserved() || other.reserved() {
            result.with_reserved(true)
        } else {
            result
        }
    }
}

impl From<StructureType> for PlannedTile {
    fn from(value: StructureType) -> Self {
        PlannedTile::new().with_structures(value.into())
    }
}

impl Display for PlannedTile {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        if self.structures().is_empty() && self.reserved() {
            write!(f, "R")
        } else {
            write!(f, "{}", self.structures())
        }
    }
}

impl RoomMatrix<PlannedTile> {
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

impl Display for RoomMatrix<PlannedTile> {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        for y in 0..ROOM_SIZE {
            for x in 0..ROOM_SIZE {
                unsafe {
                    write!(f, "{}", self.get_xy(x, y))?;
                    if x != ROOM_SIZE - 1 {
                        write!(f, " ")?;
                    }
                }
            }
            writeln!(f)?;
        }
        Ok(())
    }
}
