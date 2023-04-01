use std::cmp::Reverse;
use petgraph::{Graph, Undirected};
use screeps::RoomXY;
use crate::algorithms::distance_matrix::{distance_matrix, restricted_distance_matrix};
use crate::algorithms::matrix_common::MatrixCommon;
use crate::algorithms::room_matrix::RoomMatrix;
use crate::consts::OBSTACLE_COST;
use tap::prelude::*;
use crate::geometry::rect::ball;
use crate::geometry::room_xy::RoomXYUtils;

pub struct ChunkGraph {
    pub xy_chunks: RoomMatrix<u8>,
    pub graph: Graph<u8, u8, Undirected>,
}

/// The obstacles matrix should have OBSTACLE_COST on obstacle tiles and 0 on the rest.
pub fn room_chunk_graph(terrain: &RoomMatrix<u8>, chunk_radius: u8) -> ChunkGraph {
    let mut result = ChunkGraph {
        xy_chunks: RoomMatrix::new_custom_filled(0),
        graph: Graph::new_undirected(),
    };

    println!("{}", terrain);

    let exits: Vec<RoomXY> = terrain.exits().filter_map(|(xy, value)| (value == 0).then_some(xy)).collect();

    let dist_from_exit = distance_matrix(exits.iter().copied(), terrain.find_xy(OBSTACLE_COST));

    let obstacles = terrain.find_xy(OBSTACLE_COST).collect::<Vec<RoomXY>>();
    let tiles = terrain.find_xy(0).collect::<Vec<RoomXY>>().tap_mut(|v| v.sort_by_key(|a| Reverse(dist_from_exit.get(*a))));

    for xy in tiles {
        if result.xy_chunks.get(xy) != 0 {
            continue;
        }

        // TODO travel towards open area a bit

        let distances_from_xy = restricted_distance_matrix([xy].into_iter(), obstacles.iter().copied(), ball(xy, chunk_radius), chunk_radius);
        distances_from_xy.iter().for_each(|xy| {
            // TODO overwrite distances if closest
        });
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
        let mut terrain = RoomMatrix::new();

        unsafe {
            terrain.set_xy(0, 0, 255u8);
        }

        let chunks = room_chunk_graph(&terrain, 7);
        assert_le!(chunks.graph.node_count(), 100);
        assert_ge!(chunks.graph.node_count(), 50);
    }
}
