use rustc_hash::FxHashMap;
use screeps::{RoomXY, StructureType};
use crate::algorithms::matrix_common::MatrixCommon;
use crate::algorithms::room_matrix::RoomMatrix;
use crate::room_planning::packed_tile_structures::{PackedTileStructures, PackedTileStructuresError};
use crate::room_states::room_state::StructuresMap;

impl TryFrom<&StructuresMap> for RoomMatrix<PackedTileStructures> {
    type Error = PackedTileStructuresError;

    fn try_from(value: &StructuresMap) -> Result<RoomMatrix<PackedTileStructures>, Self::Error> {
        let mut matrix = RoomMatrix::default();
        
        for (structure_type, pos_set) in value {
            for pos in pos_set.iter() {
                let tile_structures: &mut PackedTileStructures = matrix.get_mut(*pos);
                *tile_structures = tile_structures.merge_structure(*structure_type)?;
            }
        }
        
        Ok(matrix)
    }
}

impl<V> TryFrom<&FxHashMap<StructureType, FxHashMap<RoomXY, V>>> for RoomMatrix<PackedTileStructures> {
    type Error = PackedTileStructuresError;

    fn try_from(value: &FxHashMap<StructureType, FxHashMap<RoomXY, V>>) -> Result<RoomMatrix<PackedTileStructures>, Self::Error> {
        let mut matrix = RoomMatrix::default();
        
        for (structure_type, data_vec) in value {
            for (&xy, _) in data_vec.iter() {
                let tile_structures: &mut PackedTileStructures = matrix.get_mut(xy);
                *tile_structures = tile_structures.merge_structure(*structure_type)?;
            }
        }
        
        Ok(matrix)
    }
}