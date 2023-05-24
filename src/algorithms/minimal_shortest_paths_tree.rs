use crate::algorithms::distance_matrix::distance_matrix;
use crate::algorithms::matrix_common::MatrixCommon;
use crate::algorithms::room_matrix::RoomMatrix;
use crate::algorithms::weighted_distance_matrix::{obstacle_cost, unreachable_cost};
use crate::geometry::rect::ball;
use crate::geometry::room_xy::RoomXYUtils;
use crate::map_utils::{MultiMapUtils, OrderedMultiMapUtils};
use derive_more::Constructor;
use rustc_hash::{FxHashMap, FxHashSet};
use screeps::RoomXY;
use std::collections::BTreeMap;
use std::iter::once;
use log::debug;
use crate::unwrap;

#[derive(Clone, Debug, Constructor)]
pub struct PathSpec {
    /// Possible choices for the source. One of the closest ones should be used.
    pub sources: Vec<RoomXY>,
    /// Target of the path.
    pub target: RoomXY,
    /// The number of tiles from the target where the path should end.
    pub target_range: u8,
    /// The tile `target_range` from `target` that the path will go towards should be treated as an obstacle.
    pub impassable_at_range: bool,
    /// The extra cost of making the path longer than optimal in the number of tiles it'd have to save.
    pub extra_length_cost: u8,
}

/// Creates a graph of paths between given targets and the nearest source (separately for each target), each path being
/// the shortest (without weights). Tries heuristically to minimize the total sum of weights of used tiles by leading
/// the paths through the same tiles.
pub fn minimal_shortest_paths_tree(
    cost_matrix: &RoomMatrix<u8>,
    preference_matrix: &RoomMatrix<u8>,
    path_specs: &Vec<PathSpec>,
) -> Option<Vec<Vec<RoomXY>>> {
    let obstacles_without_reserved_fields = cost_matrix.find_xy(obstacle_cost()).collect::<Vec<_>>();

    let target_dms_without_reserved_fields = path_specs
        .iter()
        .enumerate()
        .map(|(path_ix, path_spec)| {
            (
                path_ix,
                distance_matrix(
                    obstacles_without_reserved_fields.iter().copied(),
                    ball(path_spec.target, path_spec.target_range).boundary(),
                ),
            )
        })
        .collect::<FxHashMap<_, _>>();

    // Choosing the closest source and real target for each path spec, real target being a specific tile `target_range`
    // from the target.
    // There is an additional check that each target gets its unique closest target. The algorithm is greedy and may
    // fail in some rare cases when the targets are very close to each other.
    let mut closest_sources_and_real_targets = Vec::new();
    for (path_ix, path_spec) in path_specs.iter().enumerate() {
        let real_target_candidates = ball(path_spec.target, path_spec.target_range)
            .boundary()
            .collect::<Vec<_>>();

        let (closest_source, real_target, closest_source_dist, _) = path_spec
            .sources
            .iter()
            .flat_map(|&source| {
                let source_dm_without_reserved_fields =
                    distance_matrix(obstacles_without_reserved_fields.iter().copied(), once(source));
                real_target_candidates
                    .iter()
                    .map(|&real_target| {
                        let nearest_other_target_dist = target_dms_without_reserved_fields
                            .iter()
                            .filter_map(|(&target_ix, dm)| {
                                (path_ix != target_ix
                                    && closest_sources_and_real_targets
                                        .iter()
                                        .all(|(_, other_real_target)| *other_real_target != real_target))
                                .then_some(dm.get(real_target))
                            })
                            .min()
                            .unwrap_or(0);
                        (
                            source,
                            real_target,
                            source_dm_without_reserved_fields.get(real_target),
                            nearest_other_target_dist,
                        )
                    })
                    .collect::<Vec<_>>()
                    .into_iter()
            })
            .min_by_key(|(_, _, source_dist, nearest_other_target_dist)| (*source_dist, *nearest_other_target_dist))?;
        if closest_source_dist >= unreachable_cost() {
            None?;
        }

        // debug!("{} -> {} / {}", closest_source, real_target, path_spec.target);

        closest_sources_and_real_targets.push((closest_source, real_target));
    }

    // Obstacles and reserved fields - real targets.
    let obstacles = cost_matrix
        .find_xy(obstacle_cost())
        .chain(
            closest_sources_and_real_targets
                .iter()
                .zip(path_specs.iter())
                .filter_map(|((_, real_target), path_spec)| path_spec.impassable_at_range.then_some(*real_target)),
        )
        .collect::<Vec<_>>();

    let target_dms = (0..path_specs.len())
        .map(|path_ix| {
            let (closest_source, real_target) = closest_sources_and_real_targets[path_ix];
            distance_matrix(
                obstacles.iter().copied().filter(|&xy| xy != closest_source && xy != real_target),
                once(real_target),
            )
        })
        .collect::<Vec<_>>();
    let source_dms = (0..path_specs.len())
        .map(|path_ix| {
            let (closest_source, real_target) = closest_sources_and_real_targets[path_ix];
            distance_matrix(
                obstacles.iter().copied().filter(|&xy| xy != real_target),
                once(closest_source),
            )
        })
        .collect::<Vec<_>>();

    let mut path_areas = Vec::new();

    for path_ix in 0..path_specs.len() {
        let path_area = shortest_path_area(&source_dms[path_ix], &target_dms[path_ix]);
        path_areas.push(path_area);
    }

    let mut path_xys = FxHashSet::default();

    let mut paths = Vec::new();

    for (path_ix, path_spec) in path_specs.iter().enumerate() {
        let mut number_of_areas = RoomMatrix::new(0u8);

        for path_area in path_areas.iter().skip(path_ix + 1) {
            for &xy in path_area.iter() {
                number_of_areas.set(xy, number_of_areas.get(xy) + 1);
            }
        }

        let (closest_source, real_target) = closest_sources_and_real_targets[path_ix];
        let target_dm = &target_dms[path_ix];

        // Implementation of Dijkstra with respect to the cost matrix and penalizing not following decreasing distance
        // from the source.
        let mut distances = RoomMatrix::new(unreachable_cost());
        let mut queue: BTreeMap<u32, Vec<RoomXY>> = BTreeMap::new();
        let mut prev = FxHashMap::default();

        distances.set(closest_source, 0u32);
        queue.push_or_insert(0, closest_source);

        let mut real_target_dist = unreachable_cost();

        debug!("Finding route {} -> {} ({})", closest_source, real_target, path_spec.target);

        while let Some((dist, xy)) = queue.pop_from_first() {
            if dist >= real_target_dist {
                break;
            }
            if distances.get(xy) == dist {
                for near in xy.around() {
                    if target_dm.get(near) < unreachable_cost() {
                        let dist_diff = target_dm.get(near) as i8 - target_dm.get(xy) as i8 + 1;
                        assert!(dist_diff >= 0);
                        assert!(dist_diff <= 2);
                        let extra_dist_cost = (dist_diff as u32 * path_spec.extra_length_cost as u32) << 14;
                        let near_cost = if path_xys.contains(&near) {
                            extra_dist_cost
                        } else {
                            let shared_cost = (((cost_matrix.get(near) as u32) << 8) + preference_matrix.get(near) as u32) << 6;
                            extra_dist_cost + shared_cost / (number_of_areas.get(near) + 1) as u32
                        };
                        let new_dist = dist.saturating_add(near_cost);
                        let near_dist = distances.get(near);
                        if new_dist < near_dist {
                            distances.set(near, new_dist);
                            prev.insert(near, xy);
                            if near == real_target {
                                debug!("{} -> {} at cost {} (dist_diff {} extra_dist_cost {} total {})", xy, near, near_cost, dist_diff, extra_dist_cost, new_dist);
                                real_target_dist = new_dist;
                            } else {
                                debug!("{} -> {} at cost {} (dist_diff {} extra_dist_cost {} total {})", xy, near, near_cost, dist_diff, extra_dist_cost, new_dist);
                                queue.push_or_insert(new_dist, near);
                            }
                        }
                    }
                }
            }
        }

        if real_target_dist >= unreachable_cost() {
            None?;
        }

        let source_dm = &source_dms[path_ix];
        let mut path = vec![real_target];
        let mut current = real_target;
        path_xys.insert(real_target);
        while current != closest_source {
            debug!("{}", current);
            current = *unwrap!(prev.get(&current));
            path.push(current);
            path_xys.insert(current);
        }
        path.reverse();

        debug!("Found route {} -> {} ({})\n{:?}\n{}", closest_source, real_target, path_spec.target, path, distances);

        paths.push(path);
    }

    Some(paths)
}

fn shortest_path_area(source_dm: &RoomMatrix<u8>, target_dm: &RoomMatrix<u8>) -> Vec<RoomXY> {
    let mut min_total_dist = unreachable_cost();
    let combined_dm = source_dm.map(|xy, source_dist| {
        let total_dist = source_dist.saturating_add(target_dm.get(xy));
        if total_dist < min_total_dist {
            min_total_dist = total_dist;
        };
        total_dist
    });
    if min_total_dist < unreachable_cost() {
        combined_dm.find_xy(min_total_dist).collect()
    } else {
        vec![]
    }
}
