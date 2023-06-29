use crate::algorithms::distance_matrix::distance_matrix;
use crate::algorithms::matrix_common::MatrixCommon;
use crate::algorithms::room_matrix::RoomMatrix;
use crate::algorithms::weighted_distance_matrix::{obstacle_cost, unreachable_cost};
use crate::geometry::rect::ball;
use crate::geometry::room_xy::RoomXYUtils;
use crate::utils::map_utils::{MultiMapUtils, OrderedMultiMapUtils};
use derive_more::Constructor;
use rustc_hash::{FxHashMap, FxHashSet};
use screeps::RoomXY;
use std::collections::BTreeMap;

#[derive(Clone, Debug, Constructor)]
pub struct PathSpec {
    /// Possible choices for the source. One of the closest ones should be used.
    pub sources: Vec<RoomXY>,
    /// Target of the path.
    pub target: RoomXY,
    /// The number of tiles from the target where the path should end.
    pub target_range: u8,
    /// The tile `target_range` from `target` that the path will go towards should be treated as an obstacle.
    pub impassable_target: bool,
    /// The extra cost of making the path longer than optimal in the number of tiles it'd have to save.
    pub extra_length_cost: f32,
}

impl PathSpec {
    fn source_dm(&self, obstacles: &[RoomXY]) -> RoomMatrix<u8> {
        distance_matrix(obstacles.iter().copied(), self.sources.iter().copied())
    }

    fn target_dm(&self, obstacles: &[RoomXY], cost_matrix: &RoomMatrix<u8>) -> RoomMatrix<u8> {
        distance_matrix(
            obstacles.iter().copied().filter(|xy| !self.sources.contains(xy)),
            ball(self.target, self.target_range)
                .boundary()
                .filter(|&xy| cost_matrix.get(xy) < unreachable_cost()),
        )
    }
}

/// Creates a graph of paths between given targets and the nearest source (separately for each target), each path being
/// the shortest (without weights). Tries heuristically to minimize the total sum of weights of used tiles by leading
/// the paths through the same tiles.
pub fn minimal_shortest_paths_tree(
    cost_matrix: &RoomMatrix<u8>,
    preference_matrix: &RoomMatrix<u8>,
    path_specs: &Vec<PathSpec>,
    sqrt_target_scaling: bool,
    dist_tolerance: u8,
) -> Option<Vec<Vec<RoomXY>>> {
    // Obstacles and reserved fields - real targets.
    let mut obstacles = cost_matrix.find_xy(obstacle_cost()).collect::<Vec<_>>();

    let (path_ixs, mut path_areas): (Vec<_>, Vec<_>) = {
        let mut path_areas_data = path_specs
            .iter()
            .enumerate()
            .map(|(path_ix, path_spec)| {
                shortest_path_area(
                    &path_spec.source_dm(&obstacles),
                    &path_spec.target_dm(&obstacles, &cost_matrix),
                    dist_tolerance,
                )
                .map(|(path_area, dist)| (path_ix, path_area, dist))
            })
            .collect::<Option<Vec<_>>>()?;
        // TODO Detecting continuous (maybe with tolerance) fragments and selecting roads more or less in
        //      the middle will most likely result in less roads.
        path_areas_data.sort_by_key(|(i, _, dist)| *dist);
        path_areas_data
            .into_iter()
            .map(|(path_ix, path_area, _)| (path_ix, path_area))
            .unzip()
    };

    let mut path_xys = FxHashSet::default();

    let mut paths = path_ixs.iter().map(|_| Vec::new()).collect::<Vec<_>>();

    // debug!("Finding routes with cost matrix\n{}", cost_matrix);

    for i in 0..path_areas.len() {
        let path_ix = path_ixs[i];
        let path_spec: &PathSpec = &path_specs[path_ix];
        let target_dm = path_spec.target_dm(&obstacles, cost_matrix);

        let mut number_of_areas = RoomMatrix::new(0u8);
        for path_area in path_areas.iter().skip(i + 1) {
            for &xy in path_area.iter() {
                number_of_areas.set(xy, number_of_areas.get(xy) + 1);
            }
        }

        // Implementation of Dijkstra with respect to the cost matrix and penalizing not following decreasing distance
        // from the source.
        let mut distances = RoomMatrix::new(unreachable_cost());
        let mut queue: BTreeMap<u32, Vec<RoomXY>> = BTreeMap::new();
        let mut prev = FxHashMap::default();

        for &source in path_spec.sources.iter() {
            distances.set(source, 0u32);
            queue.push_or_insert(0, source);
        }

        let mut best_target = None;
        let mut best_target_dist = unreachable_cost();

        // debug!(
        //     "Finding route {:?} -> {} / {}\n{}",
        //     path_spec.sources, path_spec.target, path_spec.target_range, number_of_areas
        // );

        while let Some((dist, xy)) = queue.pop_from_first() {
            if dist >= best_target_dist {
                break;
            }
            // debug!("Processing from {} at dist {}.", xy, dist);
            if distances.get(xy) == dist {
                for near in xy.around() {
                    if target_dm.get(near) < unreachable_cost() {
                        let dist_diff = target_dm.get(near) as i8 - target_dm.get(xy) as i8 + 1;
                        assert!(dist_diff >= 0);
                        assert!(dist_diff <= 2);
                        let extra_dist_cost =
                            ((dist_diff as f32 * path_spec.extra_length_cost) * (2 << 14) as f32) as u32;
                        let near_cost = if cost_matrix.get(near) == 0 || path_xys.contains(&near) {
                            extra_dist_cost
                        } else {
                            let shared_cost =
                                (((cost_matrix.get(near) as u32) << 8) + preference_matrix.get(near) as u32) << 3;
                            let shared_targets = (number_of_areas.get(near) + 1) as f32;
                            let sharing_factor = if sqrt_target_scaling {
                                shared_targets.sqrt()
                            } else {
                                shared_targets
                            };
                            extra_dist_cost + (shared_cost as f32 / sharing_factor) as u32
                        };
                        let new_dist = dist.saturating_add(near_cost);
                        let near_dist = distances.get(near);
                        if new_dist < near_dist {
                            distances.set(near, new_dist);
                            prev.insert(near, xy);
                            if target_dm.get(near) == 0 {
                                // debug!(
                                //     "solution {} -> {} at cost {} (dist_diff {} extra_dist_cost {} total {})",
                                //     xy, near, near_cost, dist_diff, extra_dist_cost, new_dist
                                // );
                                if new_dist < best_target_dist {
                                    best_target = Some(near);
                                    best_target_dist = new_dist;
                                }
                            } else {
                                // debug!(
                                //     "{} -> {} at cost {} (dist_diff {} extra_dist_cost {} total {})",
                                //     xy, near, near_cost, dist_diff, extra_dist_cost, new_dist
                                // );
                                queue.push_or_insert(new_dist, near);
                            }
                        }
                    }
                }
            }
        }

        let target = best_target?;

        let mut path = vec![target];
        let mut current = target;
        path_xys.insert(target);
        while let Some(current_prev) = prev.get(&current) {
            current = *current_prev;
            path.push(current);
            path_xys.insert(current);
        }
        path.reverse();

        // debug!(
        //     "Found route {} -> {} ({:?} -> {} / {})\n{:?}\n{}",
        //     path[0], target, path_spec.sources, path_spec.target, path_spec.target_range, path, distances
        // );

        paths[path_ix] = path;

        // If the target is marked as impassable, paths with path area going through it need updating.
        if path_spec.impassable_target {
            obstacles.push(target);
            for j in i + 1..path_areas.len() {
                if path_areas[j].contains(&target) {
                    let ps: &PathSpec = &path_specs[path_ixs[j]];
                    path_areas[j] = shortest_path_area(
                        &ps.source_dm(&obstacles),
                        &ps.target_dm(&obstacles, cost_matrix),
                        dist_tolerance,
                    )?
                    .0;
                }
            }
        }
    }

    Some(paths)
}

fn shortest_path_area(
    source_dm: &RoomMatrix<u8>,
    target_dm: &RoomMatrix<u8>,
    dist_tolerance: u8,
) -> Option<(FxHashSet<RoomXY>, u8)> {
    let mut min_total_dist = unreachable_cost();
    let combined_dm = source_dm.map(|xy, source_dist| {
        let total_dist = source_dist.saturating_add(target_dm.get(xy));
        if total_dist < min_total_dist {
            min_total_dist = total_dist;
        };
        total_dist
    });
    if min_total_dist < unreachable_cost() {
        Some((
            combined_dm
                .iter()
                .filter_map(|(xy, dist)| (dist <= min_total_dist + dist_tolerance).then_some(xy))
                .collect(),
            min_total_dist,
        ))
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use crate::algorithms::minimal_shortest_paths_tree::{minimal_shortest_paths_tree, PathSpec};
    use crate::algorithms::room_matrix::RoomMatrix;

    #[test]
    fn test_minimal_shortest_paths_tree_single() {
        let cost_matrix = RoomMatrix::new(1u8);
        let preference_matrix = RoomMatrix::new(1u8).map(|xy, _| xy.x.u8());
        let paths = minimal_shortest_paths_tree(
            &cost_matrix,
            &preference_matrix,
            &vec![PathSpec::new(
                vec![(20, 20).try_into().unwrap(), (30, 30).try_into().unwrap()],
                (23, 25).try_into().unwrap(),
                0,
                false,
                1.0,
            )],
            false,
            0,
        );

        let paths_unwrapped = paths.unwrap();
        assert_eq!(paths_unwrapped.len(), 1);
        assert_eq!(
            paths_unwrapped[0],
            vec![
                (20, 20).try_into().unwrap(),
                (19, 21).try_into().unwrap(),
                (20, 22).try_into().unwrap(),
                (21, 23).try_into().unwrap(),
                (22, 24).try_into().unwrap(),
                (23, 25).try_into().unwrap(),
            ]
        );
    }

    #[test]
    fn test_minimal_shortest_paths_tree_two_paths() {
        let cost_matrix = RoomMatrix::new(1u8);
        let preference_matrix = RoomMatrix::new(1u8);
        let paths = minimal_shortest_paths_tree(
            &cost_matrix,
            &preference_matrix,
            &vec![
                PathSpec::new(
                    vec![(20, 20).try_into().unwrap()],
                    (22, 23).try_into().unwrap(),
                    0,
                    false,
                    1.0,
                ),
                PathSpec::new(
                    vec![(22, 20).try_into().unwrap()],
                    (20, 23).try_into().unwrap(),
                    0,
                    false,
                    1.0,
                ),
            ],
            false,
            0,
        );

        let paths_unwrapped = paths.unwrap();
        assert_eq!(paths_unwrapped.len(), 2);
        assert_eq!(
            paths_unwrapped[0],
            vec![
                (20, 20).try_into().unwrap(),
                (21, 21).try_into().unwrap(),
                (21, 22).try_into().unwrap(),
                (22, 23).try_into().unwrap(),
            ]
        );
        assert_eq!(
            paths_unwrapped[1],
            vec![
                (22, 20).try_into().unwrap(),
                (21, 21).try_into().unwrap(),
                (21, 22).try_into().unwrap(),
                (20, 23).try_into().unwrap(),
            ]
        );
    }

    #[test]
    fn test_minimal_shortest_paths_at_range() {
        let cost_matrix = RoomMatrix::new(1u8);
        let preference_matrix = RoomMatrix::new(1u8);
        let paths = minimal_shortest_paths_tree(
            &cost_matrix,
            &preference_matrix,
            &vec![PathSpec::new(
                vec![(20, 20).try_into().unwrap()],
                (25, 25).try_into().unwrap(),
                2,
                false,
                1.0,
            )],
            false,
            0,
        );

        let paths_unwrapped = paths.unwrap();
        assert_eq!(paths_unwrapped.len(), 1);
        assert_eq!(
            paths_unwrapped[0],
            vec![
                (20, 20).try_into().unwrap(),
                (21, 21).try_into().unwrap(),
                (22, 22).try_into().unwrap(),
                (23, 23).try_into().unwrap(),
            ]
        );
    }
}
