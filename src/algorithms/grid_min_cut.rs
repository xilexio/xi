use crate::algorithms::matrix_common::MatrixCommon;
use crate::algorithms::room_matrix::RoomMatrix;
use crate::consts::OBSTACLE_COST;
use crate::geometry::room_xy::RoomXYUtils;
use enum_iterator::IntoEnumIterator;
use screeps::{RoomXY, ROOM_SIZE};
use std::fmt::{Display, Formatter};

/// Computes a minimum vertex separator (i.e., min-cut, but for vertices) of a movement graph in
/// a room with source in start and sink in the exits and the tiles (vertices) that surround it.
///
/// Based on Dinitz's algorithm, customized to work on vertices on a grid instead of edges of any
/// graph. Formally, the tiles are two vertices, one input and one output, connected by a directed
/// edge from the input to the output with cost equal to the tile's cost. Outputs of tiles
/// are connected to all surrounding tiles' inputs with an edge of infinite cost.
///
/// The costs matrix represents costs for tiles, 0 for starting tiles or OBSTACLE_COST for
/// obstacles.
pub fn grid_min_cut(costs: &RoomMatrix<u8>) -> Vec<RoomXY> {
    let mut capacity: [u8; GRID_EDGE_ID_CAPACITY as usize] = [0; GRID_EDGE_ID_CAPACITY as usize];
    let mut initial_nodes: Vec<GridGraphNode> = Vec::new();

    let result_rect = Rect::new((2, 2).try_into().unwrap(), (ROOM_SIZE - 3, ROOM_SIZE - 3).try_into().unwrap()).unwrap();

    for y in 1..(ROOM_SIZE - 1) {
        for x in 1..(ROOM_SIZE - 1) {
            let xy = (x, y).try_into().unwrap();
            let raw_tile_cost = costs.get(xy);
            // No edges in or around obstacles or the start are supposed to have any capacity.
            // Exits are supposed to have only their input nodes at the tile next to an exit tile
            // accessible (this is handled later).
            if raw_tile_cost != OBSTACLE_COST && raw_tile_cost != 0 {
                // No internal edge saturation may happen outside of the result_rect.
                let tile_cost = if result_rect.contains(xy) { raw_tile_cost } else { OBSTACLE_COST };
                // Initial capacity of input's non-internal edges is 0.
                // It only has an internal edge with the capacity equal to the tile cost.
                let input_node = grid_node(x, y, Input);
                capacity[grid_edge(input_node, Internal).usize()] = tile_cost;
                let output_node = grid_node(x, y, Output);
                let mut is_near_start = false;
                for_each_node_around(output_node, |near_node, edge| {
                    // Initial capacity of output's internal edge is 0.
                    if !is_internal_edge(edge) {
                        let near = grid_node_to_xy(near_node);
                        let near_tile_cost = costs.get(near);
                        // No capacity to start or obstacle tiles.
                        // However, capacity to exit tiles is normal.
                        if near_tile_cost != OBSTACLE_COST && near_tile_cost != 0 {
                            // Capacity of edges between tiles set to maximum that is higher
                            // than maximum cost.
                            capacity[edge.usize()] = OBSTACLE_COST;
                        } else if near_tile_cost == 0 {
                            // If the output node is next to a start node then its input is
                            // one of starting nodes for the flow.
                            is_near_start = true;
                        }
                    }
                });
                if is_near_start {
                    initial_nodes.push(input_node);
                }
            }
        }
    }

    #[cfg(feature = "debug_grid_min_cut")]
    {
        eprintln!(
            "Initial nodes: {:?}.",
            initial_nodes.iter().map(|node| format!("{}", *node)).collect::<Vec<String>>().join(", ")
        );
        eprintln!();
    }

    loop {
        // Computing BFS distances from the initial flow nodes. The BFS ends when a node on a tile
        // next to an exit tile is reached. This is sufficient to only traverse the shortest paths.
        // The BFS only goes through not saturated edges.

        let mut bfs_distances = [OBSTACLE_COST; GRID_NODE_ID_CAPACITY as usize];
        let mut layer = initial_nodes.clone();
        let mut distance = 0u8;
        let mut exit_reached = false;

        while !layer.is_empty() && distance < OBSTACLE_COST - 1 {
            let mut next_layer = Vec::new();

            for node in layer {
                bfs_distances[node.usize()] = distance;
                for_each_node_around(node, |near_node, edge| {
                    let near = grid_node_to_xy(near_node);
                    if bfs_distances[near_node.usize()] == OBSTACLE_COST && capacity[edge.usize()] > 0 {
                        bfs_distances[near_node.usize()] = distance + 1;
                        if near.exit_distance() == 0 {
                            exit_reached = true;
                        } else {
                            next_layer.push(near_node);
                        }
                    }
                });
            }

            distance += 1;
            layer = next_layer;
        }

        #[cfg(feature = "debug_grid_min_cut")]
        {
            for y in 0..ROOM_SIZE {
                for x in 0..ROOM_SIZE {
                    let dist = bfs_distances[grid_node(x, y, Input).usize()];
                    if dist == OBSTACLE_COST {
                        eprint!(" X ");
                    } else {
                        eprint!("{:2} ", dist);
                    }
                }
                eprintln!();
            }
            eprintln!();
        }

        if !exit_reached {
            break;
        }

        // We start finding blocking flow from the nodes that are next to start tiles.
        // We have a BFS that will be used to restrict ourselves to only the shortest paths.
        // We travel only to nodes with strictly smaller BFS distances from exits.
        // We repeatedly perform DFS with backtracking and removing vertices when it cannot move
        // towards exits as a result of saturated capacities.

        let mut dfs_stack: Vec<(GridGraphNode, GridGraphEdge)> =
            initial_nodes.iter().map(|node| (*node, UNKNOWN_EDGE)).collect();
        let mut path = Vec::new();
        while !dfs_stack.is_empty() {
            let node = dfs_stack[dfs_stack.len() - 1].0;
            path.push(dfs_stack[dfs_stack.len() - 1]);
            let xy = grid_node_to_xy(node);
            if xy.exit_distance() == 0 {
                #[cfg(feature = "debug_grid_min_cut")]
                eprintln!(
                    "Found exit with path {:?}.",
                    path.iter().map(|(node, _)| format!("{}", *node)).collect::<Vec<String>>().join(", ")
                );

                // We reached the exit - adding the flow through the edges we have followed.
                let mut flow = 255;
                // We skip the first, unknown edge.
                for i in 1..path.len() {
                    let travelled_edge = path[i].1;
                    if capacity[travelled_edge.usize()] < flow {
                        flow = capacity[travelled_edge.usize()];
                    }
                }
                debug_assert!(flow > 0);
                let mut still_valid_path_length = 256;
                for i in 1..path.len() {
                    let travelled_edge = path[i].1;
                    capacity[travelled_edge.usize()] -= flow;
                    capacity[reverse_edge(travelled_edge).usize()] += flow;
                    if still_valid_path_length == 256 && capacity[travelled_edge.usize()] == 0 {
                        still_valid_path_length = i;
                    }
                }
                debug_assert!(still_valid_path_length != 256);
                // We reuse the dfs_stack.
                while path.len() > still_valid_path_length {
                    path.pop();
                    while dfs_stack[dfs_stack.len() - 1].0 != path[path.len() - 1].0 {
                        dfs_stack.pop();
                    }
                }
                path.pop();

                #[cfg(feature = "debug_grid_min_cut")]
                {
                    eprintln!("Flow was: {}.", flow);
                    eprintln!("Still valid path length: {}.", still_valid_path_length);
                    eprintln!(
                        "Path after backtracking: {:?}.",
                        path.iter().map(|(node, _)| format!("{}", *node)).collect::<Vec<String>>().join(", ")
                    );
                }
            } else {
                let mut dead_end = true;
                for_each_node_around(node, |near_node, edge| {
                    if capacity[edge.usize()] > 0 && bfs_distances[near_node.usize()] == (path.len() as u8) {
                        dead_end = false;
                        dfs_stack.push((near_node, edge));

                        debug_assert!(grid_node_to_xy(near_node).dist(grid_node_to_xy(node)) <= 1);
                    }
                });
                if dead_end {
                    #[cfg(feature = "debug_grid_min_cut")]
                    {
                        eprintln!(
                            "Dead end at {}.",
                            path.iter().map(|(node, _)| format!("{}", *node)).collect::<Vec<String>>().join(", ")
                        );
                        for_each_node_around(node, |near_node, edge| {
                            eprintln!(
                                "  Near node {}: cap {}, bfs dist {}, path len {}",
                                near_node,
                                capacity[edge.usize()],
                                bfs_distances[near_node.usize()],
                                path.len() as u8
                            );
                        });
                    }

                    while !path.is_empty() && dfs_stack.last().unwrap().0 == path.last().unwrap().0 {
                        // Two same nodes being at the end of path and DFS stack mean that the other
                        // children were already processed and it is a dead end also.
                        // Setting the BFS distance of a node to 0 means that no other node may
                        // traverse to it as a result of the strictly increasing distance rule.
                        bfs_distances[path.last().unwrap().0.usize()] = 0;
                        dfs_stack.pop();
                        path.pop();
                    }

                    #[cfg(feature = "debug_grid_min_cut")]
                    if !dfs_stack.is_empty() {
                        eprintln!("Resuming from {}.", dfs_stack.last().unwrap().0);
                    }
                }
            }
        }

        #[cfg(feature = "debug_grid_min_cut")]
        {
            for y in 15..37 {
                for x in 15..49 {
                    let residual_cap = capacity[grid_node(x, y, Input).usize()];
                    let cap = unsafe { costs.get_xy(x, y) };
                    eprint!("{}/{} ", residual_cap, cap);
                }
                eprintln!();
            }
            eprintln!();
        }
    }

    #[cfg(feature = "debug_grid_min_cut")]
    {
        for y in 15..37 {
            for x in 15..49 {
                let residual_cap = capacity[grid_node(x, y, Input).usize()];
                let cap = unsafe { costs.get_xy(x, y) };
                eprint!("{}/{} ", residual_cap, cap);
            }
            eprintln!();
        }
        eprintln!();
    }

    // We get the min-cut tiles by running the BFS from the start and selecting first tiles with
    // saturated internal edges.

    let mut layer = initial_nodes.clone();
    let mut bfs_visited = [false; GRID_NODE_ID_CAPACITY as usize];

    while !layer.is_empty() {
        let mut next_layer = Vec::new();

        for node in layer {
            bfs_visited[node.usize()] = true;
            // eprintln!("Visiting xy={} output={}.", grid_node_id_to_xy(node), node & 1);
            for_each_node_around(node, |near_node, edge| {
                // eprintln!("Near xy={} output={} with capacity={} visited={}.", grid_node_id_to_xy(near_node), near_node & 1, capacity[edge as usize], bfs_visited[near_node.usize()]);
                if !bfs_visited[near_node.usize()] && capacity[edge.usize()] > 0 {
                    next_layer.push(near_node);
                    bfs_visited[near_node.usize()] = true;
                }
            });
        }

        layer = next_layer;
    }

    let mut result = Vec::new();
    for y in 2..(ROOM_SIZE - 2) {
        for x in 2..(ROOM_SIZE - 2) {
            let input_node = grid_node(x, y, Input);
            let output_node = grid_node(x, y, Output);
            if bfs_visited[input_node.usize()] && !bfs_visited[output_node.usize()] {
                result.push(grid_node_to_xy(input_node));
            }
        }
    }

    // for y in 0..ROOM_SIZE {
    //     let mut line = "".to_string();
    //     for x in 0..ROOM_SIZE {
    //         let cost = unsafe { costs.get_xy(x, y) };
    //         if cost == OBSTACLE_COST {
    //             line += " # ";
    //         } else if cost == 0 {
    //             line += " S ";
    //         } else {
    //             let input_node = grid_node(x, y, Input);
    //             let output_node = grid_node(x, y, Output);
    //             line += ["F", "T"][bfs_visited[input_node.usize()] as usize];
    //             line += ["F", "T"][bfs_visited[output_node.usize()] as usize];
    //             line += " ";
    //         }
    //     }
    //     debug!("{}", line);
    // }

    #[cfg(feature = "debug_grid_min_cut")]
    {
        for y in 0..ROOM_SIZE {
            for x in 0..ROOM_SIZE {
                let cost = unsafe { costs.get_xy(x, y) };
                if cost == OBSTACLE_COST {
                    eprint!(" # ");
                } else if cost == 0 {
                    eprint!(" S ");
                } else {
                    let input_node = grid_node(x, y, Input);
                    let output_node = grid_node(x, y, Output);
                    eprint!(
                        "{}{} ",
                        ["F", "T"][bfs_visited[input_node.usize()] as usize],
                        ["F", "T"][bfs_visited[output_node.usize()] as usize]
                    );
                }
            }
            eprintln!();
        }
    }

    result
}

#[derive(Debug, Copy, Clone, Eq, PartialEq)]
struct GridGraphEdge(u16);

impl Display for GridGraphEdge {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        let node = edge_node(*self);
        let direction = edge_direction(*self);
        write!(f, "{}{}", node, direction as u8)
    }
}

impl GridGraphEdge {
    fn usize(self) -> usize {
        self.0 as usize
    }
}

/// Edge IDs are grid node IDs plus the direction constant times GRID_NODE_ID_CAPACITY.
/// The maximum value is 6372 * 9 = 57348, which fits in u16.
/// The multiplication is slow, but edges are always iterated over, never computed directly.
#[inline]
fn grid_edge(node: GridGraphNode, direction: GridGraphDirection) -> GridGraphEdge {
    GridGraphEdge(node.0 + GRID_NODE_ID_CAPACITY * (direction as u16))
}

#[inline]
fn edge_node(edge: GridGraphEdge) -> GridGraphNode {
    GridGraphNode(edge.0 % GRID_NODE_ID_CAPACITY)
}

#[inline]
fn edge_direction(edge: GridGraphEdge) -> GridGraphDirection {
    ((edge.0 / GRID_NODE_ID_CAPACITY) as u8).into()
}

#[inline]
fn edge_target_node(edge: GridGraphEdge) -> GridGraphNode {
    let direction = edge_direction(edge);
    let source_node = edge_node(edge);
    let (x, y) = direction_to_offset(direction);
    GridGraphNode((((source_node.0 ^ 1) as i16) + x * (1 << 1) + y * (1 << 7)) as u16)
}

#[inline]
fn reverse_edge(edge: GridGraphEdge) -> GridGraphEdge {
    let direction = edge_direction(edge);
    let target_node = edge_target_node(edge);
    grid_edge(target_node, reverse_direction(direction))
}

#[inline]
fn is_internal_edge(edge: GridGraphEdge) -> bool {
    edge.0 < GRID_NODE_ID_CAPACITY
}

const GRID_EDGE_ID_CAPACITY: u16 = GRID_NODE_ID_CAPACITY * 9;
const UNKNOWN_EDGE: GridGraphEdge = GridGraphEdge(GRID_EDGE_ID_CAPACITY - 1);

#[derive(Debug, Copy, Clone, Eq, PartialEq)]
struct GridGraphNode(u16);

impl Display for GridGraphNode {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        let xy = grid_node_to_xy(*self);
        let output_str = ["0", "I"][is_output_node(*self) as usize];
        write!(f, "{}{}", xy, output_str)
    }
}

impl GridGraphNode {
    fn usize(self) -> usize {
        self.0 as usize
    }
}

/// Grid node IDs (from least significant bits):
/// - 1 bit for Input / Output
/// - 6 bits for the X axis coordinate
/// - 6 bits for the Y axis coordinate
/// Note that some grid node ID space is wasted since ROOM_SIZE < 64. But log2 5000 > 12.
/// The maximum value is 6371 < 8192, leaving some free space that will be used in edge IDs.
fn grid_node(x: u8, y: u8, kind: TileVertexKind) -> GridGraphNode {
    GridGraphNode((kind as u16) | ((x as u16) << 1) | ((y as u16) << 7))
}

fn is_output_node(node: GridGraphNode) -> bool {
    (node.0 & 1) == 1
}

fn grid_node_to_xy(node: GridGraphNode) -> RoomXY {
    // Safe as long as the grid node ID is correct.
    unsafe { RoomXY::unchecked_new(((node.0 >> 1) & ((1 << 6) - 1)) as u8, (node.0 >> 7) as u8) }
}

const GRID_NODE_ID_CAPACITY: u16 = 1 + (1 | (((ROOM_SIZE - 1) as u16) << 1) | (((ROOM_SIZE - 1) as u16) << 7));

/// Invokes function f on each edge (normal and backflow) coming from vertex with given `id`.
/// Must not be called on an exit tile or else an integer overflow is possible.
#[inline]
fn for_each_node_around<F, R>(node: GridGraphNode, mut f: F)
where
    F: FnMut(GridGraphNode, GridGraphEdge) -> R,
{
    let mut edge = node.0;

    for direction in GridGraphDirection::into_enum_iter() {
        let (x, y) = direction_to_offset(direction);
        f(GridGraphNode((((node.0 ^ 1) as i16) + x * (1 << 1) + y * (1 << 7)) as u16), GridGraphEdge(edge));
        edge += GRID_NODE_ID_CAPACITY;
    }
}

enum TileVertexKind {
    Input = 0,
    Output = 1,
}
use TileVertexKind::*;

#[derive(Debug, IntoEnumIterator, Clone, Copy, Eq, PartialEq)]
#[repr(u8)]
enum GridGraphDirection {
    Internal = 0,
    Top = 1,
    TopRight = 2,
    Right = 3,
    BottomRight = 4,
    Bottom = 5,
    BottomLeft = 6,
    Left = 7,
    TopLeft = 8,
}
use crate::geometry::rect::Rect;
use GridGraphDirection::*;

impl From<u8> for GridGraphDirection {
    fn from(value: u8) -> Self {
        match value {
            0 => Internal,
            1 => Top,
            2 => TopRight,
            3 => Right,
            4 => BottomRight,
            5 => Bottom,
            6 => BottomLeft,
            7 => Left,
            8 => TopLeft,
            _ => unreachable!(),
        }
    }
}

#[inline]
fn reverse_direction(direction: GridGraphDirection) -> GridGraphDirection {
    if direction == Internal {
        direction
    } else {
        ((direction as u8 - 1 + 4) % 8 + 1).into()
    }
}

#[inline]
fn direction_to_offset(direction: GridGraphDirection) -> (i16, i16) {
    match direction {
        Internal => (0, 0),
        Top => (0, -1),
        TopRight => (1, -1),
        Right => (1, 0),
        BottomRight => (1, 1),
        Bottom => (0, 1),
        BottomLeft => (-1, 1),
        Left => (-1, 0),
        TopLeft => (-1, -1),
    }
}

#[cfg(test)]
mod tests {
    use crate::algorithms::grid_min_cut::GridGraphDirection::{BottomRight, Internal};
    use crate::algorithms::grid_min_cut::TileVertexKind::{Input, Output};
    use crate::algorithms::grid_min_cut::{
        edge_direction, edge_node, edge_target_node, for_each_node_around, grid_edge, grid_min_cut, grid_node,
        grid_node_to_xy, is_internal_edge, reverse_edge, GridGraphDirection,
    };
    use crate::algorithms::matrix_common::MatrixCommon;
    use crate::algorithms::room_matrix::RoomMatrix;
    use crate::consts::OBSTACLE_COST;
    use crate::geometry::rect::Rect;
    use crate::geometry::room_xy::RoomXYUtils;
    use enum_iterator::IntoEnumIterator;
    use screeps::{RoomXY, ROOM_SIZE};
    use std::error::Error;

    #[test]
    fn test_helper_functions() {
        let node_xy = RoomXY::try_from((12, 12)).unwrap();
        let input_node = grid_node(12, 12, Input);

        let input_to_output_edge = grid_edge(input_node, Internal);
        assert_eq!(edge_node(input_to_output_edge), input_node);
        assert_eq!(edge_direction(input_to_output_edge), Internal);
        assert_eq!(reverse_edge(reverse_edge(input_to_output_edge)), input_to_output_edge);

        let output_node = grid_node(12, 12, Output);

        for direction in GridGraphDirection::into_enum_iter() {
            let output_to_something_edge = grid_edge(output_node, direction);
            assert_eq!(reverse_edge(reverse_edge(output_to_something_edge)), output_to_something_edge);
        }

        let target_node_xy = grid_node_to_xy(edge_node(reverse_edge(grid_edge(output_node, BottomRight))));
        assert_eq!(target_node_xy, RoomXY::try_from((13, 13)).unwrap());

        for_each_node_around(input_node, |near_node, edge| {
            assert_eq!(edge_node(edge), input_node);
            if edge_direction(edge) == Internal {
                assert_eq!(edge_target_node(edge), output_node);
            } else {
                let xy = grid_node_to_xy(edge_target_node(edge));
                assert_eq!(xy.dist(node_xy), 1);
            }
        });

        assert!(is_internal_edge(grid_edge(grid_node(25, 26, Input), Internal)));
        assert!(!is_internal_edge(grid_edge(grid_node(25, 26, Input), BottomRight)));
    }

    #[test]
    fn test_grid_min_cut_on_empty_room() {
        let mut costs = RoomMatrix::new(1);
        unsafe {
            costs.set_xy(25, 25, 0);
        }
        let min_cut = grid_min_cut(&costs);
        assert_eq!(min_cut.len(), 8);
    }

    #[test]
    fn test_grid_min_cut_on_empty_room_and_multiple_points() {
        let mut costs = RoomMatrix::new(1);
        unsafe {
            costs.set_xy(24, 24, 0);
            costs.set_xy(25, 24, 0);
            costs.set_xy(26, 24, 0);
            costs.set_xy(24, 25, 0);
            costs.set_xy(26, 25, 0);
            costs.set_xy(24, 26, 0);
            costs.set_xy(25, 26, 0);
            costs.set_xy(26, 26, 0);
            costs.set_xy(25, 27, 0);
        }
        let min_cut = grid_min_cut(&costs);
        assert_eq!(min_cut.len(), 18);
    }

    #[test]
    fn test_grid_min_cut_on_room_with_obstacles() {
        let mut costs = RoomMatrix::new(1);
        unsafe {
            // # # . # #
            // # . S . #
            // . . S . #
            // # . # . #
            costs.set_xy(25, 25, 0);
            costs.set_xy(25, 26, 0);
            costs.set_xy(23, 24, OBSTACLE_COST);
            costs.set_xy(24, 24, OBSTACLE_COST);
            costs.set_xy(26, 24, OBSTACLE_COST);
            costs.set_xy(27, 24, OBSTACLE_COST);
            costs.set_xy(23, 25, OBSTACLE_COST);
            costs.set_xy(27, 25, OBSTACLE_COST);
            costs.set_xy(27, 26, OBSTACLE_COST);
            costs.set_xy(23, 27, OBSTACLE_COST);
            costs.set_xy(25, 27, OBSTACLE_COST);
            costs.set_xy(27, 27, OBSTACLE_COST);
        }
        let min_cut = grid_min_cut(&costs);
        assert_eq!(min_cut.len(), 4);
    }

    #[test]
    fn test_grid_min_cut_on_room_with_more_obstacles() -> Result<(), Box<dyn Error>> {
        let mut costs = RoomMatrix::new(1);
        for xy in Rect::new((15, 15).try_into()?, (40, 40).try_into()?)?.iter() {
            costs.set(xy, 0);
        }
        for xy in Rect::new((0, 0).try_into()?, (ROOM_SIZE - 1, 10).try_into()?)?.iter() {
            costs.set(xy, OBSTACLE_COST);
        }
        for xy in Rect::new((0, 11).try_into()?, (10, ROOM_SIZE - 1).try_into()?)?.iter() {
            costs.set(xy, OBSTACLE_COST);
        }
        let min_cut = grid_min_cut(&costs);
        assert_eq!(min_cut.len(), 61);
        Ok(())
    }

    #[test]
    fn test_grid_min_cut_on_room_with_three_thin_walls() -> Result<(), Box<dyn Error>> {
        let mut costs = RoomMatrix::new(1);
        for xy in Rect::new((0, 0).try_into()?, (ROOM_SIZE - 1, 0).try_into()?)?.iter() {
            costs.set(xy, OBSTACLE_COST);
        }
        for xy in Rect::new((0, 1).try_into()?, (0, ROOM_SIZE - 1).try_into()?)?.iter() {
            costs.set(xy, OBSTACLE_COST);
        }
        for xy in Rect::new((ROOM_SIZE - 1, 1).try_into()?, (ROOM_SIZE - 1, ROOM_SIZE - 1).try_into()?)?.iter() {
            costs.set(xy, OBSTACLE_COST);
        }
        costs.set((1, 45).try_into()?, OBSTACLE_COST);
        costs.set((ROOM_SIZE - 2, 45).try_into()?, OBSTACLE_COST);
        for xy in Rect::new((10, 5).try_into()?, (40, 5).try_into()?)?.iter() {
            costs.set(xy, 0);
        }
        let min_cut = grid_min_cut(&costs);
        assert_eq!(min_cut.len(), 46);
        for &xy in min_cut.iter() {
            assert_eq!(xy.y.u8(), 45);
        }
        Ok(())
    }
}
