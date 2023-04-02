use crate::algorithms::distance_matrix::{distance_matrix, restricted_distance_matrix};
use crate::algorithms::distance_transform::distance_transform;
use crate::algorithms::matrix_common::MatrixCommon;
use crate::algorithms::room_matrix::RoomMatrix;
use crate::consts::OBSTACLE_COST;
use crate::geometry::rect::ball;
use crate::geometry::room_xy::RoomXYUtils;
use petgraph::{Graph, Undirected};
use screeps::RoomXY;
use std::cmp::Reverse;
use petgraph::graph::{DefaultIx, NodeIndex};
use tap::prelude::*;

pub struct ChunkGraph {
    pub xy_chunks: RoomMatrix<NodeIndex<DefaultIx>>,
    pub graph: Graph<(), u8, Undirected>,
}

pub fn invalid_chunk_node_index() -> NodeIndex<DefaultIx> {
    NodeIndex::end()
}

/// The obstacles matrix should have OBSTACLE_COST on obstacle tiles and 0 on the rest.
pub fn room_chunk_graph(terrain: &RoomMatrix<u8>, chunk_radius: u8) -> ChunkGraph {
    let mut result = ChunkGraph {
        xy_chunks: RoomMatrix::new(invalid_chunk_node_index()),
        graph: Graph::new_undirected(),
    };

    println!("{}", terrain);

    let exits: Vec<RoomXY> = terrain
        .exits()
        .filter_map(|(xy, value)| (value == 0).then_some(xy))
        .collect();

    let exit_distances = distance_matrix(exits.iter().copied(), terrain.find_xy(OBSTACLE_COST));

    let dt = terrain.clone().tap_mut(|t| distance_transform(t));

    let obstacles = terrain.find_xy(OBSTACLE_COST).collect::<Vec<RoomXY>>();
    let tiles = terrain
        .find_xy(0)
        .collect::<Vec<RoomXY>>()
        .tap_mut(|v| v.sort_by_key(|a| Reverse(exit_distances.get(*a))));

    let mut chunk_center_distances = RoomMatrix::new(OBSTACLE_COST);

    for xy in tiles {
        if result.xy_chunks.get(xy) != invalid_chunk_node_index() {
            continue;
        }

        let chunk_center = {
            let mut chunk_center = xy;
            (1..chunk_radius).any(|_| {
                for near in chunk_center.clone().around() {
                    if result.xy_chunks.get(near) == invalid_chunk_node_index() && dt.get(near) < dt.get(chunk_center) {
                        chunk_center = near;
                        return false;
                    }
                }
                for near in chunk_center.clone().around() {
                    if result.xy_chunks.get(near) == invalid_chunk_node_index()
                        && dt.get(near) == dt.get(chunk_center)
                        && exit_distances.get(near) < exit_distances.get(chunk_center)
                    {
                        chunk_center = near;
                        return false;
                    }
                }
                true
            });
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

        let chunk_id = result.graph.add_node(());

        for (xy, chunk_center_distance) in chunk_distance_matrix.iter() {
            if chunk_center_distance < chunk_center_distances.get(xy) {
                chunk_center_distances.set(xy, chunk_center_distance);
                result.xy_chunks.set(xy, chunk_id);
            }
        }
    }

    result
}

#[cfg(test)]
mod test {
    use crate::algorithms::chunk_graph::room_chunk_graph;
    use crate::algorithms::matrix_common::MatrixCommon;
    use crate::algorithms::room_matrix::RoomMatrix;
    use more_asserts::{assert_ge, assert_le};

    #[test]
    fn test_room_chunk_graph() {
        let mut terrain = RoomMatrix::new(0);

        unsafe {
            terrain.set_xy(0, 0, 255u8);
        }

        let chunks = room_chunk_graph(&terrain, 7);
        assert_le!(chunks.graph.node_count(), 100);
        assert_ge!(chunks.graph.node_count(), 50);
    }
}
