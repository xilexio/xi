use crate::algorithms::binary_search::{upper_bound, upper_bound_by_key};
use crate::algorithms::chunk_graph::{chunk_graph, ChunkGraph, ChunkId};
use crate::algorithms::distance_matrix::{
    count_restricted_distance_matrix, distance_matrix, rect_restricted_distance_matrix,
};
use crate::algorithms::distance_transform::{distance_transform_from_obstacles, l1_distance_transform_from_obstacles};
use crate::algorithms::grid_min_cut::grid_min_cut;
use crate::algorithms::interior_matrix::interior_matrix;
use crate::algorithms::matrix_common::MatrixCommon;
use crate::algorithms::max_boundary_distance::max_boundary_distance;
use crate::algorithms::room_matrix::RoomMatrix;
use crate::algorithms::room_matrix_slice::RoomMatrixSlice;
use crate::algorithms::shortest_path_by_matrix::shortest_path_by_matrix_with_preference;
use crate::algorithms::weighted_distance_matrix::{obstacle_cost, unreachable_cost, weighted_distance_matrix};
use crate::consts::{OBSTACLE_COST, UNREACHABLE_COST};
use crate::geometry::rect::{ball, bounding_rect, room_rect, Rect};
use crate::geometry::room_xy::RoomXYUtils;
use crate::room_planner::packed_tile_structures::PackedTileStructures;
use crate::room_planner::plan::{Plan, PlanScore};
use crate::room_planner::planned_tile::PlannedTile;
use crate::room_planner::stamps::{core_stamp, labs_stamp};
use crate::room_planner::RoomPlannerError::{ControllerNotFound, PlanGenerationFinished, ResourceNotFound, StructurePlacementFailure, UnreachableResource};
use crate::room_state::packed_terrain::PackedTerrain;
use crate::room_state::RoomState;
use crate::unwrap;
use crate::visualization::visualize;
use crate::visualization::Visualization::{Graph, Matrix};
use derive_more::Constructor;
use log::debug;
use num_traits::{clamp, Signed};
use rustc_hash::FxHashSet;
use screeps::Part::{Carry, Move, Work};
use screeps::StructureType::{Extension, Nuker, Observer, Rampart, Road, Spawn, Storage, Tower};
use screeps::Terrain::{Swamp, Wall};
use screeps::{game, RoomName, RoomXY, StructureType, CREEP_LIFE_TIME, LINK_LOSS_RATIO, MINERAL_REGEN_TIME, RAMPART_DECAY_AMOUNT, RAMPART_DECAY_TIME, REPAIR_COST, REPAIR_POWER, ROAD_DECAY_AMOUNT, ROAD_DECAY_TIME};
use std::cmp::{max, min, Ordering};
use std::collections::BTreeMap;
use std::error::Error;
use std::iter::{empty, once};
use thiserror::Error;
use crate::towers::tower_attack_power;

pub mod packed_tile_structures;
pub mod plan;
pub mod planned_tile;
pub mod stamps;

const APPROXIMATE_BASE_TILES: u16 = 140;
const SOURCE_DIST_WEIGHT: f32 = 2.0;
const MINERAL_DIST_WEIGHT: f32 = 1.0;
const CONTROLLER_DIST_WEIGHT: f32 = 1.5;
const RESOURCES_DIST_PERCENTILE_CUTOFF: f32 = 0.5;
const MIN_RESOURCE_CENTERS: usize = 25;
const CHUNK_RADIUS: u8 = 5;
const MAX_LABS_DIST: u8 = 12;
const FAST_MODE_LABS_DIST: u8 = 3;
// TODO take into account rampart radius - we prefer if ramparts are squeezed in one area

const PLAIN_ROAD_COST: u16 = 100;
const SWAMP_ROAD_COST: u16 = 101;
const EXISTING_ROAD_COST: u16 = 75;

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
    #[error("plan generation already finished")]
    PlanGenerationFinished,
}

#[derive(Copy, Clone, Debug, Constructor)]
struct RoadTarget {
    xy: RoomXY,
    stop_dist: u8,
    interior: bool,
}

pub struct RoomPlanner {
    fast_mode: bool,

    room_name: RoomName,
    controller_xy: RoomXY,
    source_xys: Vec<RoomXY>,
    mineral_xy: RoomXY,
    terrain: PackedTerrain,

    walls: Vec<RoomXY>,
    controller_dm: RoomMatrix<u8>,
    source_dms: Vec<RoomMatrix<u8>>,
    mineral_dm: RoomMatrix<u8>,
    exits_dm: RoomMatrix<u8>,
    exit_rampart_distances: RoomMatrix<u8>,
    dt: RoomMatrix<u8>,
    dt_l1: RoomMatrix<u8>,
    chunks: ChunkGraph,

    core_centers_stack: Vec<RoomXY>,
    core_rotations_stack: Vec<u8>,
    labs_dists_stack: Vec<u8>,
    labs_top_left_corners_stack: Vec<RoomXY>,
    labs_rotations_stack: Vec<u8>,

    // Cache per core rotation.
    core: RoomMatrixSlice<PlannedTile>,
    storage_xy: RoomXY,
    // Cache per labs rotations
    labs: RoomMatrixSlice<PlannedTile>,
    planned_tiles: RoomMatrix<PlannedTile>,

    pub best_plan: Option<Plan>,
}

impl RoomPlanner {
    pub fn new(state: &RoomState, fast_mode: bool) -> Result<RoomPlanner, Box<dyn Error>> {
        // Preliminary checks of the room.
        let controller_xy = state.controller.ok_or(ControllerNotFound)?.xy;
        let source_xys = (!state.sources.is_empty())
            .then_some(state.sources.iter().map(|source| source.xy).collect::<Vec<_>>())
            .ok_or(ResourceNotFound)?;
        let mineral_xy = state.mineral.ok_or(ResourceNotFound)?.xy;

        // Finding distances from various room features and initializing data structures.
        let walls = state.terrain.walls().collect::<Vec<_>>();
        let controller_dm = distance_matrix(walls.iter().copied(), once(controller_xy));
        let source_dms = source_xys
            .iter()
            .copied()
            .map(|source_xy| distance_matrix(walls.iter().copied(), once(source_xy)))
            .collect::<Vec<_>>();
        let mineral_dm = distance_matrix(walls.iter().copied(), once(mineral_xy));
        let exits = room_rect()
            .boundary()
            .filter_map(|xy| (state.terrain.get(xy) != Wall).then_some(xy))
            .collect::<Vec<_>>();
        let exits_dm = distance_matrix(walls.iter().copied(), exits.iter().copied());
        let exit_rampart_distances = distance_matrix(
            empty(),
            exits_dm.iter().filter_map(|(xy, dist)| (dist <= 1).then_some(xy)),
        );
        // Distance transform in maximum metric.
        let dt = distance_transform_from_obstacles(walls.iter().copied());
        // Distance transform in l1 metric.
        let dt_l1 = l1_distance_transform_from_obstacles(walls.iter().copied());
        // Chunk graph.
        let walls_matrix = state.terrain.to_obstacle_matrix(0);
        let chunks = chunk_graph(&walls_matrix, CHUNK_RADIUS);

        let mut room_planner = RoomPlanner {
            fast_mode,

            room_name: state.room_name,
            controller_xy,
            source_xys,
            mineral_xy,

            terrain: state.terrain,
            walls,
            controller_dm,
            source_dms,
            mineral_dm,
            exits_dm,
            exit_rampart_distances,
            dt,
            dt_l1,
            chunks,

            core_centers_stack: Vec::new(),
            core_rotations_stack: Vec::new(),
            labs_dists_stack: Vec::new(),
            labs_top_left_corners_stack: Vec::new(),
            labs_rotations_stack: Vec::new(),

            core: RoomMatrixSlice::new(Rect::default(), PlannedTile::default()),
            storage_xy: (0, 0).try_into().unwrap(),

            labs: RoomMatrixSlice::new(Rect::default(), PlannedTile::default()),
            planned_tiles: RoomMatrix::new(PlannedTile::default()),

            best_plan: None,
        };

        room_planner.init_core_centers()?;

        Ok(room_planner)
    }

    /// Creates the room plan.
    /// A good place for the core is one that balances the following:
    /// - the number of ramparts required to protect the base,
    /// - maximum distance to ramparts from spawns and storage,
    /// - distance from the nearest spawn to sources, controller and mineral,
    /// - distance between ramparts to maximize minimal tower damage right outside of ramparts.
    pub fn plan(&mut self) -> Result<Plan, Box<dyn Error>> {
        self.labs_rotations_stack.pop();
        if self.labs_rotations_stack.is_empty() {
            self.labs_top_left_corners_stack.pop();
            if self.labs_top_left_corners_stack.is_empty() {
                self.labs_dists_stack.pop();
                if self.labs_dists_stack.is_empty() {
                    self.core_rotations_stack.pop();
                    if self.core_rotations_stack.is_empty() {
                        self.core_centers_stack.pop();
                        if self.core_centers_stack.is_empty() {
                            Err(PlanGenerationFinished)?;
                        }

                        self.init_core_rotations_stack();
                    }
                    self.init_labs_dists_stack();
                }
                self.init_labs_top_left_corners_stack()?;
            }
            self.init_labs_rotations_stack();
        }
        self.init_planned_tiles()?;

        debug!(
            "Processing core {}/R{} and labs {}/R{} at dist {}.",
            self.current_core_center(),
            self.current_core_rotation(),
            self.current_labs_top_left_corner(),
            self.current_labs_rotation(),
            self.current_labs_dist(),
        );

        self.plan_from_stamps()
    }

    pub fn is_finished(&self) -> bool {
        self.core_centers_stack.is_empty()
    }

    pub fn init_core_centers(&mut self) -> Result<(), Box<dyn Error>> {
        // TODO Perform theoretical calculations on good weights, include mineral in them.
        let resources_dist_sum = {
            let mut preliminary_sum = RoomMatrix::new(0.0f32);
            let resource_dms_and_weights = [
                (&self.controller_dm, CONTROLLER_DIST_WEIGHT),
                (&self.mineral_dm, MINERAL_DIST_WEIGHT),
            ]
            .into_iter()
            .chain(self.source_dms.iter().map(|dm| (dm, SOURCE_DIST_WEIGHT)));
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
                (self.exit_rampart_distances.get(xy) >= 6 && value != OBSTACLE_COST && self.core_fits(&self.dt, xy))
                    .then_some((xy, value))
            })
            .collect::<Vec<_>>();
        if resource_centers.is_empty() {
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
        debug!("Found {} valid core centers.", resource_centers.len());
        self.core_centers_stack = resource_centers
            .iter()
            .copied()
            .take(number_of_good_resource_centers)
            .map(|(xy, _)| xy)
            .collect();
        debug!(
            "Remaining {} core centers within percentile {} of weighted sum of distances to resources.",
            self.core_centers_stack.len(),
            RESOURCES_DIST_PERCENTILE_CUTOFF
        );

        if self.fast_mode {
            let mut used_chunks = FxHashSet::default();
            self.core_centers_stack = self
                .core_centers_stack
                .iter()
                .copied()
                .filter(|&xy| {
                    let xy_chunk = self.chunks.xy_chunks.get(xy);
                    if used_chunks.contains(&xy_chunk) {
                        false
                    } else {
                        used_chunks.insert(xy_chunk);
                        true
                    }
                })
                .collect::<Vec<_>>();

            debug!(
                "Remaining {} core centers after selecting one per chunk.",
                self.core_centers_stack.len()
            );
        }

        self.core_centers_stack.reverse();

        // Temporary value to be removed at the beginning.
        self.core_centers_stack.push((0, 0).try_into().unwrap());

        Ok(())
    }

    fn init_core_rotations_stack(&mut self) {
        if self.fast_mode {
            let core_center = self.current_core_center();
            let inner_core_rect = ball(core_center, 2);
            let mut corners_dt = inner_core_rect
                .corners()
                .into_iter()
                .enumerate()
                .map(|(i, xy)| (i, self.dt.get(xy)))
                .collect::<Vec<_>>();
            corners_dt.sort_by_key(|(_, dist)| *dist);
            self.core_rotations_stack = vec![corners_dt[3].0 as u8];
        } else {
            self.core_rotations_stack = vec![3, 2, 1, 0];
        }
    }

    fn init_labs_dists_stack(&mut self) {
        self.core = core_stamp();
        let core_center = self.current_core_center();
        unwrap!(self.core.translate(core_center.sub(self.core.rect.center())));
        let core_rotations = self.current_core_rotation();
        unwrap!(self.core.rotate(core_rotations));

        self.storage_xy = unwrap!(self
            .core
            .iter()
            .find_map(|(xy, tile)| (tile.structures() == Storage.into()).then_some(xy)));

        if self.fast_mode {
            self.labs_dists_stack = (1..FAST_MODE_LABS_DIST).collect();
        } else {
            self.labs_dists_stack = (1..MAX_LABS_DIST).collect();
        }
        self.labs_dists_stack.reverse();
    }

    fn init_labs_top_left_corners_stack(&mut self) -> Result<(), RoomPlannerError> {
        let labs_dist = self.current_labs_dist();

        self.labs_top_left_corners_stack = ball(self.storage_xy, labs_dist)
            .boundary()
            .filter(|&labs_corner_xy| self.storage_xy.dist(labs_corner_xy) == labs_dist)
            .flat_map(|labs_corner_xy| {
                self.other_lab_corner(labs_corner_xy, self.storage_xy)
                    .into_iter()
                    .filter_map(|other_corner| {
                        let labs_rect = Rect::new_unordered(labs_corner_xy, other_corner);
                        self.labs_fit(labs_rect).then_some(labs_rect.top_left)
                    })
                    .collect::<Vec<_>>()
                    .into_iter()
            })
            .collect();

        if self.labs_top_left_corners_stack.is_empty() {
            Err(RoomPlannerError::StructurePlacementFailure)
        } else {
            Ok(())
        }
    }

    fn init_labs_rotations_stack(&mut self) {
        if self.fast_mode {
            let top_left = self.current_labs_top_left_corner();
            let labs_rect = unwrap!(Rect::new(top_left, unsafe { top_left.add_diff((3, 3)) }));
            let corners = labs_rect.corners();
            if min(corners[1].dist(self.storage_xy), corners[3].dist(self.storage_xy))
                < min(corners[0].dist(self.storage_xy), corners[2].dist(self.storage_xy))
            {
                self.labs_rotations_stack = vec![1];
            } else {
                self.labs_rotations_stack = vec![0];
            }
        } else {
            self.labs_rotations_stack = vec![1, 0];
        }
    }

    fn init_planned_tiles(&mut self) -> Result<(), Box<dyn Error>> {
        self.labs = labs_stamp();
        unwrap!(self
            .labs
            .translate(self.current_labs_top_left_corner().sub((0, 0).try_into().unwrap()),));
        let labs_rotations = self.current_labs_rotation();
        unwrap!(self.labs.rotate(labs_rotations));

        self.planned_tiles = RoomMatrix::new(PlannedTile::default());
        self.planned_tiles.merge_structures(&self.core)?;
        self.planned_tiles.merge_structures(&self.labs)?;
        Ok(())
    }

    fn plan_from_stamps(&mut self) -> Result<(Plan), Box<dyn Error>> {
        // First attempt in which good places to grow towards are not known.

        // Connecting the labs to the storage.
        // TODO include this in labs creation and make it a part of the score to decide labs' placement
        let closest_lab_road = self.closest_labs_road();
        self.connect_with_roads(
            once(closest_lab_road),
            1,
            once(RoadTarget::new(self.storage_xy, 1, true)),
        )?;

        // After placing the stamps, creating the shortest routes from spawns to mineral, sources and
        // controller.
        let spawns = self
            .core
            .iter()
            .filter_map(|(xy, tile)| (tile.structures() == Spawn.into()).then_some(xy))
            .collect::<Vec<_>>();
        let controller_and_sources_targets = once(RoadTarget::new(self.controller_xy, 4, true)).chain(
            self.source_xys
                .clone()
                .into_iter()
                .map(|source_xy| RoadTarget::new(source_xy, 2, false)),
        );
        self.connect_with_roads(spawns.iter().copied(), 1, controller_and_sources_targets)?;
        // Creating the shortest route to the mineral from storage. It may be outside of ramparts.
        self.connect_with_roads(
            once(self.storage_xy),
            1,
            once(RoadTarget::new(self.mineral_xy, 2, false)),
        )?;

        // TODO improve this part by not requiring the whole area, also add the link
        for xy in [self.mineral_xy, self.controller_xy]
            .into_iter()
            .chain(self.source_xys.iter().copied())
        {
            for near in xy.around() {
                if self.planned_tiles.get(near).is_empty() {
                    self.planned_tiles
                        .set(near, self.planned_tiles.get(near).with_reserved(true));
                }
            }
        }

        let current_extensions_count = self
            .planned_tiles
            .iter()
            .filter(|(xy, tile)| tile.structures() == Extension.into())
            .count();
        self.grow_reachable_structures(Extension, (60 - current_extensions_count) as u8)?;
        self.grow_reachable_structures(Tower, 6)?;
        self.grow_reachable_structures(Nuker, 1)?;
        self.grow_reachable_structures(Observer, 1)?;

        let distances_from_structures = distance_matrix(
            self.walls.iter().copied(),
            self.planned_tiles
                .iter()
                .filter_map(|(xy, tile)| tile.interior().then_some(xy)),
        );

        // TODO include all fields around the controller with a path to them

        let base_min_cut_matrix = distances_from_structures.map(|xy, dist| {
            if self.terrain.get(xy) == Wall {
                obstacle_cost()
            } else if dist <= 2 {
                0
            } else {
                10 + dist
            }
        });
        let min_cut = grid_min_cut(&base_min_cut_matrix);
        for xy in min_cut.iter().copied() {
            self.planned_tiles.merge_structure(xy, Rampart)?;
        }

        let eco_score = self.energy_balance();
        let def_score = self.min_tower_damage_outside_of_ramparts() as f32;
        let score = PlanScore {
            total_score: eco_score + def_score / 120.0,
            eco_score,
            def_score,
        };
        let plan = Plan::new(self.planned_tiles.clone(), score);

        debug!("Successfully created a new plan with score {:?}.", score);
        if self.best_plan.as_ref().map(|plan| plan.score.total_score < score.total_score).unwrap_or(true) {
            self.best_plan = Some(plan.clone());
        }

        Ok(plan)
    }

    #[inline]
    fn current_core_center(&self) -> RoomXY {
        *unwrap!(self.core_centers_stack.last())
    }

    #[inline]
    fn current_core_rotation(&self) -> u8 {
        *unwrap!(self.core_rotations_stack.last())
    }

    #[inline]
    fn current_labs_dist(&self) -> u8 {
        *unwrap!(self.labs_dists_stack.last())
    }

    #[inline]
    fn current_labs_top_left_corner(&self) -> RoomXY {
        *unwrap!(self.labs_top_left_corners_stack.last())
    }

    #[inline]
    fn current_labs_rotation(&self) -> u8 {
        *unwrap!(self.labs_rotations_stack.last())
    }

    #[inline]
    fn closest_labs_road(&self) -> RoomXY {
        let mut lab_roads = self
            .labs
            .iter()
            .filter_map(|(xy, tile)| tile.structures().road().then_some(xy))
            .collect::<Vec<_>>();
        lab_roads.sort_by_key(|&xy| xy.dist(self.storage_xy));
        lab_roads[0]
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

    fn other_lab_corner(&self, lab_corner_xy: RoomXY, storage_xy: RoomXY) -> Vec<RoomXY> {
        let (dx, dy) = lab_corner_xy.sub(storage_xy);

        if dx != 0 && dy != 0 {
            match lab_corner_xy.try_add_diff((3 * dx.signum(), 3 * dy.signum())) {
                Ok(xy) => vec![xy],
                Err(_) => Vec::new(),
            }
        } else if dx == 0 {
            [
                lab_corner_xy.try_add_diff((-3, 3 * dy.signum())),
                lab_corner_xy.try_add_diff((3, 3 * dy.signum())),
            ]
            .iter()
            .filter_map(|wrapped_xy| wrapped_xy.ok())
            .collect::<Vec<_>>()
        } else {
            [
                lab_corner_xy.try_add_diff((3 * dx.signum(), -3)),
                lab_corner_xy.try_add_diff((3 * dx.signum(), 3)),
            ]
            .iter()
            .filter_map(|wrapped_xy| wrapped_xy.ok())
            .collect::<Vec<_>>()
        }
    }

    #[inline]
    fn labs_fit(&self, labs_rect: Rect) -> bool {
        // Labs need a plus, but have no center due to even width.
        // . L L .
        // L R L L
        // L L R L
        // . L L .
        let core_center = self.current_core_center();
        unsafe {
            // Note that once the first dt_l1 below passes, adding the diff is correct.
            self.dt_l1.get(labs_rect.top_left.add_diff((1, 1))) >= 2
                && self.dt_l1.get(labs_rect.top_left.add_diff((1, 2))) >= 2
                && self.dt_l1.get(labs_rect.top_left.add_diff((2, 1))) >= 2
                && self.dt_l1.get(labs_rect.top_left.add_diff((2, 2))) >= 2
                && labs_rect.corners().iter().copied().all(|xy| {
                    self.exit_rampart_distances.get(xy) >= 4
                        && (core_center.dist(xy) >= 4
                            || core_center.dist(xy) == 3 && {
                                let core_center_diff = core_center.sub(xy);
                                min(core_center_diff.0.abs(), core_center_diff.1.abs()) >= 2
                            })
                })
        }
    }

    fn grow_reachable_structures(&mut self, structure_type: StructureType, count: u8) -> Result<(), Box<dyn Error>> {
        let obstacles = self
            .planned_tiles
            .iter()
            .filter_map(|(xy, structure)| (!structure.is_passable(true)).then_some(xy))
            .chain(self.walls.iter().copied());
        let storage_dm = distance_matrix(obstacles, once(self.storage_xy));

        debug!("Placing {:?}.", structure_type);

        // Finding scores of extensions. The lower, the better. The most important factor is the distance from storage.
        let tile_score = storage_dm.map(|xy, dist| {
            if dist >= unreachable_cost()
                || !self.planned_tiles.get(xy).is_empty()
                || self.exit_rampart_distances.get(xy) <= 3
            {
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
            if xy.around().any(|near| self.planned_tiles.get(near).structures().road()) {
                // Keeping tile position and whether it is an empty tile.
                priority_queue.insert((tile_score.get(xy), i), (xy, true));
                i += 1;
            }
        }

        let avg_around_score = |planned_tiles: &RoomMatrix<PlannedTile>, xy: RoomXY| {
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
        };

        let mut remaining = count;
        while remaining > 0 && !priority_queue.is_empty() {
            let ((xy_score, _), (xy, placement)) = priority_queue.pop_first().unwrap();
            // debug!("[{}] {}: {}, {}", remaining_extensions, xy_score, xy, placement);
            if placement {
                self.planned_tiles
                    .set(xy, PlannedTile::from(structure_type).with_interior(true));
                let current_score = tile_score.get(xy);

                let removal_score = avg_around_score(&self.planned_tiles, xy).saturating_sub(current_score);

                if removal_score < OBSTACLE_COST {
                    priority_queue.insert((removal_score, i), (xy, false));
                    i += 1;
                    // debug!("  + {}: {}, {}", removal_score, xy, false);
                }

                remaining -= 1;
            } else {
                let current_score = tile_score.get(xy);
                let removal_score = avg_around_score(&self.planned_tiles, xy).saturating_sub(current_score);

                if removal_score != xy_score {
                    // If the score changed as a result of, e.g., removing some empty tiles around, we re-queue the
                    // tile.
                    priority_queue.insert((removal_score, i), (xy, false));
                    i += 1;
                    // debug!(" => {}: {}, {}", removal_score, xy, false);
                } else {
                    self.planned_tiles.set(xy, PlannedTile::from(Road).with_interior(true));

                    for near in xy.around() {
                        if tile_score.get(near) != OBSTACLE_COST && self.planned_tiles.get(near).is_empty() {
                            let score = tile_score.get(near);
                            priority_queue.insert((score, i), (near, true));
                            // debug!("  + {}: {}, {}", score, near, true);
                            i += 1;
                        }
                    }

                    remaining += 1;
                }
            }
        }

        // TODO place extension when there is a close place
        // if there are at least 3 extensions to reach with a single road, place it, replacing an extension
        // !! keep number of surrounding extensions per tile
        // total score is average distance to extensions (and if possible clumpiness - no lone extensions)

        Ok(())
    }

    fn connect_with_roads(
        &mut self,
        start: impl Iterator<Item = RoomXY>,
        start_dist: u8,
        targets: impl Iterator<Item = RoadTarget>,
    ) -> Result<(), Box<dyn Error>> {
        let grid_bit = (self.storage_xy.x.u8() + self.storage_xy.y.u8()) % 2;
        let mut checkerboard = RoomMatrix::new(0u8);
        let mut cost_matrix = RoomMatrix::new(PLAIN_ROAD_COST);
        for (xy, t) in self.terrain.iter() {
            checkerboard.set(xy, [0, 1][((grid_bit + xy.x.u8() + xy.y.u8()) % 2) as usize]);
            if t == Wall {
                cost_matrix.set(xy, obstacle_cost());
            } else if t == Swamp {
                cost_matrix.set(xy, SWAMP_ROAD_COST);
            }
        }
        for (xy, tile) in self.planned_tiles.iter() {
            if !tile.is_passable(true) {
                cost_matrix.set(xy, obstacle_cost());
            } else if tile.structures().road() {
                cost_matrix.set(xy, EXISTING_ROAD_COST);
            }
        }

        let start_vec = start.collect::<Vec<_>>();

        for target in targets {
            // TODO it should be less expensive to recompute it using the existing cost matrix as base, at least on the side further away from added roads than from previously existing ones
            let distances = weighted_distance_matrix(&cost_matrix, start_vec.iter().copied());
            // TODO for now it is a small optimization that tries to merge roads, but a proper merging should occur using Steiner Minimal Tree algorithm
            // TODO .ok_or(RoadConnectionFailure)? if we cannot get within final_dist
            // TODO final_dist here does not work as intended
            // debug!("connect_with_roads from {:?} to {:?}", start_vec, target);
            let path = shortest_path_by_matrix_with_preference(&distances, &checkerboard, target.xy);
            // debug!("path: {:?}", path);
            for &xy in &path[(target.stop_dist as usize)..(path.len() - (start_dist as usize))] {
                let mut tile = self.planned_tiles.get(xy).merge(Road)?;
                if target.interior {
                    tile = tile.with_interior(true);
                }
                // debug!("{} before {:?} after {:?}", xy, self.planned_tiles.get(xy), tile);
                self.planned_tiles.set(xy, tile);
                cost_matrix.set(xy, EXISTING_ROAD_COST);
            }
        }

        Ok(())
    }

    fn min_tower_damage_outside_of_ramparts(&self) -> u32 {
        let rampart_xys = self.planned_tiles.find_structure_xys(Rampart);
        let interior = interior_matrix(self.walls.iter().copied(), rampart_xys.iter().copied());
        let tower_xys = self.planned_tiles.find_structure_xys(Tower);

        let mut min_tower_damage = u32::MAX;
        for xy in rampart_xys {
            for near in xy.around() {
                if !interior.get(near) && self.terrain.get(near) != Wall {
                    let damage = tower_xys.iter().copied().map(|xy| tower_attack_power(xy.dist(near))).sum::<u32>();
                    if damage < min_tower_damage {
                        min_tower_damage = damage;
                    }
                }
            }
        }

        min_tower_damage
    }

    fn energy_balance(&self) -> f32 {
        let income = 3000.0 / 300.0 * (self.source_xys.len() as f32);

        let miner_work = 12;
        let miner_move = 3;
        let miner_carry = 4;
        let miner_cost = miner_work * Work.cost() + miner_move + Move.cost() + miner_carry * Carry.cost();
        let miners_cost_per_tick = (self.source_xys.len() as f32) * (miner_cost as f32) / (CREEP_LIFE_TIME as f32);

        let link_cost = income * LINK_LOSS_RATIO;

        let road_count = self.planned_tiles.find_structure_xys(Road).len();
        let road_decay_per_tick = (road_count as f32) * (ROAD_DECAY_AMOUNT as f32) / (ROAD_DECAY_TIME as f32);
        let rampart_count = self.planned_tiles.find_structure_xys(Rampart).len();
        let rampart_decay_per_tick =
            (rampart_count as f32) * (RAMPART_DECAY_AMOUNT as f32) / (RAMPART_DECAY_TIME as f32);

        let repair_cost_per_tick = (road_decay_per_tick + rampart_decay_per_tick) * REPAIR_COST;

        // TODO for now ignoring the costs related to other creeps and controller maintenance as they are constant

        income - miners_cost_per_tick - link_cost - repair_cost_per_tick
    }

    fn cpu_cost(&self) -> f32 {
        let not_roads = self
            .planned_tiles
            .iter()
            .filter_map(|(xy, tile)| (!tile.structures().road()).then_some(xy));
        let road_matrix = distance_matrix(not_roads.into_iter(), once(self.storage_xy));

        let source_travel_intents = self
            .source_xys
            .iter()
            .copied()
            .map(|xy| {
                xy.around()
                    .map(|near| road_matrix.get(near))
                    .min()
                    .unwrap_or(obstacle_cost()) as f32
            })
            .sum::<f32>();
        let source_travel_intents_per_tick = source_travel_intents / (CREEP_LIFE_TIME as f32);

        let mineral_travel_intents = self
            .mineral_xy
            .around()
            .map(|near| road_matrix.get(near))
            .min()
            .unwrap_or(obstacle_cost()) as f32;
        // Around 22 hauler trips + 1 miner trip per regeneration.
        let mineral_travel_intents_per_tick = 23.0 * mineral_travel_intents / (MINERAL_REGEN_TIME as f32);

        let mut rampart_count = 0;
        let rampart_repair_travel_intents = self
            .planned_tiles
            .find_structure_xys(Rampart)
            .into_iter()
            .map(|xy| {
                rampart_count += 1;
                road_matrix.get(xy) as f32
            })
            .sum::<f32>();
        // Assuming 1000 energy worth of repair is used in one trip.
        let rampart_repair_travel_intents_per_tick =
            rampart_repair_travel_intents * (RAMPART_DECAY_AMOUNT as f32) / (1000.0 * (RAMPART_DECAY_TIME as f32));
        let rampart_repair_intents_per_tick = (rampart_count as f32) * (RAMPART_DECAY_AMOUNT as f32)
            / ((REPAIR_POWER as f32) * (RAMPART_DECAY_TIME as f32));

        let mut road_count = 0;
        let road_repair_travel_intents = self
            .planned_tiles
            .find_structure_xys(Road)
            .into_iter()
            .map(|xy| {
                road_count += 1;
                road_matrix.get(xy) as f32
            })
            .sum::<f32>();
        // Assuming 1000 energy worth of repair is used in one trip.
        let road_travel_intents_per_tick =
            road_repair_travel_intents * (ROAD_DECAY_AMOUNT as f32) / (1000.0 * (ROAD_DECAY_TIME as f32));
        let road_repair_intents_per_tick =
            (road_count as f32) * (ROAD_DECAY_AMOUNT as f32) / ((REPAIR_POWER as f32) * (ROAD_DECAY_TIME as f32));

        // TODO mineral mining/hauling intents, controller maintenance intents, constant intents for storage and spawning

        (source_travel_intents_per_tick
            + mineral_travel_intents_per_tick
            + rampart_repair_travel_intents_per_tick
            + rampart_repair_intents_per_tick
            + road_travel_intents_per_tick
            + road_repair_intents_per_tick)
            * 0.2
    }
}
