use std::error::Error;
use crate::algorithms::matrix_common::MatrixCommon;
use crate::algorithms::room_matrix::RoomMatrix;
use crate::room_planner::packed_tile_structures::{MainStructureType, PackedTileStructures, PackedTileStructuresError};
use crate::room_state::StructuresMap;
use modular_bitfield::bitfield;
use modular_bitfield::specifiers::B7;
use rustc_hash::FxHashMap;
use screeps::{RoomXY, StructureType, ROOM_SIZE};
use std::fmt::{Display, Formatter};
use log::debug;
use thiserror::Error;
use crate::algorithms::room_matrix_slice::RoomMatrixSlice;

#[derive(Error, Debug)]
pub enum PlannedTileError {
    #[error("trying to place an impassable structure and a reservation in one tile")]
    ReservationConflict,
}

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

    pub fn merge(self, structure_type: StructureType) -> Result<Self, PackedTileStructuresError> {
        Ok(self.with_structures(self.structures().merge(structure_type)?))
    }

    pub fn merge_tile(self, other: Self) -> Result<Self, Box<dyn Error>> {
        let result = if self.structures().is_empty() {
            other
        } else if other.structures().is_empty() {
            self
        } else {
            let merged_structures = self.structures().merge_tile(other.structures())?;
            if other.structures().main() != MainStructureType::Empty {
                other.with_structures(merged_structures)
            } else {
                self.with_structures(merged_structures)
            }
        };

        if self.reserved() || other.reserved() {
            if result.structures().is_passable(true) {
                Ok(result.with_reserved(true))
            } else {
                Err(PlannedTileError::ReservationConflict)?
            }
        } else {
            Ok(result)
        }
    }

    pub fn iter(self) -> impl Iterator<Item = StructureType> {
        self.structures().iter()
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
            write!(f, " _ ")
        } else {
            write!(f, "{}", self.structures())
        }
    }
}

impl RoomMatrix<PlannedTile> {
    pub fn to_structures_map(&self) -> StructuresMap {
        let mut result = FxHashMap::default();
        for (xy, tile) in self.iter() {
            for structure_type in tile.iter() {
                result.entry(structure_type).or_insert(Vec::new()).push(xy);
            }
        }
        result
    }

    pub fn find_structure_xys(&self, structure_type: StructureType) -> Vec<RoomXY> {
        if structure_type == StructureType::Road {
            self.iter()
                .filter_map(|(xy, tile)| (tile.structures().road()).then_some(xy))
                .collect::<Vec<_>>()
        } else if structure_type == StructureType::Rampart {
            self.iter()
                .filter_map(|(xy, tile)| (tile.structures().rampart()).then_some(xy))
                .collect::<Vec<_>>()
        } else {
            self.iter()
                .filter_map(|(xy, tile)| (tile.structures().main() == structure_type.try_into().unwrap()).then_some(xy))
                .collect::<Vec<_>>()
        }
    }

    pub fn merge_structures(&mut self, slice: &RoomMatrixSlice<PlannedTile>) -> Result<(), Box<dyn Error>> {
        for (xy, other_tile) in slice.iter() {
            let current_tile = self.get(xy);
            match current_tile.merge_tile(other_tile) {
                Ok(tile) => self.set(xy, tile),
                Err(e) => {
                    debug!("Failed to merge {} at {}.", current_tile, xy);
                    Err(e)?
                }
            }
        }
        Ok(())
    }

    pub fn merge_structure(&mut self, xy: RoomXY, structure_type: StructureType) -> Result<(), Box<dyn Error>> {
        self.set(xy, self.get(xy).merge(structure_type)?);
        Ok(())
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
