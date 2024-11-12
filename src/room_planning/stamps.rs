use crate::algorithms::matrix_common::MatrixCommon;
use crate::algorithms::room_matrix_slice::RoomMatrixSlice;
use crate::geometry::rect::Rect;
use crate::room_planning::planned_tile::{BasePart, PlannedTile};
use screeps::StructureType::{Container, Extension, Factory, Lab, Link, PowerSpawn, Road, Spawn, Storage, Terminal};
use crate::room_planning::plan_rooms::MIN_CONTAINER_RCL;
use crate::room_planning::room_planner::SOURCE_AND_CONTROLLER_ROAD_RCL;

/// Fast filler/core stamp.
// {
//   "rcl": 8,
//   "buildings": {
//     "road": [
//       {"x":17,"y":9},
//       {"x":18,"y":9},
//       {"x":19,"y":9},
//       {"x":20,"y":9},
//       {"x":21,"y":9},
//       {"x":16,"y":10},
//       {"x":16,"y":11},
//       {"x":16,"y":12},
//       {"x":16,"y":13},
//       {"x":16,"y":14},
//       {"x":22,"y":10},
//       {"x":22,"y":11},
//       {"x":22,"y":12},
//       {"x":18,"y":15},
//       {"x":19,"y":15},
//       {"x":20,"y":15},
//       {"x":17,"y":15},
//       {"x":21,"y":15},
//       {"x":22,"y":14},
//       {"x":22,"y":13}
//     ],
//     "powerSpawn": [
//       {"x":18,"y":10}
//     ],
//     "storage": [
//       {"x":17,"y":10}
//     ],
//     "terminal": [
//       {"x":19,"y":10}
//     ],
//     "extension": [
//       {"x":20,"y":10},
//       {"x":21,"y":10},
//       {"x":21,"y":11},
//       {"x":21,"y":13},
//       {"x":21,"y":14},
//       {"x":20,"y":14},
//       {"x":18,"y":14},
//       {"x":17,"y":14},
//       {"x":17,"y":13},
//       {"x":20,"y":12},
//       {"x":19,"y":13},
//       {"x":18,"y":12}
//     ],
//     "link": [
//       {"x":19,"y":11}
//     ],
//     "container": [
//       {"x":19,"y":12}
//     ],
//     "spawn": [
//       {"x":17,"y":12},
//       {"x":19,"y":14},
//       {"x":21,"y":12}
//     ],
//     "factory": [
//       {"x":17,"y":11}
//     ]
//   }
// }
// TODO memoize - maybe use https://crates.io/crates/memoize
pub fn core_stamp() -> RoomMatrixSlice<PlannedTile> {
    let rect = Rect::new((0, 0).try_into().unwrap(), (6, 6).try_into().unwrap()).unwrap();
    let mut result = RoomMatrixSlice::new(rect, PlannedTile::default());

    result.set((1, 0).try_into().unwrap(), PlannedTile::from(Road).with_min_rcl(SOURCE_AND_CONTROLLER_ROAD_RCL));
    result.set((2, 0).try_into().unwrap(), PlannedTile::from(Road).with_min_rcl(SOURCE_AND_CONTROLLER_ROAD_RCL));
    result.set((3, 0).try_into().unwrap(), PlannedTile::from(Road).with_min_rcl(SOURCE_AND_CONTROLLER_ROAD_RCL));
    result.set((4, 0).try_into().unwrap(), PlannedTile::from(Road).with_min_rcl(SOURCE_AND_CONTROLLER_ROAD_RCL));
    result.set((5, 0).try_into().unwrap(), PlannedTile::from(Road).with_min_rcl(SOURCE_AND_CONTROLLER_ROAD_RCL));

    result.set((0, 1).try_into().unwrap(), PlannedTile::from(Road).with_min_rcl(SOURCE_AND_CONTROLLER_ROAD_RCL));
    result.set((1, 1).try_into().unwrap(), PlannedTile::from(Storage).with_min_rcl(4));
    result.set(
        (2, 1).try_into().unwrap(),
        PlannedTile::from(PowerSpawn).with_min_rcl(8),
    );
    result.set((3, 1).try_into().unwrap(), PlannedTile::from(Terminal).with_min_rcl(6));
    result.set((4, 1).try_into().unwrap(), PlannedTile::from(Extension).with_min_rcl(3));
    result.set((5, 1).try_into().unwrap(), PlannedTile::from(Extension).with_min_rcl(3));
    result.set((6, 1).try_into().unwrap(), PlannedTile::from(Road).with_min_rcl(SOURCE_AND_CONTROLLER_ROAD_RCL));

    result.set((0, 2).try_into().unwrap(), PlannedTile::from(Road).with_min_rcl(SOURCE_AND_CONTROLLER_ROAD_RCL));
    result.set((1, 2).try_into().unwrap(), PlannedTile::from(Factory).with_min_rcl(7));
    result.set((2, 2).try_into().unwrap(), PlannedTile::new().with_reserved(true));
    result.set((3, 2).try_into().unwrap(), PlannedTile::from(Link).with_min_rcl(5));
    result.set((4, 2).try_into().unwrap(), PlannedTile::default().with_reserved(true));
    result.set((5, 2).try_into().unwrap(), PlannedTile::from(Extension).with_min_rcl(3));
    result.set((6, 2).try_into().unwrap(), PlannedTile::from(Road).with_min_rcl(SOURCE_AND_CONTROLLER_ROAD_RCL));

    result.set((0, 3).try_into().unwrap(), PlannedTile::from(Road).with_min_rcl(SOURCE_AND_CONTROLLER_ROAD_RCL));
    result.set((1, 3).try_into().unwrap(), PlannedTile::from(Spawn).with_min_rcl(1));
    result.set((2, 3).try_into().unwrap(), PlannedTile::from(Extension).with_min_rcl(2));
    result.set(
        (3, 3).try_into().unwrap(),
        PlannedTile::from(Container).with_reserved(true).with_min_rcl(MIN_CONTAINER_RCL),
    );
    result.set((4, 3).try_into().unwrap(), PlannedTile::from(Extension).with_min_rcl(4));
    result.set((5, 3).try_into().unwrap(), PlannedTile::from(Spawn).with_min_rcl(7));
    result.set((6, 3).try_into().unwrap(), PlannedTile::from(Road).with_min_rcl(SOURCE_AND_CONTROLLER_ROAD_RCL));

    result.set((0, 4).try_into().unwrap(), PlannedTile::from(Road).with_min_rcl(SOURCE_AND_CONTROLLER_ROAD_RCL));
    result.set((1, 4).try_into().unwrap(), PlannedTile::from(Extension).with_min_rcl(2));
    result.set((2, 4).try_into().unwrap(), PlannedTile::default().with_reserved(true));
    result.set((3, 4).try_into().unwrap(), PlannedTile::from(Extension).with_min_rcl(2));
    result.set((4, 4).try_into().unwrap(), PlannedTile::default().with_reserved(true));
    result.set((5, 4).try_into().unwrap(), PlannedTile::from(Extension).with_min_rcl(3));
    result.set((6, 4).try_into().unwrap(), PlannedTile::from(Road).with_min_rcl(SOURCE_AND_CONTROLLER_ROAD_RCL));

    result.set((0, 5).try_into().unwrap(), PlannedTile::from(Road).with_min_rcl(SOURCE_AND_CONTROLLER_ROAD_RCL));
    result.set((1, 5).try_into().unwrap(), PlannedTile::from(Extension).with_min_rcl(2));
    result.set((2, 5).try_into().unwrap(), PlannedTile::from(Extension).with_min_rcl(2));
    result.set((3, 5).try_into().unwrap(), PlannedTile::from(Spawn).with_min_rcl(8));
    result.set((4, 5).try_into().unwrap(), PlannedTile::from(Extension).with_min_rcl(3));
    result.set((5, 5).try_into().unwrap(), PlannedTile::from(Extension).with_min_rcl(4));
    result.set((6, 5).try_into().unwrap(), PlannedTile::from(Road).with_min_rcl(SOURCE_AND_CONTROLLER_ROAD_RCL));

    result.set((1, 6).try_into().unwrap(), PlannedTile::from(Road).with_min_rcl(SOURCE_AND_CONTROLLER_ROAD_RCL));
    result.set((2, 6).try_into().unwrap(), PlannedTile::from(Road).with_min_rcl(SOURCE_AND_CONTROLLER_ROAD_RCL));
    result.set((3, 6).try_into().unwrap(), PlannedTile::from(Road).with_min_rcl(SOURCE_AND_CONTROLLER_ROAD_RCL));
    result.set((4, 6).try_into().unwrap(), PlannedTile::from(Road).with_min_rcl(SOURCE_AND_CONTROLLER_ROAD_RCL));
    result.set((5, 6).try_into().unwrap(), PlannedTile::from(Road).with_min_rcl(SOURCE_AND_CONTROLLER_ROAD_RCL));

    result.map(|xy, tile| {
        if !tile.is_empty() {
            tile.with_base_part(BasePart::Interior)
        } else {
            tile
        }
    })
}

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
        if !tile.is_empty() {
            tile.with_base_part(BasePart::Interior)
        } else {
            tile
        }
    })
}
