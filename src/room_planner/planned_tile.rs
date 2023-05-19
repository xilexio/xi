use crate::algorithms::matrix_common::MatrixCommon;
use crate::algorithms::room_matrix::RoomMatrix;
use crate::algorithms::room_matrix_slice::RoomMatrixSlice;
use crate::room_planner::packed_tile_structures::{MainStructureType, PackedTileStructures, PackedTileStructuresError};
use crate::room_state::StructuresMap;
use log::debug;
use modular_bitfield::specifiers::B3;
use modular_bitfield::{bitfield, BitfieldSpecifier};
use rustc_hash::FxHashMap;
use screeps::{RoomXY, StructureType, ROOM_SIZE};
use std::error::Error;
use std::fmt::{Display, Formatter};
use thiserror::Error;

#[derive(Error, Debug)]
pub enum PlannedTileError {
    #[error("trying to place an impassable structure and a reservation in one tile")]
    ReservationConflict,
}

#[derive(Debug, BitfieldSpecifier, Clone, Copy, Eq, PartialEq, Ord, PartialOrd)]
#[bits = 3]
pub enum BasePart {
    /// No rampart protection on given tile.
    Outside,
    /// Put under ramparts when in ranged attack range from outside and not outside. Do not influence placement of main ramparts.
    ProtectedIfInside,
    /// Put under ramparts when in ranged attack range from outside. Do not influence the placement of main ramparts.
    Protected,
    /// Keep ramparts on the tile or farther. Not necessarily under ramparts.
    Connected,
    /// Put under ramparts when in ranged attack range. Try to keep ramparts at ranged attack range or greater.
    Interior,
}

#[bitfield(bits = 16)]
#[derive(Copy, Clone, Eq, PartialEq, Debug)]
pub struct PlannedTile {
    pub structures: PackedTileStructures,
    pub reserved: bool,
    pub base_part: BasePart,
    pub min_rcl: B3,
    pub grown: bool,
    fill: bool,
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

    pub fn replace(self, structure_type: StructureType) -> Self {
        self.with_structures(self.structures().replace(structure_type))
    }

    pub fn upgrade_base_part(self, base_part: BasePart) -> Self {
        if self.base_part() < base_part {
            if (self.base_part() == BasePart::Protected || self.base_part() == BasePart::ProtectedIfInside)
                && base_part == BasePart::Connected
            {
                self.with_base_part(BasePart::Interior)
            } else {
                self.with_base_part(base_part)
            }
        } else {
            self
        }
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

    #[inline]
    pub fn merge_structure(
        &mut self,
        xy: RoomXY,
        structure_type: StructureType,
        base_part: BasePart,
        grown: bool
    ) -> Result<(), Box<dyn Error>> {
        // debug!("merge_structure {} {:?} {:?}", xy, structure_type, base_part);
        self.set(xy, self.get(xy).merge(structure_type)?.upgrade_base_part(base_part).with_grown(grown));
        Ok(())
    }

    #[inline]
    pub fn replace_structure(&mut self, xy: RoomXY, structure_type: StructureType, base_part: BasePart, grown: bool) {
        // debug!("replace_structure {} {:?} {:?} {}", xy, structure_type, base_part);
        self.set(
            xy,
            self.get(xy)
                .replace(structure_type)
                .upgrade_base_part(base_part)
                .with_grown(grown)
        )
    }

    #[inline]
    pub fn upgrade_base_part(&mut self, xy: RoomXY, base_part: BasePart) {
        // debug!("upgrade_base_part {} {:?}", xy, base_part);
        self.set(xy, self.get(xy).upgrade_base_part(base_part));
    }

    #[inline]
    pub fn reserve(&mut self, xy: RoomXY) {
        // debug!("reserve {}", xy);
        self.set(xy, self.get(xy).with_reserved(true));
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
