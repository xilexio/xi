use crate::algorithms::matrix_common::MatrixCommon;
use crate::algorithms::room_matrix_slice::RoomMatrixSlice;
use crate::geometry::rect::Rect;
use crate::room_planner::planned_tile::{BasePart, PlannedTile};
use screeps::StructureType::{Container, Extension, Factory, Lab, Link, PowerSpawn, Road, Spawn, Storage, Terminal};

// TODO memoize - maybe use https://crates.io/crates/memoize
pub fn core_stamp() -> RoomMatrixSlice<PlannedTile> {
    let rect = Rect::new((0, 0).try_into().unwrap(), (6, 6).try_into().unwrap()).unwrap();
    let mut result = RoomMatrixSlice::new(rect, PlannedTile::default());

    result.set((1, 0).try_into().unwrap(), Road.into());
    result.set((2, 0).try_into().unwrap(), Road.into());
    result.set((3, 0).try_into().unwrap(), Road.into());
    result.set((4, 0).try_into().unwrap(), Road.into());
    result.set((5, 0).try_into().unwrap(), Road.into());

    result.set((0, 1).try_into().unwrap(), Road.into());
    result.set((1, 1).try_into().unwrap(), Storage.into());
    result.set((2, 1).try_into().unwrap(), PowerSpawn.into());
    result.set((3, 1).try_into().unwrap(), Terminal.into());
    result.set((4, 1).try_into().unwrap(), Extension.into());
    result.set((5, 1).try_into().unwrap(), Extension.into());
    result.set((6, 1).try_into().unwrap(), Road.into());

    result.set((0, 2).try_into().unwrap(), Road.into());
    result.set((1, 2).try_into().unwrap(), Factory.into());
    result.set((2, 2).try_into().unwrap(), PlannedTile::new().with_reserved(true));
    result.set((3, 2).try_into().unwrap(), Link.into());
    result.set((4, 2).try_into().unwrap(), PlannedTile::default().with_reserved(true));
    result.set((5, 2).try_into().unwrap(), Extension.into());
    result.set((6, 2).try_into().unwrap(), Road.into());

    result.set((0, 3).try_into().unwrap(), Road.into());
    result.set((1, 3).try_into().unwrap(), Spawn.into());
    result.set((2, 3).try_into().unwrap(), Extension.into());
    result.set(
        (3, 3).try_into().unwrap(),
        PlannedTile::from(Container).with_reserved(true),
    );
    result.set((4, 3).try_into().unwrap(), Extension.into());
    result.set((5, 3).try_into().unwrap(), Spawn.into());
    result.set((6, 3).try_into().unwrap(), Road.into());

    result.set((0, 4).try_into().unwrap(), Road.into());
    result.set((1, 4).try_into().unwrap(), Extension.into());
    result.set((2, 4).try_into().unwrap(), PlannedTile::default().with_reserved(true));
    result.set((3, 4).try_into().unwrap(), Extension.into());
    result.set((4, 4).try_into().unwrap(), PlannedTile::default().with_reserved(true));
    result.set((5, 4).try_into().unwrap(), Extension.into());
    result.set((6, 4).try_into().unwrap(), Road.into());

    result.set((0, 5).try_into().unwrap(), Road.into());
    result.set((1, 5).try_into().unwrap(), Extension.into());
    result.set((2, 5).try_into().unwrap(), Extension.into());
    result.set((3, 5).try_into().unwrap(), Spawn.into());
    result.set((4, 5).try_into().unwrap(), Extension.into());
    result.set((5, 5).try_into().unwrap(), Extension.into());
    result.set((6, 5).try_into().unwrap(), Road.into());

    result.set((1, 6).try_into().unwrap(), Road.into());
    result.set((2, 6).try_into().unwrap(), Road.into());
    result.set((3, 6).try_into().unwrap(), Road.into());
    result.set((4, 6).try_into().unwrap(), Road.into());
    result.set((5, 6).try_into().unwrap(), Road.into());

    result.map(|xy, tile| tile.with_base_part(BasePart::Interior))
}

// A compact core
// pub fn core_stamp() -> RoomMatrixSlice<PackedTileStructures> {
//     let rect = Rect::new((0, 0).try_into().unwrap(), (4, 4).try_into().unwrap()).unwrap();
//     let mut result = RoomMatrixSlice::new(rect, PackedTileStructures::default());
//     result.set((2, 0).try_into().unwrap(), Road.into());
//     result.set((3, 0).try_into().unwrap(), Road.into());
//
//     result.set((1, 1).try_into().unwrap(), Terminal.into());
//     result.set((2, 1).try_into().unwrap(), Spawn.into());
//     result.set((3, 1).try_into().unwrap(), Storage.into());
//     result.set((4, 1).try_into().unwrap(), Road.into());
//
//     result.set((1, 2).try_into().unwrap(), Nuker.into());
//     result.set((2, 2).try_into().unwrap(), PackedTileStructures::default().with_reservation());
//     result.set((3, 2).try_into().unwrap(), Container.into());
//     result.set((4, 2).try_into().unwrap(), Road.into());
//
//     result.set((1, 3).try_into().unwrap(), Link.into());
//     result.set((2, 3).try_into().unwrap(), Factory.into());
//     result.set((3, 3).try_into().unwrap(), PowerSpawn.into());
//
//     result
// }

pub fn labs_stamp() -> RoomMatrixSlice<PlannedTile> {
    let rect = Rect::new((0, 0).try_into().unwrap(), (3, 3).try_into().unwrap()).unwrap();
    let mut result = RoomMatrixSlice::new(rect, PlannedTile::default());
    result.set((1, 0).try_into().unwrap(), Lab.into());
    result.set((2, 0).try_into().unwrap(), Lab.into());

    result.set((0, 1).try_into().unwrap(), Lab.into());
    result.set((1, 1).try_into().unwrap(), Road.into());
    result.set((2, 1).try_into().unwrap(), Lab.into());
    result.set((3, 1).try_into().unwrap(), Lab.into());

    result.set((0, 2).try_into().unwrap(), Lab.into());
    result.set((1, 2).try_into().unwrap(), Lab.into());
    result.set((2, 2).try_into().unwrap(), Road.into());
    result.set((3, 2).try_into().unwrap(), Lab.into());

    result.set((1, 3).try_into().unwrap(), Lab.into());
    result.set((2, 3).try_into().unwrap(), Lab.into());

    result.map(|xy, tile| {
        if tile.is_empty() {
            tile
        } else {
            tile.with_base_part(BasePart::Interior)
        }
    })
}
