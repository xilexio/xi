use crate::algorithms::binary_search::{upper_bound, upper_bound_by_key};
use crate::algorithms::chunk_graph::chunk_graph;
use crate::algorithms::distance_matrix::{count_restricted_distance_matrix, distance_matrix};
use crate::algorithms::distance_transform::{distance_transform_from_obstacles, l1_distance_transform_from_obstacles};
use crate::algorithms::grid_min_cut::grid_min_cut;
use crate::algorithms::interior_matrix::interior_matrix;
use crate::algorithms::matrix_common::MatrixCommon;
use crate::algorithms::max_boundary_distance::max_boundary_distance;
use crate::algorithms::room_matrix::RoomMatrix;
use crate::algorithms::room_matrix_slice::RoomMatrixSlice;
use crate::algorithms::weighted_distance_matrix::{obstacle_cost, unreachable_cost};
use crate::consts::{OBSTACLE_COST, UNREACHABLE_COST};
use crate::geometry::rect::{ball, bounding_rect, room_rect};
use crate::geometry::room_xy::RoomXYUtils;
use crate::room_planner::packed_tile_structures::{MainStructureType, PackedTileStructures};
use crate::room_planner::plan::Plan;
use crate::room_planner::planned_tile::PlannedTile;
use crate::room_planner::roads::{connect_with_roads, RoadTarget};
use crate::room_planner::stamps::{core_stamp, labs_stamp};
use crate::room_planner::RoomPlannerError::{
    ControllerNotFound, ResourceNotFound, StructurePlacementFailure, UnreachableResource,
};
use crate::room_state::RoomState;
use crate::visualization::visualize;
use crate::visualization::Visualization::{Graph, Matrix};
use log::debug;
use num_traits::clamp;
use screeps::StructureType::{Extension, Rampart, Road, Spawn, Storage};
use screeps::Terrain::Wall;
use screeps::{game, RoomXY};
use std::cmp::{max, min};
use std::collections::BTreeMap;
use std::error::Error;
use std::iter::once;
use thiserror::Error;

pub mod packed_tile_structures;
pub mod plan;
pub mod planned_tile;
pub mod roads;
pub mod stamps;

const APPROXIMATE_BASE_TILES: u16 = 140;
const SOURCE_DIST_WEIGHT: f32 = 2.0;
const MINERAL_DIST_WEIGHT: f32 = 1.0;
const CONTROLLER_DIST_WEIGHT: f32 = 1.5;
const RESOURCES_DIST_PERCENTILE_CUTOFF: f32 = 0.1;
const MIN_RESOURCE_CENTERS: usize = 25;
const CHUNK_RADIUS: u8 = 5;
const MAX_LABS_DIST: u8 = 12;
// TODO take into account rampart radius - we prefer if ramparts are squeezed in one area

#[derive(Error, Debug, Eq, PartialEq)]
pub enum RoomPlannerError {
    #[error("controller not found")]
    ControllerNotFound,
    #[error("at least one source or mineral not found")]
    ResourceNotFound,
    #[error("one of sources, the mineral or the controller is unreachable")]
    UnreachableResource,
    #[error("unable to find positions for all required structures")]
    StructurePlacementFailure,
    #[error("failed to connect some points with roads")]
    RoadConnectionFailure,
}

pub struct RoomPlanner {
    state: RoomState,
}

impl RoomPlanner {
    pub fn new(state: &RoomState) -> RoomPlanner {
        RoomPlanner { state: state.clone() }
    }

    /// Creates the room plan.
    /// A good place for the core is one that balances the following:
    /// - the number of ramparts required to protect the base,
    /// - maximum distance to ramparts from spawns and storage,
    /// - distance from the nearest spawn to sources, controller and mineral,
    /// - distance between ramparts to maximize minimal tower damage right outside of ramparts.
    pub fn plan(&mut self) -> Result<Plan, Box<dyn Error>> {
        // Preliminary checks of the room.
        let controller = self.state.controller.ok_or(ControllerNotFound)?;
        if self.state.sources.is_empty() {
            Err(ResourceNotFound)?;
        }
        let sources = self.state.sources.clone();
        let mineral = self.state.mineral.ok_or(ResourceNotFound)?;


        // Finding distances from various room features and initializing data structures.
        let walls = self.state.terrain.walls().collect::<Vec<_>>();
        let controller_dm = distance_matrix(walls.iter().copied(), once(controller.xy));
        let source_dms = sources
            .iter()
            .map(|source| distance_matrix(walls.iter().copied(), once(source.xy)))
            .collect::<Vec<_>>();
        let mineral_dm = distance_matrix(walls.iter().copied(), once(mineral.xy));
        let exits = room_rect()
            .boundary()
            .filter_map(|xy| (self.state.terrain.get(xy) != Wall).then_some(xy))
            .collect::<Vec<_>>();
        let exits_dm = distance_matrix(walls.iter().copied(), exits.iter().copied());
        // Distance transform in maximum metric.
        let dt = distance_transform_from_obstacles(walls.iter().copied());
        // Distance transform in l1 metric.
        let dt_l1 = l1_distance_transform_from_obstacles(walls.iter().copied());
        // Chunk graph.
        let walls_matrix = self.state.terrain.to_obstacle_matrix(0);
        let chunks = chunk_graph(&walls_matrix, CHUNK_RADIUS);

        // visualize(
        //     self.state.name,
        //     Matrix(Box::new(chunks.xy_chunks.map(|_, id| id.index() as u8))),
        // );
        // visualize(self.state.name, Graph(chunks.graph));
        // Err(UnreachableResource)?;

        // TODO Perform theoretical calculations on good weights, include mineral in them.
        let resources_dist_sum = {
            let mut preliminary_sum = RoomMatrix::new(0.0f32);
            let resource_dms_and_weights = [
                (&controller_dm, CONTROLLER_DIST_WEIGHT),
                (&mineral_dm, MINERAL_DIST_WEIGHT),
            ]
            .into_iter()
            .chain(source_dms.iter().map(|dm| (dm, SOURCE_DIST_WEIGHT)));
            for (dm, weight) in resource_dms_and_weights {
                preliminary_sum.update(|xy, value| {
                    let dm_value = dm.get(xy);
                    if dm_value >= UNREACHABLE_COST {
                        f32::INFINITY
                    } else {
                        value + (dm.get(xy) as f32) * weight
                    }
                });
            }
            let max_finite_value =
                preliminary_sum
                    .iter()
                    .fold(1.0, |acc, (_, v)| if v != f32::INFINITY && v > acc { v } else { acc });
            preliminary_sum.map(|xy, value| {
                if value.is_finite() {
                    (value / max_finite_value * 250.0).round() as u8
                } else {
                    OBSTACLE_COST
                }
            })
        };
        // Finding only resource centers where the core can fit.
        let mut resource_centers = resources_dist_sum
            .iter()
            .filter_map(|(xy, value)| {
                (exits_dm.get(xy) >= 5 && value != OBSTACLE_COST && self.core_fits(&dt, xy)).then_some((xy, value))
            })
            .collect::<Vec<_>>();
        if resource_centers.len() == 0 {
            Err(UnreachableResource)?
        }
        // Finite f32 have a sound order.
        resource_centers.sort_by_key(|&(_, value)| value);
        // visualize(self.state.name, Matrix(Box::new(resources_dist_sum)));
        let resource_center_dist_sum_cutoff =
            resource_centers[(resource_centers.len() as f32 * RESOURCES_DIST_PERCENTILE_CUTOFF) as usize].1;
        let number_of_good_resource_centers = min(
            max(
                MIN_RESOURCE_CENTERS,
                upper_bound_by_key(&resource_centers, resource_center_dist_sum_cutoff, |&(_, value)| value),
            ),
            resource_centers.len(),
        );
        debug!("Found {} good resource centers.", number_of_good_resource_centers);


        // // Performing a preliminary min-cut by finding the area to protect.
        // // It includes the selected approximate_base_center and 140 fields near it.
        // // TODO Set weights that depend on the distance.
        // let approximate_base_center = resource_centers[0].0;
        // debug!("approximate_base_center {}", approximate_base_center);
        // let mut approximate_base_min_cut_matrix = RoomMatrix::new(1);
        // for &xy in walls.iter() {
        //     approximate_base_min_cut_matrix.set(xy, OBSTACLE_COST);
        // }
        // for dm in once(&controller_dm).chain(source_dms.iter()) {
        //     for xy in shortest_path_by_matrix_with_preference(dm, &exits_dm, approximate_base_center, 1).into_iter() {
        //         // TODO Also include 2 tiles form the road for safety from ranged attacks.
        //         approximate_base_min_cut_matrix.set(xy, 0);
        //     }
        // }
        // // TODO Make these 140 fields be selected away from exits with some weight.
        // let tiles_near_exits = exits_dm.iter().filter_map(|(xy, dist)| (dist <= 4).then_some(xy));
        // // for xy in exits_dm.iter().filter_map(|(xy, dist)| (dist <= 4).then_some(xy)) {
        // //     planned_tiles.set(xy, Extension.into());
        // // }
        // let approximate_base_dm =
        //     count_restricted_distance_matrix(walls.iter().copied().chain(tiles_near_exits), approximate_base_center, APPROXIMATE_BASE_TILES);
        // for (xy, _) in approximate_base_dm
        //     .iter()
        //     .filter(|&(xy, value)| value < UNREACHABLE_COST)
        // {
        //     approximate_base_min_cut_matrix.set(xy, 0);
        //     planned_tiles.set(xy, Extension.into());
        // }
        // let preliminary_ramparts = grid_min_cut(&approximate_base_min_cut_matrix);
        // for &xy in preliminary_ramparts.iter() {
        //     planned_tiles.set(xy, Rampart.into());
        // }

        // Creating the plan and placing the core in a good location.
        let mut planned_tiles = resource_centers
            .iter()
            .take(number_of_good_resource_centers)
            .copied()
            .find_map(|(xy, score)| {
                debug!("Processing resource center {} with score {}.", xy, score);

                (0..4).find_map(|rotations| {
                    debug!("Processing core with {} rotations.", rotations);
                    let mut planned_tiles = RoomMatrix::new(PlannedTile::default());

                    let core = {
                        let mut stamp_matrix = core_stamp();
                        stamp_matrix.translate(xy.sub(stamp_matrix.rect.center())).unwrap();
                        stamp_matrix.rotate(rotations).unwrap();
                        stamp_matrix
                    };
                    planned_tiles.merge_from(&core, |old, new| old.merge(new));

                    // First attempt in which good places to grow towards are not known.

                    // Placing the labs as close to the storage as possible.
                    let storage_xy = core
                        .iter()
                        .find_map(|(xy, tile)| (tile.structures() == Storage.into()).then_some(xy))
                        .unwrap();
                    let labs = storage_xy
                        .outward_iter(Some(3), Some(MAX_LABS_DIST))
                        .find_map(|lab_xy| {
                            if self.labs_fit(&dt_l1, lab_xy) {
                                debug!("Trying labs at {}.", lab_xy);
                                let mut labs = labs_stamp();
                                labs.translate(lab_xy.sub(labs.rect.center())).unwrap();
                                if min(labs.rect.top_right().dist(storage_xy), labs.rect.bottom_right.dist(storage_xy))
                                    < min(labs.rect.top_left.dist(storage_xy), labs.rect.bottom_left().dist(storage_xy))
                                {
                                    debug!("Rotating the labs.");
                                    labs.rotate(1).unwrap();
                                }
                                if labs.iter().all(|(xy, structure)| {
                                    let existing_structure = planned_tiles.get(xy);
                                    existing_structure.is_empty() || existing_structure == structure
                                }) {
                                    Some(labs)
                                } else {
                                    None
                                }
                            } else {
                                None
                            }
                        })?;
                    planned_tiles.merge_from(&labs, |old, new| old.merge(new));

                    // Connecting the labs to the storage.
                    // TODO include this in labs creation and make it a part of the score to decide labs' placement
                    let closest_lab_road = {
                        let mut lab_roads = labs
                            .iter()
                            .filter_map(|(xy, tile)| tile.structures().road().then_some(xy))
                            .collect::<Vec<_>>();
                        lab_roads.sort_by_key(|&xy| xy.dist(storage_xy));
                        lab_roads[0]
                    };
                    connect_with_roads(
                        &self.state.terrain,
                        &mut planned_tiles,
                        once(closest_lab_road),
                        1,
                        once(RoadTarget::new(storage_xy, 1, true)),
                        storage_xy,
                    )
                    .ok()?;

                    // After placing the stamps, creating the shortest routes from spawns to mineral, sources and
                    // controller.
                    let spawns = core
                        .iter()
                        .filter_map(|(xy, tile)| (tile.structures() == Spawn.into()).then_some(xy))
                        .collect::<Vec<_>>();
                    connect_with_roads(
                        &self.state.terrain,
                        &mut planned_tiles,
                        spawns.iter().copied(),
                        1,
                        once(RoadTarget::new(controller.xy, 4, true)).chain(
                            sources
                                .iter()
                                .copied()
                                .map(|source_info| RoadTarget::new(source_info.xy, 2, true)),
                        ),
                        storage_xy,
                    )
                    .ok()?;
                    // Creating the shortest route to the mineral from storage. It may be outside of ramparts.
                    connect_with_roads(
                        &self.state.terrain,
                        &mut planned_tiles,
                        once(storage_xy),
                        1,
                        once(RoadTarget::new(mineral.xy, 2, false)),
                        storage_xy,
                    )
                    .ok()?;

                    // TODO improve this part by not requiring the whole area, also add the link
                    for xy in [mineral.xy, controller.xy].into_iter().chain(sources.iter().map(|source| source.xy)) {
                        for near in xy.around() {
                            if planned_tiles.get(near).is_empty() {
                                planned_tiles.set(near, planned_tiles.get(near).with_reserved(true));
                            }
                        }
                    }

                    self.place_extensions(walls.iter().copied(), storage_xy, &mut planned_tiles)
                        .ok()?;

                    let distances_from_structures = distance_matrix(
                        walls.iter().copied(),
                        planned_tiles
                            .iter()
                            .filter_map(|(xy, tile)| tile.interior().then_some(xy)),
                    );

                    // TODO include all fields around the controller with a path to them

                    let base_min_cut_matrix = distances_from_structures.map(|xy, dist| {
                        if self.state.terrain.get(xy) == Wall {
                            obstacle_cost()
                        } else if dist <= 2 {
                            0
                        } else {
                            10 + ((dist as f32).sqrt() as u8)
                        }
                    });
                    let min_cut = grid_min_cut(&base_min_cut_matrix);
                    for xy in min_cut.iter().copied() {
                        planned_tiles.set(xy, Rampart.into());
                    }

                    Some(planned_tiles)
                })
            })
            .ok_or(StructurePlacementFailure)?;

        Ok(Plan { planned_tiles })
    }

    #[inline]
    fn core_fits(&self, dt: &RoomMatrix<u8>, xy: RoomXY) -> bool {
        let center_dt_dist = dt.get(xy);
        if center_dt_dist >= 4 {
            true
        } else if center_dt_dist < 3 {
            false
        } else {
            unsafe {
                dt.get(xy.add_diff((0, -1))) >= 3
                    && dt.get(xy.add_diff((1, 0))) >= 3
                    && dt.get(xy.add_diff((0, 1))) >= 3
                    && dt.get(xy.add_diff((-1, 0))) >= 3
            }
        }
    }

    #[inline]
    fn labs_fit(&self, dt_l1: &RoomMatrix<u8>, xy: RoomXY) -> bool {
        // Labs need a plus, but have no center due to even width.
        // . L L .
        // L R L L
        // L L R L
        // . L L .
        // Note that once the first dt_l1 below passes, adding the diff is correct.
        dt_l1.get(xy) >= 2
            && unsafe {
                dt_l1.get(xy.add_diff((0, 1))) >= 2
                    && dt_l1.get(xy.add_diff((1, 0))) >= 2
                    && dt_l1.get(xy.add_diff((1, 1))) >= 2
            }
    }

    fn place_extensions(
        &mut self,
        walls: impl Iterator<Item = RoomXY>,
        storage_xy: RoomXY,
        planned_tiles: &mut RoomMatrix<PlannedTile>,
    ) -> Result<(), Box<dyn Error>> {
        let obstacles = planned_tiles
            .iter()
            .filter_map(|(xy, structure)| (!structure.is_passable(true)).then_some(xy))
            .chain(walls);
        let storage_dm = distance_matrix(obstacles, once(storage_xy));

        debug!("Placing extensions.");

        // Finding scores of extensions. The lower, the better. The most important factor is the distance from storage.
        let tile_score = storage_dm.map(|xy, dist| {
            if dist >= unreachable_cost() || !planned_tiles.get(xy).is_empty() || xy.exit_distance() < 2 {
                obstacle_cost()
            } else {
                dist
            }
        });

        // An algorithm which grows extensions and roads like roots. Based on a priority queue of scores of empty tiles
        // in which extensions may be placed and of tiles with extensions which may be removed to give access to more
        // tiles for other extensions.
        // The score of an empty tile is defined above. The score of an already placed tile requires balancing loss of
        // score from a closer tile to exchange it for a few farther tiles. It is equal to twice the mean score of
        // empty tiles around minus the score of the removed tile. However, if there is only a single empty tile around,
        // it is three times that tile's score minus the removed tile's score.
        let mut i = 0u16;
        let mut priority_queue = BTreeMap::new();
        for xy in tile_score.find_not_xy(obstacle_cost()) {
            if xy.around().any(|near| planned_tiles.get(near).structures().road()) {
                // Keeping tile position and whether it is an empty tile.
                priority_queue.insert((tile_score.get(xy), i), (xy, true));
                i += 1;
            }
        }

        fn avg_around_score(planned_tiles: &RoomMatrix<PlannedTile>, tile_score: &RoomMatrix<u8>, xy: RoomXY) -> u8 {
            let mut total_score_around = 0u16;
            let mut empty_tiles_around = 0u8;
            for near in xy.around() {
                let near_score = tile_score.get(near);
                if near_score != OBSTACLE_COST && planned_tiles.get(near).is_empty() {
                    total_score_around += near_score as u16;
                    empty_tiles_around += 1;
                }
            }

            if empty_tiles_around > 0 {
                let multiplier = if empty_tiles_around == 1 { 3 } else { 2 };
                clamp(
                    multiplier * total_score_around / (empty_tiles_around as u16),
                    0,
                    OBSTACLE_COST as u16 - 1,
                ) as u8
            } else {
                OBSTACLE_COST
            }
        }

        let mut remaining_extensions = 68
            - planned_tiles
                .iter()
                .filter(|(xy, tile)| tile.structures() == Extension.into())
                .count();
        while remaining_extensions > 0 && !priority_queue.is_empty() {
            let ((xy_score, _), (xy, placement)) = priority_queue.pop_first().unwrap();
            debug!("[{}] {}: {}, {}", remaining_extensions, xy_score, xy, placement);
            if placement {
                planned_tiles.set(xy, Extension.into());
                let current_score = tile_score.get(xy);

                let removal_score = avg_around_score(planned_tiles, &tile_score, xy).saturating_sub(current_score);

                if removal_score < OBSTACLE_COST {
                    priority_queue.insert((removal_score, i), (xy, false));
                    i += 1;
                    debug!("  + {}: {}, {}", removal_score, xy, false);
                }

                remaining_extensions -= 1;
            } else {
                let current_score = tile_score.get(xy);
                let removal_score = avg_around_score(planned_tiles, &tile_score, xy).saturating_sub(current_score);

                if removal_score != xy_score {
                    // If the score changed as a result of, e.g., removing some empty tiles around, we re-queue the
                    // tile.
                    priority_queue.insert((removal_score, i), (xy, false));
                    i += 1;
                    debug!(" => {}: {}, {}", removal_score, xy, false);
                } else {
                    planned_tiles.set(xy, Road.into());

                    for near in xy.around() {
                        if tile_score.get(near) != OBSTACLE_COST && planned_tiles.get(near).is_empty() {
                            let score = tile_score.get(near);
                            priority_queue.insert((score, i), (near, true));
                            debug!("  + {}: {}, {}", score, near, true);
                            i += 1;
                        }
                    }

                    remaining_extensions += 1;
                }
            }
        }

        // TODO place extension when there is a close place
        // if there are at least 3 extensions to reach with a single road, place it, replacing an extension
        // !! keep number of surrounding extensions per tile
        // total score is average distance to extensions (and if possible clumpiness - no lone extensions)

        Ok(())
    }
}
