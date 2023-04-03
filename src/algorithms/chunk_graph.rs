use crate::algorithms::distance_matrix::{distance_matrix, restricted_distance_matrix};
use crate::algorithms::distance_transform::distance_transform;
use crate::algorithms::matrix_common::MatrixCommon;
use crate::algorithms::room_matrix::RoomMatrix;
use crate::consts::OBSTACLE_COST;
use crate::geometry::rect::ball;
use crate::geometry::room_xy::RoomXYUtils;
use petgraph::graph::NodeIndex;
use petgraph::prelude::StableGraph;
use petgraph::Undirected;
use rustc_hash::FxHashMap;
use screeps::RoomXY;
use std::cmp::Reverse;
use tap::prelude::*;

pub type ChunkId = NodeIndex<u16>;

pub struct ChunkGraph {
    /// The assignment tiles -> chunks.
    pub xy_chunks: RoomMatrix<ChunkId>,
    /// Sizes of chunks.
    chunk_sizes: FxHashMap<ChunkId, u16>,
    /// A graph of chunks with edges between neighboring chunks.
    /// Nodes are labelled by chunk centers. Weights of edges are the distance between chunk
    /// centers.
    pub graph: StableGraph<RoomXY, u8, Undirected, u16>,
}

pub fn invalid_chunk_node_index() -> ChunkId {
    NodeIndex::end()
}

/// The obstacles matrix should have OBSTACLE_COST on obstacle tiles and 0 on the rest.
pub fn chunk_graph(terrain: &RoomMatrix<u8>, chunk_radius: u8) -> ChunkGraph {
    let exits: Vec<RoomXY> = terrain
        .exits()
        .filter_map(|(xy, value)| (value == 0).then_some(xy))
        .collect();

    let exit_distances = distance_matrix(exits.iter().copied(), terrain.find_xy(OBSTACLE_COST));

    let dt = terrain
        .map(|t| OBSTACLE_COST - t)
        .tap_mut(|t| distance_transform(t));

    let tiles = terrain
        .find_xy(0)
        .collect::<Vec<RoomXY>>()
        .tap_mut(|v| v.sort_by_key(|a| Reverse(exit_distances.get(*a))));

    let mut chunk_center_distances = RoomMatrix::new(255);

    let mut xy_chunks = RoomMatrix::new(invalid_chunk_node_index());
    let mut chunk_sizes = FxHashMap::default();
    let approximate_number_of_nodes =
        2500 / (4 * (chunk_radius as usize) * (chunk_radius as usize));
    let mut graph =
        StableGraph::with_capacity(approximate_number_of_nodes, approximate_number_of_nodes * 2);

    // We iterate over all non-obstacle sorted decreasingly by distance to nearest exit.
    for xy in tiles.iter().copied() {
        if xy_chunks.get(xy) != invalid_chunk_node_index() {
            continue;
        }

        // The tile is a potential chunk center if it is not within a chunk.
        // We select the furthest one and then travel chunk_radius towards a more open area
        // using distance transform and only through unassigned fields.
        let chunk_center = {
            let mut chunk_center = xy;
            'finding_chunk_radius: for _ in 0..chunk_radius {
                for near in chunk_center.around() {
                    if terrain.get(near) != OBSTACLE_COST
                        && xy_chunks.get(near) == invalid_chunk_node_index()
                        && dt.get(near) > dt.get(chunk_center)
                    {
                        chunk_center = near;
                        continue 'finding_chunk_radius;
                    }
                }
                for near in chunk_center.around() {
                    if terrain.get(near) != OBSTACLE_COST
                        && xy_chunks.get(near) == invalid_chunk_node_index()
                        && dt.get(near) == dt.get(chunk_center)
                        && exit_distances.get(near) < exit_distances.get(chunk_center)
                    {
                        chunk_center = near;
                        continue 'finding_chunk_radius;
                    }
                }
                break;
            }
            chunk_center
        };

        let chunk_ball = ball(chunk_center, chunk_radius);

        let chunk_distance_matrix = restricted_distance_matrix(
            [chunk_center].into_iter(),
            chunk_ball
                .iter()
                .filter(|xy| terrain.get(*xy) == OBSTACLE_COST),
            chunk_ball,
            chunk_radius,
        );

        let chunk_id: ChunkId = graph.add_node(chunk_center);
        chunk_sizes.insert(chunk_id, 0);

        for (xy, chunk_center_distance) in chunk_distance_matrix.iter() {
            if chunk_center_distance < chunk_center_distances.get(xy) {
                let current_chunk = xy_chunks.get(xy);
                chunk_center_distances.set(xy, chunk_center_distance);
                xy_chunks.set(xy, chunk_id);
                if current_chunk != invalid_chunk_node_index() {
                    *chunk_sizes.get_mut(&current_chunk).unwrap() -= 1;
                }
                *chunk_sizes.get_mut(&chunk_id).unwrap() += 1;
            }
        }
    }

    let min_chunk_size = (chunk_radius as u16) * (chunk_radius as u16);

    // We remove all chunks that are smaller than min_chunk_size and save all remaining chunks'
    // centers.
    let mut layer = Vec::new();
    chunk_sizes.retain(|&chunk_id, &mut size| {
        if size >= min_chunk_size {
            layer.push((*graph.node_weight(chunk_id).unwrap(), chunk_id));
            true
        } else {
            let removal = graph.remove_node(chunk_id);
            debug_assert!(removal.is_some());
            false
        }
    });

    // We remove nodes from the graph too. We do not update xy_chunks, as this will be taken care
    // of in the next step.
    for chunk_id in graph.node_indices() {
        if !chunk_sizes.contains_key(&chunk_id) {
            chunk_sizes.remove(&chunk_id);
        }
    }

    // We have decided on chunk centers. But there are unassigned tiles and potentially tiles
    // disconnected from the rest of chunks now. We solve this by running a BFS from chunk centers.
    xy_chunks = RoomMatrix::new(invalid_chunk_node_index());
    for (xy, chunk_id) in layer.iter().copied() {
        xy_chunks.set(xy, chunk_id);
        *chunk_sizes.get_mut(&chunk_id).unwrap() = 1;
    }

    // Performing a BFS from chunk centers to remove any tiles that are disconnected from them.
    // Note that this will not remove the center tile, so none of the chunks will become empty.
    let mut k = 0;
    while !layer.is_empty() {
        let mut next_layer = Vec::new();

        // Reversing the layer every second iteration so that no single direction will be prioritized.
        if k % 2 == 1 {
            layer.reverse();
        }

        for (xy, chunk_id) in layer.into_iter() {
            for near in xy.around() {
                if terrain.get(near) != OBSTACLE_COST {
                    let near_chunk_id = xy_chunks.get(near);
                    if near_chunk_id == invalid_chunk_node_index() {
                        next_layer.push((near, chunk_id));
                        xy_chunks.set(near, chunk_id);
                        *chunk_sizes.get_mut(&chunk_id).unwrap() += 1;
                    } else {
                        graph.update_edge(chunk_id, near_chunk_id, 1);
                    }
                }
            }
        }

        layer = next_layer;
        k += 1;
    }

    ChunkGraph {
        xy_chunks,
        chunk_sizes,
        graph,
    }
}

#[cfg(test)]
mod test {
    use crate::algorithms::chunk_graph::chunk_graph;
    use crate::algorithms::matrix_common::MatrixCommon;
    use crate::algorithms::room_matrix::RoomMatrix;
    use crate::consts::OBSTACLE_COST;
    use crate::geometry::rect::{room_rect, Rect};
    use more_asserts::{assert_ge, assert_le};
    use std::error::Error;

    #[test]
    fn test_chunk_graph_on_empty_room() {
        let terrain = RoomMatrix::new(0);

        let chunks = chunk_graph(&terrain, 5);
        assert_le!(chunks.graph.node_count(), 60);
        assert_ge!(chunks.graph.node_count(), 20);
        assert_le!(chunks.graph.edge_count(), 250);
        assert_ge!(chunks.graph.edge_count(), 60);
    }

    #[test]
    fn test_chunk_graph_on_room_with_single_obstacle() -> Result<(), Box<dyn Error>> {
        let mut terrain = RoomMatrix::new(0);

        let rect = Rect::new((10, 10).try_into()?, (40, 40).try_into()?)?;
        for xy in rect.iter() {
            terrain.set(xy, OBSTACLE_COST);
        }

        let chunks = chunk_graph(&terrain, 5);
        assert_le!(chunks.graph.node_count(), 25);
        assert_ge!(chunks.graph.node_count(), 15);
        Ok(())
    }

    #[test]
    fn test_chunk_graph_on_checkerboard() -> Result<(), Box<dyn Error>> {
        let mut terrain = RoomMatrix::new(0);

        for xy in room_rect().iter() {
            if xy.x.u8() % 2 == xy.y.u8() % 2 {
                terrain.set(xy, OBSTACLE_COST);
            }
        }

        let chunks = chunk_graph(&terrain, 5);
        assert_le!(chunks.graph.node_count(), 50);
        assert_ge!(chunks.graph.node_count(), 10);
        Ok(())
    }
}
