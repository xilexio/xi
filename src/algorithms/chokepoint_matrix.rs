use crate::algorithms::chunk_graph::{invalid_chunk_node_index, ChunkGraph};
use crate::algorithms::distance_transform::{directional_distance_transform, distance_transform};
use crate::algorithms::matrix_common::MatrixCommon;
use crate::algorithms::room_matrix::RoomMatrix;
use crate::algorithms::weighted_distance_matrix::obstacle_cost;
use crate::geometry::direction::{mul_offsets, rotate_clockwise, rotate_counterclockwise, OFFSET_BY_DIRECTION};
use crate::geometry::rect::Rect;
use crate::geometry::room_xy::RoomXYUtils;
use crate::unwrap;
use rustc_hash::FxHashSet;
use screeps::{Direction, RoomXY, ROOM_SIZE};

/// Checks chokepoint in a single direction. The direction is outward vector from the protected area.
/// The result is a matrix with a tuple `(a, b)` for each tile with `a` being the chokepoint width or
/// `obstacle_cost` for obstacles and `b` being `min_separated_tiles` for chokepoints wider than `max_chokepoint_width`
/// or when reaching an exit tile towards "outside" according to the direction or the number of tiles
/// separated by the chokepoint tiles, capped at `min_separated_tiles`.
pub fn chokepoint_matrix(
    chunk_graph: &ChunkGraph,
    direction: Direction,
    max_chokepoint_width: u8,
    min_separated_tiles: u8,
) -> RoomMatrix<(u8, u8)> {
    let mut result = RoomMatrix::new((obstacle_cost(), 0));

    // TODO this can be further optimized using exit distance matrix - if we travel to any tile that is closer to
    //  exit than chokepoint tiles and is on the "outside" then it is not too small.

    // Various preprocessing.
    // The directions towards which chokepoint tiles go based on the direction to "outside".
    let check_directions = match direction {
        Direction::Top => [Direction::Left, Direction::Right],
        Direction::TopRight => [Direction::Bottom, Direction::Left],
        Direction::Right => [Direction::Top, Direction::Bottom],
        Direction::BottomRight => [Direction::Top, Direction::Left],
        Direction::Bottom => [Direction::Right, Direction::Left],
        Direction::BottomLeft => [Direction::Top, Direction::Right],
        Direction::Left => [Direction::Bottom, Direction::Top],
        Direction::TopLeft => [Direction::Right, Direction::Bottom],
    };

    let mut dt = chunk_graph.xy_chunks.map(|_, chunk| {
        if chunk == invalid_chunk_node_index() {
            0
        } else {
            ROOM_SIZE
        }
    });
    let mut dt_dir1 = dt.clone();
    let mut dt_dir2 = dt.clone();
    distance_transform(&mut dt, 1);
    // Directional distance transform gives distance in the reverse direction from last obstacle.
    directional_distance_transform(&mut dt_dir1, -check_directions[0], ROOM_SIZE);
    directional_distance_transform(&mut dt_dir2, -check_directions[1], ROOM_SIZE);

    let dfs_search_directions = [
        -direction,
        rotate_counterclockwise(rotate_counterclockwise(rotate_counterclockwise(direction))),
        rotate_clockwise(rotate_clockwise(rotate_clockwise(direction))),
        rotate_counterclockwise(rotate_counterclockwise(direction)),
        rotate_clockwise(rotate_clockwise(direction)),
        rotate_counterclockwise(direction),
        rotate_clockwise(direction),
        direction,
    ];

    let sufficient_dt_dist = (min_separated_tiles as f32).sqrt().ceil() as u8;

    let rect = unsafe {
        Rect::unchecked_new(
            RoomXY::unchecked_new(1, 1),
            RoomXY::unchecked_new(ROOM_SIZE - 2, ROOM_SIZE - 2),
        )
    };
    'main_loop: for xy in rect.iter() {
        let dist1 = dt_dir1.get(xy);
        let dist2 = dt_dir2.get(xy);
        // debug!(
        //     "{} {} {} {}",
        //     xy,
        //     dist1,
        //     dist2,
        //     dist1.saturating_add(dist2).saturating_sub(1)
        // );
        if dist1 == 0 {
            // Obstacle.
            result.set(xy, (obstacle_cost(), 0));
        } else {
            debug_assert!(dist2 > 0);
            let chokepoint_width = dist1.saturating_add(dist2).saturating_sub(1);
            if chokepoint_width >= max_chokepoint_width {
                // Chokepoint too wide to be treated as a chokepoint.
                result.set(xy, (chokepoint_width, min_separated_tiles));
            } else {
                // Thin enough chokepoint. However, it is unknown if it separates enough area.

                // DFS to count the number of separated tiles.
                // Stops early if it detects a chunk not present in the chokepoint tiles.
                // Also stops early if it detects distance transform with sufficient size.
                // Prefers the direction reverse to the direction in the function argument.

                // Creating the initial DFS stack from the tiles within the area separated from "outside" by the
                // chokepoint tiles, where the "outside" is defined based on the main direction.
                // First computing the chokepoint tiles.
                let mut chokepoint_xys = Vec::default();

                for check_dist in 0..dist1 - 1 {
                    let chokepoint_xy = unwrap!(xy.try_add_diff(mul_offsets(
                        OFFSET_BY_DIRECTION[check_directions[0] as usize],
                        (dist1 - check_dist - 1) as i8
                    )));
                    chokepoint_xys.push(chokepoint_xy);
                }

                for check_dist in 0..dist2 {
                    let chokepoint_xy = unwrap!(xy.try_add_diff(mul_offsets(
                        OFFSET_BY_DIRECTION[check_directions[1] as usize],
                        check_dist as i8
                    )));
                    chokepoint_xys.push(chokepoint_xy);
                }

                let direction_offset = OFFSET_BY_DIRECTION[direction as usize];
                let mut dfs_stack = chokepoint_xys
                    .iter()
                    .copied()
                    .filter_map(|chokepoint_xy| chokepoint_xy.try_add_diff(direction_offset).ok())
                    .collect::<Vec<_>>();

                // There is an edge case where it can start diagonally.
                // The order of checked directions above is important here.
                if !chokepoint_xys.is_empty() && check_directions[0] == -check_directions[1] {
                    let extra_xys = chokepoint_xys
                        .first()
                        .into_iter()
                        .filter_map(|&chokepoint_xy| {
                            chokepoint_xy
                                .try_add_diff(OFFSET_BY_DIRECTION[rotate_counterclockwise(direction) as usize])
                                .ok()
                        })
                        .chain(chokepoint_xys.last().into_iter().filter_map(|&chokepoint_xy| {
                            chokepoint_xy
                                .try_add_diff(OFFSET_BY_DIRECTION[rotate_clockwise(direction) as usize])
                                .ok()
                        }))
                        .filter(|chokepoint_xy| !dfs_stack.contains(chokepoint_xy))
                        .collect::<Vec<_>>();
                    dfs_stack.extend(extra_xys.into_iter());
                }

                if dfs_stack.is_empty() {
                    // The edge case where the chokepoint tiles are glued to the wall.
                    result.set(xy, (chokepoint_width, 0));
                    continue;
                }

                // Clearning out the obstacles from the stack.
                dfs_stack.retain(|&xy| chunk_graph.xy_chunks.get(xy) != invalid_chunk_node_index());

                let chokepoint_chunks = chokepoint_xys
                    .iter()
                    .copied()
                    .map(|xy| chunk_graph.xy_chunks.get(xy))
                    .collect::<FxHashSet<_>>();
                let chokepoint_xy_set = chokepoint_xys.into_iter().collect::<FxHashSet<_>>();
                let mut separated_tiles = dfs_stack.iter().copied().collect::<FxHashSet<_>>();

                // We start counting the separated tiles using DFS.
                while let Some(search_xy) = dfs_stack.pop() {
                    if search_xy.exit_distance() == 0 {
                        // We reached an exit, so this is not a chokepoint in the main direction.
                        result.set(xy, (chokepoint_width, min_separated_tiles));
                        continue 'main_loop;
                    }

                    // We start searching from least preferred directions so that the preferred direction will end up
                    // on top of the DFS stack.
                    for search_direction in dfs_search_directions {
                        // It is safe to unwrap here since xy was not at the exit tile.
                        let around_xy = search_xy
                            .try_add_diff(OFFSET_BY_DIRECTION[search_direction as usize])
                            .unwrap();
                        let chunk = chunk_graph.xy_chunks.get(around_xy);

                        if chunk == invalid_chunk_node_index()
                            || separated_tiles.contains(&around_xy)
                            || chokepoint_xy_set.contains(&around_xy)
                        {
                            // Skipping obstacles, already visited tiles and chokepoint tiles.
                            continue;
                        }

                        // Chunk-based short-circuit optimization.
                        if !chokepoint_chunks.contains(&chunk) {
                            debug_assert!(
                                separated_tiles.len() as u16 + chunk_graph.chunk_sizes.get(&chunk).unwrap()
                                    >= min_separated_tiles as u16
                            );
                            // We reached another chunk and it is large enough to deduce that the number of
                            // separated tiles is large enough.
                            result.set(xy, (chokepoint_width, min_separated_tiles));
                            continue 'main_loop;
                        }

                        // Distance-transform-based optimization.
                        if dt.get(around_xy) >= sufficient_dt_dist {
                            // We reached a point that alone has enough space around it to account for all separated
                            // tiles. We have no information on how many tiles are already covered by the DFS.
                            result.set(xy, (chokepoint_width, min_separated_tiles));
                            continue 'main_loop;
                        }

                        separated_tiles.insert(around_xy);
                        if separated_tiles.len() as u8 >= min_separated_tiles {
                            // There are enough separated points now.
                            result.set(xy, (chokepoint_width, min_separated_tiles));
                            continue 'main_loop;
                        }

                        dfs_stack.push(around_xy);
                    }
                }

                // The separated area turned out to be not large enough.
                result.set(xy, (chokepoint_width, separated_tiles.len() as u8));
            }
        }
    }

    result
}
