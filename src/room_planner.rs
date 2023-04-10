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
use crate::algorithms::shortest_path_by_matrix::shortest_path_by_matrix_with_preference;
use crate::consts::{OBSTACLE_COST, UNREACHABLE_COST};
use crate::geometry::rect::{ball, bounding_rect, room_rect};
use crate::geometry::room_xy::RoomXYUtils;
use crate::room_planner::packed_tile_structures::PackedTileStructures;
use crate::room_planner::plan::Plan;
use crate::room_planner::roads::connect_with_roads;
use crate::room_planner::stamps::{core_stamp, labs_stamp};
use crate::room_planner::RoomPlannerError::{
    ControllerNotFound, ResourceNotFound, StructurePlacementFailure, UnreachableResource,
};
use crate::room_state::RoomState;
use crate::visualization::visualize;
use crate::visualization::Visualization::{Graph, Matrix};
use log::debug;
use num_traits::clamp;
use screeps::game::spawns;
use screeps::StructureType::{Extension, PowerSpawn, Rampart, Road, Spawn, Storage};
use screeps::Terrain::Wall;
use screeps::{game, RoomXY};
use std::cmp::{max, min};
use std::collections::BTreeMap;
use std::error::Error;
use std::iter::once;
use thiserror::Error;

pub mod packed_tile_structures;
pub mod plan;
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
        // The matrix for the plan.
        let mut structures_matrix = RoomMatrix::new(PackedTileStructures::default());

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
        // //     structures_matrix.set(xy, Extension.into());
        // // }
        // let approximate_base_dm =
        //     count_restricted_distance_matrix(walls.iter().copied().chain(tiles_near_exits), approximate_base_center, APPROXIMATE_BASE_TILES);
        // for (xy, _) in approximate_base_dm
        //     .iter()
        //     .filter(|&(xy, value)| value < UNREACHABLE_COST)
        // {
        //     approximate_base_min_cut_matrix.set(xy, 0);
        //     structures_matrix.set(xy, Extension.into());
        // }
        // let preliminary_ramparts = grid_min_cut(&approximate_base_min_cut_matrix);
        // for &xy in preliminary_ramparts.iter() {
        //     structures_matrix.set(xy, Rampart.into());
        // }

        // Creating the plan and placing the core in a good location.
        structures_matrix = resource_centers
            .iter()
            .take(number_of_good_resource_centers)
            .copied()
            .find_map(|(xy, score)| {
                debug!("Processing resource center {} with score {}.", xy, score);

                (0..4).find_map(|rotations| {
                    debug!("Processing core with {} rotations.", rotations);
                    let mut structures_matrix = RoomMatrix::new(PackedTileStructures::default());

                    let core = {
                        let mut stamp_matrix = core_stamp();
                        stamp_matrix.translate(xy.sub(stamp_matrix.rect.center())).unwrap();
                        stamp_matrix.rotate(rotations).unwrap();
                        stamp_matrix
                    };
                    structures_matrix.merge_from(&core);

                    // First attempt in which good places to grow towards are not known.

                    // Placing the labs as close to the storage as possible.
                    let storage_xy = core.find_xy(Storage.into()).next().unwrap();
                    let labs = storage_xy
                        .outward_iter(Some(3), Some(MAX_LABS_DIST))
                        .find_map(|lab_xy| {
                            if self.labs_fit(&dt_l1, lab_xy) {
                                debug!("Trying labs at {}.", lab_xy);
                                let mut labs = labs_stamp();
                                let lab_centers = labs.rect.centers();
                                labs.translate(lab_xy.sub(lab_centers[0])).unwrap();
                                if min(lab_centers[1].dist(storage_xy), lab_centers[3].dist(storage_xy))
                                    < min(lab_centers[0].dist(storage_xy), lab_centers[2].dist(storage_xy))
                                {
                                    debug!("Rotating the labs.");
                                    labs.rotate(1).unwrap();
                                }
                                if labs.iter().all(|(xy, structure)| {
                                    let existing_structure = structures_matrix.get(xy);
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
                    structures_matrix.merge_from(&labs);

                    // Connecting the labs to the storage.
                    // TODO include this in labs creation and make it a part of the score to decide labs' placement
                    let closest_lab_road = {
                        let mut lab_roads = labs.find_xy(Road.into()).collect::<Vec<_>>();
                        lab_roads.sort_by_key(|&xy| xy.dist(storage_xy));
                        lab_roads[0]
                    };
                    connect_with_roads(
                        walls.iter().copied(),
                        &mut structures_matrix,
                        once(closest_lab_road),
                        once(storage_xy),
                    )
                    .ok()?;

                    // After placing the core, creating the shortest routes from spawns to mineral, sources and
                    // controller. Mineral needs convenient access to storage too to haul it, but it has smaller
                    // priority.
                    let spawns = core.find_xy(Spawn.into()).collect::<Vec<_>>();
                    connect_with_roads(
                        walls.iter().copied(),
                        &mut structures_matrix,
                        spawns.iter().copied(),
                        [controller.xy, mineral.xy]
                            .into_iter()
                            .chain(sources.iter().copied().map(|source_info| source_info.xy)),
                    )
                    .ok()?;

                    Some(structures_matrix)
                })
            })
            .ok_or(StructurePlacementFailure)?;
        // structures_matrix.merge_from(&core);


        // Adding the labs close to the core.
        // let labs = self.place_labs(&dt_l1, &core)?;
        // structures_matrix.merge_from(&labs);


        // Connecting everything with roads.
        // let spawns = core
        //     .find_xy(Spawn.into())
        //     .chain(fast_filler.find_xy(Spawn.into()))
        //     .collect::<Vec<_>>();
        // Connecting nearest spawn to sources and mineral.
        // connect_with_roads(
        //     walls.iter().copied(),
        //     &mut structures_matrix,
        //     spawns.iter().copied(),
        //     once(mineral.xy).chain(sources.iter().copied().map(|source_info| source_info.xy)),
        // )?;
        // Connecting storage to labs, spawns and controller.
        // TODO it is okay for these roads to be a bit longer so that less roads are built
        // let core_storage = core.find_xy(Storage.into()).next().unwrap();
        // connect_with_roads(
        //     walls.iter().copied(),
        //     &mut structures_matrix,
        //     once(core_storage),
        //     once(controller.xy)
        //         .chain(spawns.iter().copied())
        //         .chain(labs.find_xy(Road.into())),
        // )?;
        // Connecting spawns to labs.
        // TODO it is okay for this road to be a bit longer so that less roads are built
        // connect_with_roads(
        //     walls.iter().copied(),
        //     &mut structures_matrix,
        //     labs.find_xy(Road.into()),
        //     core.find_xy(Spawn.into()).chain(fast_filler.find_xy(Spawn.into())),
        // )?;


        // self.place_extensions(walls.iter().copied(), core_storage, &mut structures_matrix)?;

        // TODO roads around ff and core, spawns -> storage, mineral/source -> nearest spawn
        // TODO place ff closer to sources/mineral/controller and not rampart midpoint
        // TODO fill general area around spawn with extensions (more than needed - leave place for towers), then dig into them to reach more
        // TODO 60 - 17 = 43 extensions, 6 towers, labs, observer

        Ok(Plan {
            structures: structures_matrix.to_structures_map(),
        })
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

    // TODO move this info to main function docs
    // TODO just rotate the core accordingly
    fn place_core(
        &mut self,
        dt: &RoomMatrix<u8>,
        walls: &Vec<RoomXY>,
        resource_centers: &Vec<(RoomXY, u8)>,
        resource_center_dist_sum_cutoff: u8,
    ) -> Result<RoomMatrixSlice<PackedTileStructures>, Box<dyn Error>> {
        // let preliminary_rampart_rect = bounding_rect(preliminary_ramparts.iter().copied());
        // let preliminary_rampart_max_distances = max_boundary_distance(preliminary_rampart_rect);
        //
        // let interior = interior_matrix(walls.iter().copied(), preliminary_ramparts.iter().copied());
        //
        // core_param_candidates.sort_by_key(|&(_, _, dist)| dist);
        // if core_param_candidates.is_empty() {
        //     Err(StructurePlacementFailure)?;
        // }

        // let mut data = RoomMatrix::new(255);
        // core_param_candidates.iter().for_each(|(xy, dir, dist)| {
        //     if dir.contains(&0) {
        //         data.set(*xy, *dist);
        //     }
        // });
        // visualize(self.state.name, Matrix(Box::new(data)));

        // let candidate_ix = (game::time() as usize / 2) % min(core_param_candidates.len(), 100);
        // let core_params = core_param_candidates[candidate_ix].clone();
        // debug!("core params: {:?}", core_params);
        // let core_center_offset = core_params.0.sub((2, 2).try_into().unwrap());
        let mut core = core_stamp();

        // let rotation_ix = (game::time() as usize / 2) % core_params.1.len();
        // core.rotate(core_params.1[rotation_ix]).unwrap();
        // core.translate(core_center_offset).unwrap();
        Ok(core)
    }

    // fn place_fast_filler(
    //     &mut self,
    //     dt: &RoomMatrix<u8>,
    //     core: &RoomMatrixSlice<PackedTileStructures>,
    // ) -> Result<RoomMatrixSlice<PackedTileStructures>, Box<dyn Error>> {
    //     let core_center = core.rect.center();
    //
    //     let mut fast_filler_param_candidates = ball(core_center, self.max_fast_filler_distance)
    //         .iter()
    //         .filter_map(|xy| {
    //             if xy.dist(core_center) >= 5 && xy.exit_distance() >= 6 && dt.get(xy) >= 3 {
    //                 // Fast filler needs partial roads on 3 sides.
    //                 // . R R R R R .
    //                 // R E E E E E R
    //                 // R E . E . E R
    //                 // R S E . E S R
    //                 // . E . E . E .
    //                 // . E E E E E .
    //                 let place_for_long_roads = unsafe {
    //                     [
    //                         dt.get(xy.add_diff((0, -1))) >= 3,
    //                         dt.get(xy.add_diff((1, 0))) >= 3,
    //                         dt.get(xy.add_diff((0, 1))) >= 3,
    //                         dt.get(xy.add_diff((-1, 0))) >= 3,
    //                     ]
    //                 };
    //                 let place_for_long_roads_count =
    //                     place_for_long_roads
    //                         .iter()
    //                         .fold(0u8, |acc, &b| if b { acc + 1 } else { acc });
    //                 if place_for_long_roads_count == 4 {
    //                     Some((xy, vec![0u8, 1u8, 2u8, 3u8]))
    //                 } else {
    //                     let place_for_roads = unsafe {
    //                         [
    //                             place_for_long_roads[0]
    //                                 && dt.get(xy.add_diff((-2, -1))) >= 3
    //                                 && dt.get(xy.add_diff((2, -1))) >= 3,
    //                             place_for_long_roads[1]
    //                                 && dt.get(xy.add_diff((1, -2))) >= 3
    //                                 && dt.get(xy.add_diff((1, 2))) >= 3,
    //                             place_for_long_roads[2]
    //                                 && dt.get(xy.add_diff((-2, 1))) >= 3
    //                                 && dt.get(xy.add_diff((2, 1))) >= 3,
    //                             place_for_long_roads[3]
    //                                 && dt.get(xy.add_diff((-1, -2))) >= 3
    //                                 && dt.get(xy.add_diff((-1, 2))) >= 3,
    //                         ]
    //                     };
    //                     let rotations = [0u8, 1u8, 2u8, 3u8]
    //                         .into_iter()
    //                         .filter(|&i| place_for_roads[i as usize])
    //                         .collect::<Vec<_>>();
    //                     if !rotations.is_empty() {
    //                         Some((xy, rotations))
    //                     } else {
    //                         None
    //                     }
    //                 }
    //             } else {
    //                 None
    //             }
    //         })
    //         .collect::<Vec<_>>();
    //     fast_filler_param_candidates.sort_by_key(|&(xy, _)| xy.dist(core_center));
    //     if fast_filler_param_candidates.is_empty() {
    //         Err(StructurePlacementFailure)?;
    //     }
    //
    //     // TODO fast filler may be glued to the core on its side without roads
    //
    //     let candidate_ix = (game::time() as usize / 2) % min(fast_filler_param_candidates.len(), 20);
    //     let fast_filler_params = fast_filler_param_candidates[candidate_ix].clone();
    //     debug!("fast filler params: {:?}", fast_filler_params);
    //     let fast_filler_center_offset = fast_filler_params.0.sub((2, 2).try_into().unwrap());
    //     let mut fast_filler = fast_filler_stamp();
    //
    //     let rotation_ix = (game::time() as usize / 2) % fast_filler_params.1.len();
    //     fast_filler.rotate(fast_filler_params.1[rotation_ix]).unwrap();
    //     fast_filler.translate(fast_filler_center_offset).unwrap();
    //     Ok(fast_filler)
    // }

    fn place_labs(
        &mut self,
        dt_l1: &RoomMatrix<u8>,
        core: &RoomMatrixSlice<PackedTileStructures>,
    ) -> Result<RoomMatrixSlice<PackedTileStructures>, Box<dyn Error>> {
        let core_center = core.rect.center();

        let mut labs_param_candidates = ball(core_center, MAX_LABS_DIST)
            .iter()
            .filter_map(|xy| {
                if xy.dist(core_center) >= 4 && xy.exit_distance() >= 6 && dt_l1.get(xy) >= 2 {
                    // Labs need a plus, but have no center due to even width.
                    // . L L .
                    // L R L L
                    // L L R L
                    // . L L .
                    let can_build = unsafe {
                        dt_l1.get(xy.add_diff((0, 1))) >= 2
                            && dt_l1.get(xy.add_diff((1, 0))) >= 2
                            && dt_l1.get(xy.add_diff((1, 1))) >= 2
                    };
                    can_build.then_some(xy)
                } else {
                    None
                }
            })
            .collect::<Vec<_>>();
        labs_param_candidates.sort_by_key(|&xy| unsafe {
            min(
                min(xy.dist(core_center), xy.add_diff((0, 1)).dist(core_center)),
                min(
                    xy.add_diff((1, 0)).dist(core_center),
                    xy.add_diff((1, 1)).dist(core_center),
                ),
            )
        });
        if labs_param_candidates.is_empty() {
            Err(StructurePlacementFailure)?;
        }

        let candidate_ix = (game::time() as usize / 2) % min(labs_param_candidates.len(), 20);
        let labs_params = labs_param_candidates[candidate_ix].clone();
        debug!("labs params: {:?}", labs_params);
        let labs_top_left_center_offset = labs_params.sub((1, 1).try_into().unwrap());
        let mut labs = labs_stamp();

        let rotations = ((game::time() as usize / 2) % 2) as u8;
        labs.rotate(rotations).unwrap();
        labs.translate(labs_top_left_center_offset).unwrap();
        Ok(labs)
    }

    fn place_extensions(
        &mut self,
        walls: impl Iterator<Item = RoomXY>,
        storage_xy: RoomXY,
        structures_matrix: &mut RoomMatrix<PackedTileStructures>,
    ) -> Result<(), Box<dyn Error>> {
        let obstacles = structures_matrix
            .iter()
            .filter_map(|(xy, structure)| (!structure.is_passable(true)).then_some(xy))
            .chain(walls);
        let storage_dm = distance_matrix(obstacles, once(storage_xy));

        debug!("Placing extensions.");

        // Finding scores of extensions. The lower, the better. The most important factor is the distance from storage.
        let tile_score = storage_dm.map(|xy, dist| {
            if dist >= UNREACHABLE_COST || !structures_matrix.get(xy).is_empty() || xy.exit_distance() < 2 {
                OBSTACLE_COST
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
        for xy in tile_score.find_not_xy(OBSTACLE_COST) {
            if xy.around().any(|near| !structures_matrix.get(near).road().is_empty()) {
                // Keeping tile position and whether it is an empty tile.
                priority_queue.insert((tile_score.get(xy), i), (xy, true));
                i += 1;
            }
        }

        fn avg_around_score(
            structures_matrix: &RoomMatrix<PackedTileStructures>,
            tile_score: &RoomMatrix<u8>,
            xy: RoomXY,
        ) -> u8 {
            let mut total_score_around = 0u16;
            let mut empty_tiles_around = 0u8;
            for near in xy.around() {
                let near_score = tile_score.get(near);
                if near_score != OBSTACLE_COST && structures_matrix.get(near).is_empty() {
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

        let mut remaining_extensions = 60 - structures_matrix.find_xy(Extension.into()).count();
        while remaining_extensions > 0 && !priority_queue.is_empty() {
            let ((xy_score, _), (xy, placement)) = priority_queue.pop_first().unwrap();
            debug!("[{}] {}: {}, {}", remaining_extensions, xy_score, xy, placement);
            if placement {
                structures_matrix.set(xy, Extension.into());
                let current_score = tile_score.get(xy);

                let removal_score = avg_around_score(structures_matrix, &tile_score, xy).saturating_sub(current_score);

                if removal_score < OBSTACLE_COST {
                    priority_queue.insert((removal_score, i), (xy, false));
                    i += 1;
                    debug!("  + {}: {}, {}", removal_score, xy, false);
                }

                remaining_extensions -= 1;
            } else {
                let current_score = tile_score.get(xy);
                let removal_score = avg_around_score(structures_matrix, &tile_score, xy).saturating_sub(current_score);

                if removal_score != xy_score {
                    // If the score changed as a result of, e.g., removing some empty tiles around, we re-queue the
                    // tile.
                    priority_queue.insert((removal_score, i), (xy, false));
                    i += 1;
                    debug!(" => {}: {}, {}", removal_score, xy, false);
                } else {
                    structures_matrix.set(xy, Road.into());

                    for near in xy.around() {
                        if tile_score.get(near) != OBSTACLE_COST && structures_matrix.get(near).is_empty() {
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
