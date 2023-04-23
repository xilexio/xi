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
use crate::algorithms::shortest_path_by_distance_matrix::{
    closest_in_circle_by_matrix, distance_by_matrix, shortest_path_by_distance_matrix,
    shortest_path_by_matrix_with_preference,
};
use crate::algorithms::weighted_distance_matrix::{obstacle_cost, unreachable_cost, weighted_distance_matrix};
use crate::consts::{OBSTACLE_COST, UNREACHABLE_COST};
use crate::cost_approximation::energy_balance_and_cpu_cost;
use crate::geometry::rect::{ball, bounding_rect, room_rect, Rect};
use crate::geometry::room_xy::RoomXYUtils;
use crate::room_planner::packed_tile_structures::PackedTileStructures;
use crate::room_planner::plan::{Plan, PlanScore, PlannedControllerInfo, PlannedMineralInfo, PlannedSourceInfo};
use crate::room_planner::planned_tile::{BasePart, PlannedTile};
use crate::room_planner::stamps::{core_stamp, labs_stamp};
use crate::room_planner::RoomPlannerError::{
    ControllerNotFound, PlanGenerationFinished, RampartPlacementFailure, ResourceNotFound, RoadConnectionFailure,
    StructurePlacementFailure, UnreachableResource,
};
use crate::room_state::packed_terrain::PackedTerrain;
use crate::room_state::{RoomState, SourceInfo};
use crate::towers::tower_attack_power;
use crate::unwrap;
use crate::visualization::visualize;
use crate::visualization::Visualization::{Graph, Matrix};
use derive_more::Constructor;
use log::{debug, trace};
use num_traits::{clamp, Signed};
use rustc_hash::{FxHashMap, FxHashSet};
use screeps::Part::{Carry, Move, Work};
use screeps::StructureType::{Container, Extension, Link, Nuker, Observer, Rampart, Road, Spawn, Storage, Tower};
use screeps::Terrain::{Plain, Swamp, Wall};
use screeps::{
    game, RoomName, RoomXY, StructureType, CREEP_LIFE_TIME, LINK_LOSS_RATIO, MINERAL_REGEN_TIME, RAMPART_DECAY_AMOUNT,
    RAMPART_DECAY_TIME, RANGED_MASS_ATTACK_POWER_RANGE_1, RANGED_MASS_ATTACK_POWER_RANGE_3, REPAIR_COST, REPAIR_POWER,
    ROAD_DECAY_AMOUNT, ROAD_DECAY_TIME,
};
use std::cmp::{max, min, Ordering};
use std::collections::BTreeMap;
use std::error::Error;
use std::iter::{empty, once};
use thiserror::Error;

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

const RANGED_ACTION_RANGE: u8 = 3;

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
    #[error("could not place ramparts to cover all of the interior of the base")]
    RampartPlacementFailure,
    #[error("plan generation already finished")]
    PlanGenerationFinished,
}

#[derive(Copy, Clone, Debug, Constructor)]
struct RoadTarget {
    xy: RoomXY,
    stop_range: u8,
    skipped_roads: u8,
    base_part: BasePart,
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
    enclosures: FxHashMap<ChunkId, (ChunkId, bool)>,

    core_centers_stack: Vec<RoomXY>,
    core_rotations_stack: Vec<u8>,
    labs_dists_stack: Vec<u8>,
    labs_top_left_corners_stack: Vec<RoomXY>,
    labs_rotations_stack: Vec<u8>,

    // Cache per core rotation.
    core: RoomMatrixSlice<PlannedTile>,
    storage_xy: RoomXY,
    checkerboard: RoomMatrix<u8>,
    // Cache per labs rotations
    labs: RoomMatrixSlice<PlannedTile>,
    planned_tiles: RoomMatrix<PlannedTile>,
    planned_sources: Vec<PlannedSourceInfo>,
    planned_controller: PlannedControllerInfo,
    planned_mineral: PlannedMineralInfo,

    pub best_plan: Option<Plan>,
}

impl RoomPlanner {
    // TODO option to plan remotes used outside of shard3 or when there is enough space.
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
        let enclosures = chunks.enclosures();

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
            enclosures,

            core_centers_stack: Vec::new(),
            core_rotations_stack: Vec::new(),
            labs_dists_stack: Vec::new(),
            labs_top_left_corners_stack: Vec::new(),
            labs_rotations_stack: Vec::new(),

            core: RoomMatrixSlice::new(Rect::default(), PlannedTile::default()),
            storage_xy: (0, 0).try_into().unwrap(),
            checkerboard: RoomMatrix::default(),

            labs: RoomMatrixSlice::new(Rect::default(), PlannedTile::default()),
            planned_tiles: RoomMatrix::default(),
            planned_sources: Vec::new(),
            planned_controller: PlannedControllerInfo::default(),
            planned_mineral: PlannedMineralInfo::default(),

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

        self.checkerboard = RoomMatrix::new(0u8);
        let grid_bit = (self.storage_xy.x.u8() + self.storage_xy.y.u8()) % 2;
        for (xy, t) in self.terrain.iter() {
            self.checkerboard
                .set(xy, [0, 1][((grid_bit + xy.x.u8() + xy.y.u8()) % 2) as usize]);
        }

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

    #[inline]
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

    fn plan_from_stamps(&mut self) -> Result<Plan, Box<dyn Error>> {
        // First attempt in which good places to grow towards are not known.

        // Connecting the labs to the storage.
        // TODO include this in labs creation and make it a part of the score to decide labs' placement
        let closest_lab_road = self.closest_labs_road();
        self.connect_with_roads(
            once(self.storage_xy),
            once(RoadTarget::new(closest_lab_road, 0, 1, BasePart::Interior)),
        )?;

        // After placing the stamps, creating the shortest routes from spawns to mineral, sources and
        // controller.
        let spawns = self
            .core
            .iter()
            .filter_map(|(xy, tile)| (tile.structures() == Spawn.into()).then_some(xy))
            .collect::<Vec<_>>();
        let controller_and_sources_targets = once(RoadTarget::new(self.controller_xy, 3, 1, BasePart::Interior)).chain(
            self.source_xys
                .clone()
                .into_iter()
                .map(|source_xy| RoadTarget::new(source_xy, 1, 1, BasePart::ProtectedIfInside)),
        );
        let controller_and_sources_road_xys =
            self.connect_with_roads(spawns.iter().copied(), controller_and_sources_targets)?;
        // Creating the shortest route to the mineral from storage. It may be outside of ramparts.
        let mineral_road_xy = self.connect_with_roads(
            once(self.storage_xy),
            once(RoadTarget::new(self.mineral_xy, 1, 1, BasePart::ProtectedIfInside)),
        )?[0];
        let controller_road_xy = controller_and_sources_road_xys[0];

        // Adding links.
        self.planned_sources = Vec::new();
        for (i, source_xy) in self.source_xys.clone().into_iter().enumerate() {
            let road_xy = controller_and_sources_road_xys[1 + i];
            let (link_xy, work_xy) = self.place_resource_storage(source_xy, road_xy, 1, BasePart::Protected, true)?;
            self.planned_sources
                .push(PlannedSourceInfo::new(source_xy, link_xy, work_xy));
        }

        {
            let road_xy = controller_and_sources_road_xys[0];
            let (link_xy, work_xy) = self.place_resource_storage(
                self.controller_xy,
                road_xy,
                RANGED_ACTION_RANGE,
                BasePart::Interior,
                true,
            )?;
            self.planned_controller = PlannedControllerInfo::new(link_xy, work_xy);
        }

        // Adding mineral mining container.
        {
            let (_, work_xy) =
                self.place_resource_storage(self.mineral_xy, mineral_road_xy, 1, BasePart::Outside, false)?;
            self.planned_mineral = PlannedMineralInfo::new(work_xy);
        }

        // Making sure that the controller can be actively protected.
        self.add_controller_protection();

        let current_extensions_count = self
            .planned_tiles
            .iter()
            .filter(|(xy, tile)| tile.structures() == Extension.into())
            .count();
        self.grow_reachable_structures(Extension, (60 - current_extensions_count) as u8)?;
        self.grow_reachable_structures(Tower, 6)?;
        self.grow_reachable_structures(Nuker, 1)?;
        self.grow_reachable_structures(Observer, 1)?;

        self.place_ramparts()?;

        let (energy_balance, cpu_cost) = self.energy_balance_and_cpu_cost();
        let def_score = self.min_tower_damage_outside_of_ramparts() as f32;
        let total_score = (energy_balance + def_score / 900.0) / cpu_cost;
        let score = PlanScore::new(total_score, energy_balance, cpu_cost, def_score);
        let plan = Plan::new(
            self.planned_tiles.clone(),
            self.planned_controller,
            self.planned_sources.clone(),
            self.planned_mineral,
            score,
        );

        debug!("Successfully created a new plan with score {:?}.", score);
        if self
            .best_plan
            .as_ref()
            .map(|plan| plan.score.total_score < score.total_score)
            .unwrap_or(true)
        {
            self.best_plan = Some(plan.clone());
        }

        Ok(plan)
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

    /// Places a road from the nearest start to each target. Returns the coordinates of the read closest to the target.
    /// Prefers diagonal roads. Does not place the first `start_dist` road tiles and ends at distance `target.stop_dist`
    /// from the target.
    /// Does not place the road on the start and last `target.skipped_roads` from the target.
    fn connect_with_roads(
        &mut self,
        start: impl Iterator<Item = RoomXY>,
        targets: impl Iterator<Item = RoadTarget>,
    ) -> Result<Vec<RoomXY>, Box<dyn Error>> {
        let mut cost_matrix = RoomMatrix::new(PLAIN_ROAD_COST);
        for (xy, t) in self.terrain.iter() {
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

        let mut road_ends = Vec::new();

        for target in targets {
            // TODO it should be less expensive to recompute it using the existing cost matrix as base, at least on the side further away from added roads than from previously existing ones
            let distances = weighted_distance_matrix(&cost_matrix, start_vec.iter().copied());
            // TODO for now it is a small optimization that tries to merge roads, but a proper merging should occur using Steiner Minimal Tree algorithm

            let (real_target, real_target_dist) = closest_in_circle_by_matrix(&distances, target.xy, target.stop_range);

            if real_target_dist >= unreachable_cost() {
                // debug!("connect_with_roads from {:?} to {:?} / {} D{}\n{}", start_vec, target, real_target, real_target_dist, distances);
                Err(RoadConnectionFailure)?;
            }

            // TODO checkerboard is good, but we should prioritize roads more away from ramparts to make them smaller
            let path = shortest_path_by_matrix_with_preference(&distances, &self.checkerboard, real_target);
            // debug!("path: {:?}", path);
            for &xy in &path[target.skipped_roads as usize..path.len() - 1] {
                self.planned_tiles.merge_structure(xy, Road, target.base_part)?;
                // debug!("{} before {:?} after {:?}", xy, self.planned_tiles.get(xy), tile);
                cost_matrix.set(xy, EXISTING_ROAD_COST);
            }

            road_ends.push(path[target.skipped_roads as usize]);
        }

        Ok(road_ends)
    }

    fn place_resource_storage(
        &mut self,
        resource_xy: RoomXY,
        road_xy: RoomXY,
        resource_range: u8,
        base_part: BasePart,
        link: bool,
    ) -> Result<(RoomXY, RoomXY), Box<dyn Error>> {
        let work_xys = unwrap!(ball(road_xy, 1).intersection(ball(resource_xy, resource_range)))
            .iter()
            .filter(|&near| self.terrain.get(near) != Wall && self.planned_tiles.get(near).is_empty())
            .collect::<Vec<_>>();
        if work_xys.is_empty() {
            Err(StructurePlacementFailure)?;
        }

        let work_xy = unwrap!(work_xys
            .into_iter()
            .min_by_key(|&near| obstacle_cost::<u8>() - self.exits_dm.get(near)));
        if !link {
            self.planned_tiles.merge_structure(work_xy, Container, base_part)?;
            self.planned_tiles.reserve(work_xy);

            Ok((work_xy, work_xy))
        } else {
            self.planned_tiles.reserve(work_xy);

            let link_xys = unwrap!(ball(road_xy, 1).intersection(ball(work_xy, 1)))
                .iter()
                .filter(|&near| self.terrain.get(near) != Wall && self.planned_tiles.get(near).is_empty())
                .collect::<Vec<_>>();
            if link_xys.is_empty() {
                Err(StructurePlacementFailure)?
            }

            let link_xy = unwrap!(link_xys.into_iter().min_by_key(|&near_work_xy| {
                (
                    self.storage_xy.dist(near_work_xy),
                    obstacle_cost::<u8>() - self.exits_dm.get(near_work_xy),
                )
            }));
            self.planned_tiles.merge_structure(link_xy, Link, base_part)?;
            self.planned_tiles.upgrade_base_part(work_xy, base_part);

            Ok((link_xy, work_xy))
        }
    }

    /// Marks tiles around the controller and, if not connected to the interior, leading to it so that there will be a
    /// `BasePart::Connected` path from the interior to these tiles.
    fn add_controller_protection(&mut self) {
        let mut near_controller_xys = ball(self.controller_xy, 1)
            .boundary()
            .filter(|&xy| self.terrain.get(xy) != Wall)
            .collect::<Vec<_>>();
        near_controller_xys.sort_by_key(|&xy| self.planned_controller.work_xy.dist(xy));

        for near_controller_xy in near_controller_xys.into_iter() {
            if self.planned_tiles.get(near_controller_xy).base_part() < BasePart::Connected {
                if near_controller_xy
                    .around()
                    .any(|near| self.planned_tiles.get(near).base_part() >= BasePart::Connected)
                {
                    self.planned_tiles
                        .upgrade_base_part(near_controller_xy, BasePart::Connected);
                } else {
                    let connected = self
                        .planned_tiles
                        .iter()
                        .filter_map(|(xy, tile)| (tile.base_part() >= BasePart::Connected).then_some(xy));
                    let connection_dm = distance_matrix(self.walls.iter().copied(), connected);
                    for xy in shortest_path_by_distance_matrix(&connection_dm, near_controller_xy, 1) {
                        self.planned_tiles.upgrade_base_part(xy, BasePart::Connected);
                    }
                }
            }
        }
    }

    fn grow_reachable_structures(&mut self, structure_type: StructureType, count: u8) -> Result<(), Box<dyn Error>> {
        let obstacles = self
            .planned_tiles
            .iter()
            .filter_map(|(xy, structure)| (!structure.is_passable(true)).then_some(xy))
            .chain(self.walls.iter().copied());

        let storage_dm = distance_matrix(obstacles, once(self.storage_xy));

        // debug!("Placing {:?}.", structure_type);

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
                    .set(xy, PlannedTile::from(structure_type).with_base_part(BasePart::Interior));
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
                    self.planned_tiles
                        .set(xy, PlannedTile::from(Road).with_base_part(BasePart::Interior));

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

    /// Uses min-cut to place ramparts around the base and outside according to `BasePart` definition.
    fn place_ramparts(&mut self) -> Result<(), Box<dyn Error>> {
        // debug!("{}", self.planned_tiles.map(|xy, tile| { tile.base_part() as u8 }));

        let interior_base_parts_dm = distance_matrix(
            self.walls.iter().copied(),
            self.planned_tiles
                .iter()
                .filter_map(|(xy, tile)| (tile.base_part() == BasePart::Interior).then_some(xy)),
        );

        let min_cut_cost_matrix = interior_base_parts_dm.map(|xy, interior_dist| {
            if self.terrain.get(xy) == Wall {
                obstacle_cost()
            } else if interior_dist < RANGED_ACTION_RANGE
                || self.planned_tiles.get(xy).base_part() == BasePart::Connected
            {
                0
            } else {
                10 + interior_dist
            }
        });

        let min_cut = grid_min_cut(&min_cut_cost_matrix);
        for xy in min_cut.iter().copied() {
            self.planned_tiles.merge_structure(xy, Rampart, BasePart::Outside)?;
        }

        let interior = interior_matrix(self.walls.iter().copied(), min_cut.iter().copied(), true, true);
        let interior_dm = distance_matrix(
            empty(),
            interior.iter().filter_map(|(xy, interior)| (!interior).then_some(xy)),
        );

        for (xy, interior_dist) in interior_dm.iter() {
            // Checking if ramparts are okay.
            let base_part = self.planned_tiles.get(xy).base_part();
            if (base_part == BasePart::Interior || base_part == BasePart::Connected) && interior_dist == 0 {
                Err(RampartPlacementFailure)?;
            }

            // Covering some parts in ranged attack range outside or inside the base with ramparts.
            if interior_dist <= RANGED_ACTION_RANGE
                && (base_part == BasePart::Protected
                    || base_part == BasePart::Interior
                    || interior_dist > 0 && base_part == BasePart::ProtectedIfInside)
            {
                self.planned_tiles.merge_structure(xy, Rampart, BasePart::Outside)?;
            }
        }

        Ok(())
    }

    fn min_tower_damage_outside_of_ramparts(&self) -> u32 {
        let rampart_xys = self.planned_tiles.find_structure_xys(Rampart);
        let interior = interior_matrix(self.walls.iter().copied(), rampart_xys.iter().copied(), false, true);
        let tower_xys = self.planned_tiles.find_structure_xys(Tower);

        let mut min_tower_damage = u32::MAX;
        for xy in rampart_xys {
            for near in xy.around() {
                if !interior.get(near) && self.terrain.get(near) != Wall {
                    let damage = tower_xys
                        .iter()
                        .copied()
                        .map(|xy| tower_attack_power(xy.dist(near)))
                        .sum::<u32>();
                    if damage < min_tower_damage {
                        min_tower_damage = damage;
                    }
                }
            }
        }

        min_tower_damage
    }

    fn energy_balance_and_cpu_cost(&self) -> (f32, f32) {
        let obstacles = self.planned_tiles.iter().filter_map(|(xy, tile)| {
            (self.terrain.get(xy) == Wall && !tile.structures().road() || !tile.is_passable(true)).then_some(xy)
        });
        let dm = distance_matrix(obstacles.into_iter(), once(self.storage_xy));

        let mut plain_roads_count = 0u32;
        let mut plain_roads_total_dist = 0u32;
        let mut swamp_roads_count = 0u32;
        let mut swamp_roads_total_dist = 0u32;
        let mut wall_roads_count = 0u32;
        let mut wall_roads_total_dist = 0u32;
        let mut rampart_count = 0u32;
        let mut container_count = 0u32;

        for (xy, planned_tile) in self.planned_tiles.iter() {
            if planned_tile.structures().road() {
                match self.terrain.get(xy) {
                    Plain => {
                        plain_roads_count += 1;
                        plain_roads_total_dist += dm.get(xy) as u32;
                    }
                    Swamp => {
                        swamp_roads_count += 1;
                        swamp_roads_total_dist += dm.get(xy) as u32;
                    }
                    Wall => {
                        wall_roads_count += 1;
                        wall_roads_total_dist += dm.get(xy) as u32;
                    }
                }
            }

            if planned_tile.structures().rampart() {
                rampart_count += 1;
            }

            if planned_tile.structures().main() == Container.try_into().unwrap() {
                container_count += 1;
            }
        }

        let plain_roads_avg_dist = plain_roads_total_dist as f32 / plain_roads_count as f32;
        let swamp_roads_avg_dist = swamp_roads_total_dist as f32 / swamp_roads_count as f32;
        let wall_roads_avg_dist = wall_roads_total_dist as f32 / wall_roads_count as f32;

        let source_distances = self
            .source_xys
            .iter()
            .copied()
            .map(|xy| distance_by_matrix(&dm, xy, 2))
            .collect::<Vec<_>>();

        let mineral_distance = distance_by_matrix(&dm, self.mineral_xy, 2);

        let controller_distance = distance_by_matrix(&dm, self.controller_xy, 4);

        energy_balance_and_cpu_cost(
            self.room_name,
            source_distances,
            mineral_distance,
            controller_distance,
            plain_roads_count,
            plain_roads_avg_dist,
            swamp_roads_count,
            swamp_roads_avg_dist,
            wall_roads_count,
            wall_roads_avg_dist,
            rampart_count,
            container_count,
        )

        // TODO the final eco score should have energy balance and cpu cost separate and then try to select rooms that still fit in cpu requirements, but give total max energy
        //  alternatively, it can be combined by subtracting cpu cost multiplied by average energy balance / cpu cost modified by how much we want to use on aggression
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
}
