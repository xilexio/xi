use crate::algorithms::distance_matrix::{distance_matrix, rect_restricted_distance_matrix};
use crate::algorithms::distance_transform::distance_transform_from_obstacles;
use crate::algorithms::matrix_common::MatrixCommon;
use crate::algorithms::room_matrix::RoomMatrix;
use crate::algorithms::vertex_cut::vertex_cut;
use crate::consts::OBSTACLE_COST;
use crate::geometry::rect::{ball, room_rect};
use crate::geometry::room_xy::RoomXYUtils;
use petgraph::prelude::EdgeRef;
use petgraph::stable_graph::{NodeIndex, StableGraph};
use petgraph::Undirected;
use rustc_hash::{FxHashMap, FxHashSet};
use screeps::RoomXY;
use std::cmp::Reverse;
use std::iter::once;
use tap::prelude::*;

pub type ChunkId = NodeIndex<u16>;

pub struct ChunkGraph {
    /// The assignment tiles -> chunks.
    pub xy_chunks: RoomMatrix<ChunkId>,
    /// Sizes of chunks.
    pub chunk_sizes: FxHashMap<ChunkId, u16>,
    /// A graph of chunks with edges between neighboring chunks.
    /// Nodes are labelled by chunk centers. Weights of edges are the distance between chunk
    /// centers.
    pub graph: StableGraph<RoomXY, u8, Undirected, u16>,
}

impl ChunkGraph {
    /// Returns vector with all chunks containing an exit tile.
    pub fn exit_chunks(&self) -> FxHashSet<ChunkId> {
        let mut result = FxHashSet::default();
        for xy in room_rect().boundary() {
            let chunk = self.xy_chunks.get(xy);
            if chunk != invalid_chunk_node_index() {
                result.insert(chunk);
            }
        }
        result
    }

    /// Returns cut vertices, i.e., all chunks whose removal will split the graph in two.
    pub fn hard_chokepoints(&self) -> FxHashSet<ChunkId> {
        let mut chokepoints = vertex_cut(&self.graph);
        for node in self.exit_chunks().into_iter() {
            chokepoints.remove(&node);
        }
        chokepoints
    }

    /// Returns map with all enclosed chunks mapped to the outermost chokepoint chunks that block access to them
    /// and information whether this is a chokepoint (possibly internal) itself.
    pub fn enclosures(&self) -> FxHashMap<ChunkId, (ChunkId, bool)> {
        let chokepoints = self.hard_chokepoints();

        // Visited in the graph where we check non-enclosed nodes.
        let mut not_enclosed = self.exit_chunks();
        {
            let mut path = not_enclosed.iter().copied().collect::<Vec<_>>();

            while let Some(node) = path.pop() {
                for edge in self.graph.edges(node) {
                    if !not_enclosed.contains(&edge.target()) && !chokepoints.contains(&edge.target()) {
                        path.push(edge.target());
                        not_enclosed.insert(edge.target());
                    }
                }
            }
        }

        let mut result = FxHashMap::default();
        {
            // We only take into consideration outer chokepoints, not inner ones.
            let mut path = chokepoints
                .iter()
                .copied()
                .filter(|&chokepoint| {
                    self.graph
                        .edges(chokepoint)
                        .any(|edge| not_enclosed.contains(&edge.target()))
                })
                .map(|node| (node, node))
                .collect::<Vec<_>>();
            for (node, _) in path.iter().copied() {
                result.insert(node, (node, true));
            }
            while let Some((node, chokepoint)) = path.pop() {
                for edge in self.graph.edges(node) {
                    if !result.contains_key(&edge.target()) && !not_enclosed.contains(&edge.target()) {
                        result.insert(edge.target(), (chokepoint, chokepoints.contains(&edge.target())));
                        path.push((edge.target(), chokepoint));
                    }
                }
            }
        }

        result
    }
}

pub fn invalid_chunk_node_index() -> ChunkId {
    NodeIndex::end()
}

/// The obstacles matrix should have OBSTACLE_COST on obstacle tiles and 0 on the rest.
// TODO remove terrain in favor of obstacles iterator.
pub fn chunk_graph(terrain: &RoomMatrix<u8>, chunk_radius: u8) -> ChunkGraph {
    let exits: Vec<RoomXY> = terrain
        .boundary()
        .filter_map(|(xy, value)| (value == 0).then_some(xy))
        .collect();

    let exit_distances = distance_matrix(terrain.find_xy(OBSTACLE_COST), exits.iter().copied());

    let dt = distance_transform_from_obstacles(terrain.find_xy(OBSTACLE_COST), 1);

    let tiles = terrain
        .find_xy(0)
        .collect::<Vec<RoomXY>>()
        .tap_mut(|v| v.sort_by_key(|a| Reverse(exit_distances.get(*a))));

    let mut chunk_center_distances = RoomMatrix::new(255);

    let mut xy_chunks = RoomMatrix::new(invalid_chunk_node_index());
    let mut chunk_sizes = FxHashMap::default();
    let approximate_number_of_nodes = 2500 / (4 * (chunk_radius as usize) * (chunk_radius as usize));
    let mut graph = StableGraph::with_capacity(approximate_number_of_nodes, approximate_number_of_nodes * 2);

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

        let chunk_distance_matrix = rect_restricted_distance_matrix(
            chunk_ball.iter().filter(|xy| terrain.get(*xy) == OBSTACLE_COST),
            once(chunk_center),
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

    let minimum_chunk_size = min_chunk_size(chunk_radius);

    // We remove all chunks that are smaller than min_chunk_size and save all remaining chunks'
    // centers.
    let mut layer = Vec::new();
    chunk_sizes.retain(|&chunk_id, &mut size| {
        if size >= minimum_chunk_size {
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
                    } else if near_chunk_id != chunk_id {
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

#[inline]
pub fn min_chunk_size(chunk_radius: u8) -> u16 {
    (chunk_radius as u16) * (chunk_radius as u16)
}

#[cfg(test)]
mod tests {
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
