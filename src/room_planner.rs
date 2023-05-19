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
use crate::algorithms::minimal_shortest_paths_tree::{minimal_shortest_paths_tree, PathSpec};
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
use crate::profiler::measure_time;
use crate::room_planner::packed_tile_structures::{MainStructureType, PackedTileStructures};
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
use js_sys::Math::random;
use log::debug;
use num_traits::{clamp, Signed};
use petgraph::visit::Walker;
use rustc_hash::{FxHashMap, FxHashSet};
use screeps::StructureType::{
    Container, Extension, Extractor, Link, Nuker, Observer, Rampart, Road, Spawn, Storage, Tower,
};
use screeps::Terrain::{Plain, Swamp, Wall};
use screeps::{
    game, RoomName, RoomXY, StructureType, CREEP_LIFE_TIME, LINK_LOSS_RATIO, MINERAL_REGEN_TIME, RAMPART_DECAY_AMOUNT,
    RAMPART_DECAY_TIME, RANGED_MASS_ATTACK_POWER_RANGE_1, RANGED_MASS_ATTACK_POWER_RANGE_3, REPAIR_COST, REPAIR_POWER,
    ROAD_DECAY_AMOUNT, ROAD_DECAY_TIME, ROOM_SIZE, TOWER_FALLOFF_RANGE, TOWER_OPTIMAL_RANGE,
};
use std::cmp::{max, min, Ordering, Reverse};
use std::collections::BTreeMap;
use std::error::Error;
use std::io::Read;
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
const GROWTH_RAMPART_COST: u8 = 4;
const GROWN_STRUCTURE_REMOVAL_COST: u8 = 8;

const PLAIN_ROAD_COST: u16 = 100;
const SWAMP_ROAD_COST: u16 = 102;
const EXISTING_ROAD_COST: u16 = 75;
const RAMPART_EXISTING_ROAD_COST: u16 = 20;

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

#[derive(Clone, Debug, Constructor)]
struct RoadParameters {
    start_xys: Vec<RoomXY>,
    target_xy: RoomXY,
    stop_range: u8,
    skipped_roads: u8,
    extra_length_cost: u8,
    reserved: bool,
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
    // Cache per labs rotations.
    labs: RoomMatrixSlice<PlannedTile>,
    main_ramparts: Vec<RoomXY>,
    interior_dm: RoomMatrix<u8>,
    min_tower_damage: u16,

    // Output.
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
        let dt = distance_transform_from_obstacles(walls.iter().copied(), 1);
        // Distance transform in l1 metric.
        let dt_l1 = l1_distance_transform_from_obstacles(walls.iter().copied(), 1);
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
            main_ramparts: Vec::new(),
            interior_dm: RoomMatrix::new(ROOM_SIZE),
            min_tower_damage: 0,

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
        for (xy, _) in self.terrain.iter() {
            self.checkerboard.set(xy, (grid_bit + xy.x.u8() + xy.y.u8()) % 2);
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
            Err(StructurePlacementFailure)
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
        self.interior_dm = RoomMatrix::new(ROOM_SIZE);

        // Connecting labs and resources to the storage and spawns while trying to keep all roads shortest and
        // minimize the total number of roads.
        let closest_lab_road = self.closest_labs_road();
        let spawns = self
            .core
            .iter()
            .filter_map(|(xy, tile)| (tile.structures() == Spawn.into()).then_some(xy))
            .collect::<Vec<_>>();

        let road_parameters =
            once(RoadParameters::new(
                vec![self.storage_xy],
                closest_lab_road,
                0,
                1,
                4,
                false,
                BasePart::Interior,
            ))
            .chain(once(RoadParameters::new(
                spawns.clone(),
                self.controller_xy,
                3,
                1,
                1,
                true,
                BasePart::Interior,
            )))
            .chain(once(RoadParameters::new(
                vec![self.storage_xy],
                self.mineral_xy,
                1,
                1,
                2,
                true,
                BasePart::ProtectedIfInside,
            )))
            .chain(self.source_xys.iter().map(|&source_xy| {
                RoadParameters::new(spawns.clone(), source_xy, 1, 1, 1, true, BasePart::ProtectedIfInside)
            }))
            .collect::<Vec<_>>();
        let work_xys = self.connect_with_roads(&road_parameters)?;

        // let controller_and_sources_targets = once(RoadTarget::new(self.controller_xy, 3, 1, BasePart::Interior)).chain(
        //     self.source_xys
        //         .clone()
        //         .into_iter()
        //         .map(|source_xy| RoadTarget::new(source_xy, 1, 1, BasePart::ProtectedIfInside)),
        // );
        // let controller_and_sources_road_xys =
        //     self.connect_with_roads(spawns.iter().copied(), controller_and_sources_targets)?;
        // // Creating the shortest route to the mineral from storage. It may be outside of ramparts.
        // let mineral_road_xy = self.connect_with_roads(
        //     once(self.storage_xy),
        //     once(RoadTarget::new(self.mineral_xy, 1, 1, BasePart::ProtectedIfInside)),
        // )?[0];
        // let controller_road_xy = controller_and_sources_road_xys[0];

        // Reserving work tiles.
        for &work_xy in work_xys.iter().skip(1) {
            self.planned_tiles.reserve(work_xy);
        }

        // Adding links.
        self.planned_sources = Vec::new();
        for (i, source_xy) in self.source_xys.clone().into_iter().enumerate() {
            let work_xy = work_xys[3 + i];
            let link_xy = self.place_resource_storage(work_xy, BasePart::Protected, true)?;
            self.planned_sources
                .push(PlannedSourceInfo::new(source_xy, link_xy, work_xy));
        }

        {
            let work_xy = work_xys[1];
            let link_xy = self.place_resource_storage(work_xy, BasePart::Interior, true)?;
            self.planned_controller = PlannedControllerInfo::new(link_xy, work_xy);
        }

        // Adding mineral mining container and the extractor.
        {
            let work_xy = work_xys[2];
            self.place_resource_storage(work_xy, BasePart::Outside, false)?;
            self.planned_mineral = PlannedMineralInfo::new(work_xy);
            self.planned_tiles
                .merge_structure(self.mineral_xy, Extractor, BasePart::Outside, false)?;
        }

        // Making sure that the controller can be actively protected.
        self.add_controller_protection();

        self.dry_run(|planner| -> Result<(), Box<dyn Error>> {
            // Preliminary growth of places for extensions, towers, nuker, observer. These will be used to compute
            // preliminary main rampart positions and then discarded.
            planner.grow_reachable_structures(Extension, 68, planner.storage_xy)?;
            // This sets the `main_ramparts` attribute.
            planner.place_main_ramparts()?;
            Ok(())
        })?;

        // Growing the extensions plus a spot for the nuker and observer
        self.grow_reachable_structures(Extension, 62, self.storage_xy)?;

        // Placing towers and roads to these towers.
        // TODO this should be able to remove grown structures and should add roads to the towers
        // TODO grown roads should not be marked as grown
        // TODO roads to towers may go through grown buildings, destroying them
        // TODO make it so that no extra building is grown
        self.place_towers()?;
        // TODO at this point remove needless grown structures - some towers may have replaced them and some not
        // TODO regrow missing extensions
        self.grow_reachable_structures(Extension, 68, self.storage_xy)?;
        // TODO replace some extensions with nuker and observer
        // self.grow_reachable_structures(Tower, 6, self.storage_xy)?;
        // self.grow_reachable_structures(Nuker, 1, self.storage_xy)?;
        // self.grow_reachable_structures(Observer, 1, self.storage_xy)?;

        self.place_main_ramparts()?;

        self.place_rampart_roads()?;

        // Growing dynamic structures that were removed when placing the roads to ramparts.
        // self.grow_reachable_structures(Extension, 60)?;
        // self.grow_reachable_structures(Tower, 6)?;
        // self.grow_reachable_structures(Nuker, 1)?;
        // self.grow_reachable_structures(Observer, 1)?;

        self.place_extra_ramparts()?;

        let (energy_balance, cpu_cost) = self.energy_balance_and_cpu_cost();
        let def_score = self.min_tower_damage as f32;
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

    // /// Places a road from the nearest start to each target. Returns the coordinates of the read closest to the target.
    // /// Prefers diagonal roads. Does not place the first `start_dist` road tiles and ends at distance `target.stop_dist`
    // /// from the target.
    // /// Does not place the road on the start and last `target.skipped_roads` from the target.
    // fn connect_with_roads(
    //     &mut self,
    //     start: impl Iterator<Item = RoomXY>,
    //     targets: impl Iterator<Item = RoadTarget>,
    // ) -> Result<Vec<RoomXY>, Box<dyn Error>> {
    //     let mut cost_matrix = RoomMatrix::new(PLAIN_ROAD_COST);
    //     for (xy, t) in self.terrain.iter() {
    //         if t == Wall {
    //             cost_matrix.set(xy, obstacle_cost());
    //         } else if t == Swamp {
    //             cost_matrix.set(xy, SWAMP_ROAD_COST);
    //         }
    //     }
    //     for (xy, tile) in self.planned_tiles.iter() {
    //         if !tile.is_passable(true) {
    //             cost_matrix.set(xy, obstacle_cost());
    //         } else if tile.structures().road() {
    //             cost_matrix.set(xy, EXISTING_ROAD_COST);
    //         }
    //     }
    //
    //     let start_vec = start.collect::<Vec<_>>();
    //
    //     let mut road_ends = Vec::new();
    //
    //     for target in targets {
    //         // TODO it should be less expensive to recompute it using the existing cost matrix as base, at least on the side further away from added roads than from previously existing ones
    //         let distances = weighted_distance_matrix(&cost_matrix, start_vec.iter().copied());
    //         // TODO for now it is a small optimization that tries to merge roads, but a proper merging should occur using Steiner Minimal Tree algorithm
    //
    //         let (real_target, real_target_dist) = closest_in_circle_by_matrix(&distances, target.xy, target.stop_range);
    //
    //         if real_target_dist >= unreachable_cost() {
    //             // debug!("connect_with_roads from {:?} to {:?} / {} D{}\n{}", start_vec, target, real_target, real_target_dist, distances);
    //             Err(RoadConnectionFailure)?;
    //         }
    //
    //         // TODO checkerboard is good, but we should prioritize roads more away from ramparts to make them smaller
    //         let path = shortest_path_by_matrix_with_preference(&distances, &self.checkerboard, real_target);
    //         // debug!("path: {:?}", path);
    //         for &xy in &path[target.skipped_roads as usize..path.len() - 1] {
    //             self.planned_tiles.merge_structure(xy, Road, target.base_part)?;
    //             // debug!("{} before {:?} after {:?}", xy, self.planned_tiles.get(xy), tile);
    //             cost_matrix.set(xy, EXISTING_ROAD_COST);
    //         }
    //
    //         road_ends.push(path[target.skipped_roads as usize]);
    //     }
    //
    //     Ok(road_ends)
    // }

    fn connect_with_roads(&mut self, roads_parameters: &Vec<RoadParameters>) -> Result<Vec<RoomXY>, Box<dyn Error>> {
        let mut cost_matrix = self.terrain.to_cost_matrix(1);
        for (xy, tile) in self.planned_tiles.iter() {
            if !tile.is_passable(true) {
                if tile.grown() {
                    // TODO the matrix is used but it still prefers shortest all the time
                    cost_matrix.set(xy, GROWN_STRUCTURE_REMOVAL_COST + cost_matrix.get(xy));
                } else {
                    cost_matrix.set(xy, obstacle_cost());
                }
            } else if tile.structures().road() {
                cost_matrix.set(xy, 0);
            }
        }

        // Preference of diagonal roads synced with the storage and keeping away from exits.
        let preference_matrix = self
            .exits_dm
            .map(|xy, dist| (255 - dist).saturating_add(2 * self.checkerboard.get(xy)));

        let paths = minimal_shortest_paths_tree(
            &cost_matrix,
            &preference_matrix,
            &roads_parameters
                .iter()
                .map(|params| PathSpec {
                    sources: params.start_xys.clone(),
                    target: params.target_xy,
                    target_range: params.stop_range,
                    impassable_at_range: params.reserved,
                    extra_length_cost: params.extra_length_cost,
                })
                .collect(),
        )
        .ok_or(RoadConnectionFailure)?;

        for (path, params) in paths.iter().zip(roads_parameters) {
            // The first tile is source and is skipped. The last tile is skipped and reserved.
            for &xy in &path[1..path.len() - params.skipped_roads as usize] {
                self.planned_tiles.replace_structure(xy, Road, params.base_part, false);
            }
        }

        Ok(paths.into_iter().map(|path| path[path.len() - 1]).collect())
    }

    fn place_resource_storage(
        &mut self,
        work_xy: RoomXY,
        base_part: BasePart,
        link: bool,
    ) -> Result<RoomXY, Box<dyn Error>> {
        if !link {
            self.planned_tiles
                .merge_structure(work_xy, Container, base_part, false)?;
            Ok(work_xy)
        } else {
            let link_xys = ball(work_xy, 1)
                .boundary()
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
            self.planned_tiles.merge_structure(link_xy, Link, base_part, false)?;
            self.planned_tiles.upgrade_base_part(work_xy, base_part);

            Ok(link_xy)
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

    fn grow_reachable_structures(
        &mut self,
        structure_type: StructureType,
        target_count: usize,
        center: RoomXY,
    ) -> Result<(), Box<dyn Error>> {
        // TODO it is not replacing back what is needed and it is growing from inside
        let mut obstacles = self
            .planned_tiles
            .iter()
            .filter_map(|(xy, tile)| (!tile.is_passable(true) && !tile.grown()).then_some(xy))
            .chain(self.walls.iter().copied())
            .collect::<FxHashSet<_>>();
        let center_dm = distance_matrix(obstacles.into_iter(), once(center));

        // debug!("Placing {:?}.", structure_type);

        // Finding cost of extensions. The most important factor is the distance from the center (usually storage).
        let tile_cost = center_dm.map(|xy, dist| {
            let tile = self.planned_tiles.get(xy);
            if dist >= unreachable_cost()
                || tile.structures().road()
                || !tile.is_empty() && !tile.grown()
                || self.exit_rampart_distances.get(xy) <= 3
            {
                obstacle_cost()
            } else if self.interior_dm.get(xy) <= 3 {
                dist.saturating_add(GROWTH_RAMPART_COST)
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
        let avg_around_score = |planned_tiles: &RoomMatrix<PlannedTile>, xy: RoomXY| {
            let mut total_score_around = 0u16;
            let mut empty_tiles_around = 0u8;
            for near in xy.around() {
                let near_score = tile_cost.get(near);
                if near_score != obstacle_cost::<u8>() && planned_tiles.get(near).is_empty() {
                    total_score_around += near_score as u16;
                    empty_tiles_around += 1;
                }
            }

            if empty_tiles_around > 0 {
                let multiplier = if empty_tiles_around == 1 { 3 } else { 2 };
                clamp(
                    multiplier * total_score_around / (empty_tiles_around as u16),
                    0,
                    obstacle_cost::<u8>() as u16 - 1,
                ) as u8
            } else {
                obstacle_cost()
            }
        };

        let mut i = 0u16;
        let mut priority_queue = BTreeMap::new();
        for xy in tile_cost.find_not_xy(obstacle_cost()) {
            if xy.around().any(|near| self.planned_tiles.get(near).structures().road()) {
                let near_tile = self.planned_tiles.get(xy);
                // Keeping tile position and whether it is an empty tile.
                if near_tile.structures().main() == MainStructureType::Empty {
                    priority_queue.insert((tile_cost.get(xy), i), (xy, true));
                } else {
                    let removal_score = avg_around_score(&self.planned_tiles, xy).saturating_sub(tile_cost.get(xy));
                    priority_queue.insert((removal_score, i), (xy, false));
                }

                i += 1;
            }
        }

        let current_count = self
            .planned_tiles
            .iter()
            .filter(|(xy, tile)| tile.structures().main() == unwrap!(structure_type.try_into()))
            .count();
        let mut remaining_structures = (0..(target_count - current_count))
            .map(|_| structure_type)
            .collect::<Vec<_>>();

        while !remaining_structures.is_empty() && !priority_queue.is_empty() {
            let ((xy_score, _), (xy, placement)) = priority_queue.pop_first().unwrap();
            // debug!("[{}] {}: {}, {}", remaining_extensions, xy_score, xy, placement);
            if placement {
                let current_structure_type = unwrap!(remaining_structures.pop());

                self.planned_tiles
                    .replace_structure(xy, current_structure_type, BasePart::Interior, true);
                let current_score = tile_cost.get(xy);

                let removal_score = avg_around_score(&self.planned_tiles, xy).saturating_sub(current_score);

                if removal_score < obstacle_cost() {
                    priority_queue.insert((removal_score, i), (xy, false));
                    i += 1;
                    // debug!("  + {}: {}, {}", removal_score, xy, false);
                }
            } else {
                let current_score = tile_cost.get(xy);
                let removal_score = avg_around_score(&self.planned_tiles, xy).saturating_sub(current_score);

                if removal_score != xy_score {
                    // If the score changed as a result of, e.g., removing some empty tiles around, we re-queue the
                    // tile.
                    priority_queue.insert((removal_score, i), (xy, false));
                    i += 1;
                    // debug!(" => {}: {}, {}", removal_score, xy, false);
                } else {
                    let current_structure_type = self.planned_tiles.get(xy).structures().main();

                    self.planned_tiles
                        .replace_structure(xy, Road, BasePart::Interior, false);

                    for near in xy.around() {
                        if tile_cost.get(near) != OBSTACLE_COST && self.planned_tiles.get(near).is_empty() {
                            let score = tile_cost.get(near);
                            priority_queue.insert((score, i), (near, true));
                            // debug!("  + {}: {}, {}", score, near, true);
                            i += 1;
                        }
                    }

                    // TODO xi::unwrap: Unwrapping failed on Result::Err at src\room_planner.rs:943,47 in xi::room_planner: Err(InvalidMainStructureType).
                    debug_assert!(current_structure_type != MainStructureType::Empty);
                    remaining_structures.push(unwrap!(current_structure_type.try_into()));
                }
            }
        }

        // TODO place extension when there is a close place
        // if there are at least 3 extensions to reach with a single road, place it, replacing an extension
        // !! keep number of surrounding extensions per tile
        // total score is average distance to extensions (and if possible clumpiness - no lone extensions)

        Ok(())
    }

    fn place_towers(&mut self) -> Result<(), Box<dyn Error>> {
        let obstacles = self
            .planned_tiles
            .iter()
            .filter_map(|(xy, tile)| (!tile.is_passable(true) && !tile.grown()).then_some(xy))
            .chain(self.walls.iter().copied());
        let storage_dm = distance_matrix(obstacles, once(self.storage_xy));

        let main_ramparts_dt = distance_transform_from_obstacles(self.main_ramparts.iter().copied(), ROOM_SIZE);

        let valid_tiles_matrix = self.interior_dm.map(|xy, dist| {
            dist > 0 && {
                let tile = self.planned_tiles.get(xy);
                tile.is_empty() || tile.grown() && !tile.is_passable(true)
            }
        });

        let valid_tiles = valid_tiles_matrix.find_xy(true).collect::<Vec<_>>();

        // debug!("{}", valid_tiles_matrix.map(|_, d| if d { 255u8 } else { 0u8 }));

        if valid_tiles.len() < 6 {
            Err(StructurePlacementFailure)?;
        }

        let rect = bounding_rect(self.main_ramparts.iter().copied());
        let rect_diameter = max(rect.width(), rect.height());
        let rect_center = rect.center();

        let outside_of_main_ramparts = self
            .main_ramparts
            .iter()
            .flat_map(|xy| {
                xy.around()
                    .filter(|&near| self.interior_dm.get(near) == 0 && self.terrain.get(near) != Wall)
            })
            .collect::<FxHashSet<_>>()
            .into_iter()
            .collect::<Vec<_>>();

        let mut solutions = Vec::new();

        // We try a few approaches and select the best.

        // The first approach may sometimes fail and is finding the solution from pairs whose center is exactly the
        // rectangle's center.
        measure_time("symmetric pairs tower placement", || {
            // Top-left center or the exact center depending on parity of width/height.
            let mut pair_top_xys = valid_tiles
                .iter()
                .copied()
                .filter_map(|xy| {
                    if xy.y <= rect_center.y {
                        let mirror_xy = unwrap!(rect.mirror_xy(xy));
                        if valid_tiles_matrix.get(mirror_xy) {
                            // It is better if the towers are not close to the border, as it decreases the average strength.
                            let near_rect_count = [xy, mirror_xy]
                                .into_iter()
                                .filter(|&xy| rect.boundary_dist(xy) < TOWER_OPTIMAL_RANGE as u8)
                                .count();
                            // It is better if the towers are not near the ramparts since it requires an extra rampart on them.
                            let near_rampart_count = [xy, mirror_xy]
                                .into_iter()
                                .filter(|&xy| main_ramparts_dt.get(xy) <= RANGED_ACTION_RANGE)
                                .count();
                            // It is better if the towers are near for ease of filling.
                            let storage_dist = storage_dm.get(xy).saturating_add(storage_dm.get(mirror_xy));
                            return Some((xy, mirror_xy, near_rect_count, near_rampart_count, storage_dist));
                        }
                    }

                    None
                })
                .collect::<Vec<_>>();
            if pair_top_xys.len() >= 3 {
                pair_top_xys.sort_by_key(|&(_, _, near_rect_count, near_rampart_count, storage_dist)| {
                    (near_rect_count, near_rampart_count, storage_dist)
                });

                let solution = [
                    pair_top_xys[0].0,
                    pair_top_xys[0].1,
                    pair_top_xys[1].0,
                    pair_top_xys[1].1,
                    pair_top_xys[2].0,
                    pair_top_xys[2].1,
                ];
                solutions.push(solution);

                if pair_top_xys.len() >= 6 {
                    let solution = [
                        pair_top_xys[3].0,
                        pair_top_xys[3].1,
                        pair_top_xys[4].0,
                        pair_top_xys[4].1,
                        pair_top_xys[5].0,
                        pair_top_xys[5].1,
                    ];
                    solutions.push(solution);

                    let solution = [
                        pair_top_xys[0].0,
                        pair_top_xys[0].1,
                        pair_top_xys[2].0,
                        pair_top_xys[2].1,
                        pair_top_xys[4].0,
                        pair_top_xys[4].1,
                    ];
                    solutions.push(solution);

                    let solution = [
                        pair_top_xys[1].0,
                        pair_top_xys[1].1,
                        pair_top_xys[3].0,
                        pair_top_xys[3].1,
                        pair_top_xys[5].0,
                        pair_top_xys[5].1,
                    ];
                    solutions.push(solution);
                }

                debug!(
                    "Best symmetric pairs {:?}.",
                    pair_top_xys
                        .iter()
                        .map(|&(_, _, near_rect_count, near_rampart_count, storage_dist)| (
                            near_rect_count,
                            near_rampart_count,
                            storage_dist
                        ))
                );
            }

            for xys in solutions.iter() {
                debug!(
                    "Symmetric pairs min damage: {}.",
                    Self::min_tower_damage(xys, &outside_of_main_ramparts)
                );
            }
        });

        let storage_xy = self.storage_xy;
        let mut grow = |center: RoomXY| {
            self.dry_run(|planner| {
                if planner.grow_reachable_structures(Tower, 6, center).is_ok() {
                    let xys = planner.planned_tiles.find_structure_xys(Tower);
                    if let Ok(solution) = xys.try_into() {
                        solutions.push(solution);
                        debug!(
                            "Growth min damage: {}.",
                            Self::min_tower_damage(&solution, &outside_of_main_ramparts)
                        );
                    }
                }
            });
        };

        // Second approach is growing the towers near storage.
        measure_time("grown near storage tower placement", || {
            grow(storage_xy);
        });

        // Third approach is growing the towers near rectangle's center.
        measure_time("grown near center tower placement", || {
            grow(rect_center);
        });

        // Fourth approach is finding more or less evenly spread towers near ramparts.
        measure_time("near ramparts tower placement", || {
            let near_ramparts = main_ramparts_dt
                .iter()
                .filter_map(|(xy, dist)| {
                    (self.interior_dm.get(xy) > 0
                        && self.planned_tiles.get(xy).is_empty()
                        && RANGED_ACTION_RANGE < dist
                        && dist < TOWER_FALLOFF_RANGE as u8 + 2)
                        .then_some(xy)
                })
                .collect::<Vec<_>>();

            if near_ramparts.len() >= 6 {
                // Trying four samples.
                for _ in 0..4 {
                    // Trying from large distances.
                    for min_distance_between in [15, 10, 7, 5, 3, 1] {
                        let mut solution_vec: Vec<RoomXY> = Vec::new();
                        // A total of 24 tries to find at least 6 points sufficiently far away.
                        for i in 0..30 {
                            let xy = near_ramparts[(random() * near_ramparts.len() as f64) as usize];
                            if solution_vec
                                .iter()
                                .copied()
                                .all(|other_xy| other_xy.dist(xy) >= min_distance_between)
                            {
                                solution_vec.push(xy);
                                if solution_vec.len() == 6 {
                                    break;
                                }
                            }
                        }

                        if solution_vec.len() == 6 {
                            let solution = unwrap!(solution_vec.try_into());
                            debug!(
                                "Near ramparts min damage: {}.",
                                Self::min_tower_damage(&solution, &outside_of_main_ramparts)
                            );
                            solutions.push(solution);
                            break;
                        }
                    }
                }
            }
        });

        // Fifth approach is a greedy one.
        measure_time("greedy tower placement", || {
            let mut solution_vec = Vec::new();
            let mut current_damages = outside_of_main_ramparts.iter().map(|_| 0u16).collect::<Vec<_>>();
            for _ in 0..6 {
                let mut best_xy = *unwrap!(valid_tiles.first());
                let mut best_damage = 0u16;
                for &xy in valid_tiles.iter() {
                    if solution_vec.contains(&xy) {
                        continue;
                    }

                    let mut min_damage = u16::MAX;
                    for (i, &outside_xy) in outside_of_main_ramparts.iter().enumerate() {
                        let damage = current_damages[i] + tower_attack_power(outside_xy.dist(xy));
                        min_damage = min(damage, min_damage);
                    }
                    if min_damage > best_damage {
                        best_damage = min_damage;
                        best_xy = xy;
                    }
                }

                solution_vec.push(best_xy);
                for (i, &outside_xy) in outside_of_main_ramparts.iter().enumerate() {
                    current_damages[i] += tower_attack_power(outside_xy.dist(best_xy));
                }
            }

            if solution_vec.len() == 6 {
                let solution = unwrap!(solution_vec.try_into());
                debug!(
                    "Greedy min damage: {}.",
                    Self::min_tower_damage(&solution, &outside_of_main_ramparts)
                );
                solutions.push(solution);
            }
        });

        // Sixth approach is genetic algorithm that tries to improve on top of what previous algorithms spewed out.
        measure_time("genetic algorithm tower placement", || {
            // let mut population = Vec::new();
            let mut population = solutions.clone();
            for _ in 0..100 {
                let mut xys = [RoomXY::default(); 6];
                for i in 0..6 {
                    loop {
                        let xy = valid_tiles[(random() * valid_tiles.len() as f64) as usize];
                        if (0..i).all(|j| xys[j] != xy) {
                            xys[i] = xy;
                            break;
                        }
                    }
                }
                population.push(xys);
            }

            for generation in 0..8 {
                measure_time("sorting", || {
                    // TODO This is by far the most costly part of the algorithm.
                    //      This should be improved by computing only for points which dominate other points.
                    //      If not possible, skip half or more points.
                    population
                        .sort_by_key(|xys| Reverse(RoomPlanner::min_tower_damage(xys, &outside_of_main_ramparts)));
                });
                let mut new_population = Vec::new();

                // Preserve the best.
                for i in 0..min(population.len(), 25) {
                    new_population.push(population[i]);
                }

                if generation % 2 == 1 {
                    measure_time("crossing", || {
                        // Cross the best, each with each.
                        for i in 0..min(population.len(), 13) {
                            for j in 0..min(population.len(), i) {
                                let mut xys = population[i];

                                for k in 0..xys.len() {
                                    if random() > 0.5 {
                                        xys[k] = population[j][k];
                                    }
                                }

                                if (0..6).all(|k| (0..k).all(|l| xys[l] != xys[k])) {
                                    new_population.push(xys);
                                }
                            }
                        }
                    });
                } else {
                    measure_time("mutating", || {
                        // Mutate the best.
                        for i in 0..min(population.len(), 25) {
                            // 2.5 mutations on average, more mutations for better ones.
                            for _ in 0..3 {
                                let mut xys = population[i];

                                for _ in 0..4 {
                                    let j = (random() * 6.0) as usize;
                                    let j_value = xys[j];

                                    let new_j_value = (0..5)
                                        .map(|_| ((random() * 4.0) as i8 + 1, (random() * 4.0) as i8 + 1))
                                        .find_map(|offset| {
                                            j_value.try_add_diff(offset).ok().and_then(|xy| {
                                                (valid_tiles_matrix.get(xy) && !xys.contains(&xy)).then_some(xy)
                                            })
                                        });

                                    if let Some(xy) = new_j_value {
                                        xys[j] = xy;
                                    }
                                }

                                new_population.push(xys);
                            }
                        }
                    });
                }

                population = new_population
                    .into_iter()
                    .collect::<FxHashSet<_>>()
                    .into_iter()
                    .collect::<Vec<_>>();

                let best_damage = unwrap!(population
                    .iter()
                    .copied()
                    .map(|xys| (RoomPlanner::min_tower_damage(&xys, &outside_of_main_ramparts)))
                    .max());
                debug!("Generation {} best damage {}", generation, best_damage);
            }
        });

        let mut scored_solutions = solutions
            .into_iter()
            .map(|xys| (xys, Self::min_tower_damage(&xys, &outside_of_main_ramparts)))
            .collect::<Vec<_>>();
        scored_solutions.sort_by_key(|&(_, score)| score);

        while let Some((solution, min_damage)) = scored_solutions.pop() {
            let obstacles = self
                .interior_dm
                .iter()
                .filter_map(|(xy, dist)| (dist <= 1 || !self.planned_tiles.get(xy).is_passable(true)).then_some(xy))
                .chain(solution.iter().copied());
            let storage_dm = distance_matrix(obstacles, once(self.storage_xy));

            if solution
                .iter()
                .all(|&xy| xy.around().any(|near| storage_dm.get(near) < unreachable_cost()))
            {
                debug!("Chosen towers with minimum damage {}: {:?}.", min_damage, solution);
                self.min_tower_damage = min_damage;

                for xy in solution.iter().copied() {
                    self.planned_tiles
                        .replace_structure(xy, Tower, BasePart::Interior, false);
                }

                self.connect_with_roads(
                    &solution
                        .iter()
                        .map(|&tower_xy| {
                            RoadParameters::new(vec![self.storage_xy], tower_xy, 1, 0, 1, false, BasePart::Interior)
                        })
                        .collect::<Vec<_>>(),
                )?;

                return Ok(());
            }

            // TODO save somewhere the costs matrix
            // TODO consider changing costs in case there are roads not going away from the storage
        }

        Err(StructurePlacementFailure.into())
    }

    fn min_tower_damage(xys: &[RoomXY; 6], outside_of_main_ramparts: &[RoomXY]) -> u16 {
        unwrap!(outside_of_main_ramparts
            .iter()
            .copied()
            .map(|xy| xys.iter().map(|&tower_xy| tower_attack_power(xy.dist(tower_xy))).sum())
            .min())
    }

    /// Uses min-cut to place ramparts around the base and outside according to `BasePart` definition.
    fn place_main_ramparts(&mut self) -> Result<(), Box<dyn Error>> {
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

        self.main_ramparts = grid_min_cut(&min_cut_cost_matrix);

        for xy in self.main_ramparts.iter().copied() {
            self.planned_tiles.merge_structure(xy, Rampart, BasePart::Outside, false)?;
        }

        let interior = interior_matrix(
            self.walls.iter().copied(),
            self.main_ramparts.iter().copied(),
            true,
            true,
        );
        self.interior_dm = distance_matrix(
            empty(),
            interior.iter().filter_map(|(xy, interior)| (!interior).then_some(xy)),
        )
        .map(|xy, dist| if self.terrain.get(xy) == Wall { 0 } else { dist });

        debug!("Placed the main ramparts.");

        Ok(())
    }

    fn place_rampart_roads(&mut self) -> Result<(), Box<dyn Error>> {
        let obstacles = self
            .planned_tiles
            .iter()
            .filter_map(|(xy, tile)| (!tile.is_passable(true) && !tile.grown()).then_some(xy))
            .chain(self.walls.iter().copied());
        let storage_dm = distance_matrix(obstacles, once(self.storage_xy));

        self.main_ramparts.sort_by_key(|&xy| storage_dm.get(xy));

        let mut cost_matrix = RoomMatrix::new(PLAIN_ROAD_COST);
        for (xy, t) in self.terrain.iter() {
            if t == Wall {
                cost_matrix.set(xy, obstacle_cost());
            } else if t == Swamp {
                cost_matrix.set(xy, SWAMP_ROAD_COST);
            }
        }
        for (xy, tile) in self.planned_tiles.iter() {
            if !tile.is_passable(true) && !tile.grown() {
                cost_matrix.set(xy, obstacle_cost());
            } else if tile.structures().road() {
                cost_matrix.set(xy, RAMPART_EXISTING_ROAD_COST);
            }
        }

        for rampart_xy in self.main_ramparts.iter().copied() {
            // TODO optimization if a road is already nearby

            let distances = weighted_distance_matrix(&cost_matrix, once(self.storage_xy));

            if distances.get(rampart_xy) >= unreachable_cost() {
                // debug!("connect_with_roads from {:?} to {:?} / {} D{}\n{}", start_vec, target, real_target, real_target_dist, distances);
                Err(RoadConnectionFailure)?;
            }

            // TODO checkerboard is good, but we should prioritize roads more away from ramparts to make them smaller
            let path = shortest_path_by_matrix_with_preference(&distances, &self.checkerboard, rampart_xy);
            for &xy in &path[0..path.len() - 1] {
                // TODO re-run ramparts at edges or just do it later
                let tile = self.planned_tiles.get(xy);
                self.planned_tiles
                    .replace_structure(xy, Road, BasePart::ProtectedIfInside, false);
                cost_matrix.set(xy, RAMPART_EXISTING_ROAD_COST);
            }
        }

        debug!("Placed rampart roads.");

        Ok(())
    }

    fn place_extra_ramparts(&mut self) -> Result<(), Box<dyn Error>> {
        for (xy, interior_dist) in self.interior_dm.iter() {
            // Checking if ramparts are okay.
            let base_part = self.planned_tiles.get(xy).base_part();
            if (base_part == BasePart::Interior || base_part == BasePart::Connected) && interior_dist == 0 {
                debug!("fail at {}, {:?}\n{}", xy, self.planned_tiles.get(xy), self.interior_dm);
                Err(RampartPlacementFailure)?;
            }

            // Covering some parts in ranged attack range outside or inside the base with ramparts.
            if interior_dist <= RANGED_ACTION_RANGE
                && (base_part == BasePart::Protected
                    || base_part == BasePart::Interior
                    || interior_dist > 0 && base_part == BasePart::ProtectedIfInside)
            {
                self.planned_tiles
                    .merge_structure(xy, Rampart, BasePart::Outside, false)?;
            }
        }

        debug!("Placed extra ramparts.");

        Ok(())
    }

    fn dry_run<F, R>(&mut self, mut f: F) -> R
    where
        F: FnMut(&mut RoomPlanner) -> R,
    {
        let planned_tiles = self.planned_tiles.clone();
        let result = f(self);
        self.planned_tiles = planned_tiles;
        result
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
