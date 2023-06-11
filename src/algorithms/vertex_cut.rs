use std::collections::hash_map::Entry::Vacant;
use petgraph::prelude::EdgeRef;
use petgraph::stable_graph::{IndexType, NodeIndex, StableGraph};
use petgraph::Undirected;
use rustc_hash::{FxHashMap, FxHashSet};
use crate::u;

/// Returns indexes of nodes that are articulation vertices, i.e., removing them would split the graph in two connected
/// components.
pub fn vertex_cut<N, E, I>(graph: &StableGraph<N, E, Undirected, I>) -> FxHashSet<NodeIndex<I>>
where
    I: IndexType,
{
    let mut cut = FxHashSet::default();
    let root = u!(graph.node_indices().next());
    // Current DFS path. All pre-visited nodes are still on the path.
    let mut dfs_stack = vec![root];
    // All pre-visited or visited nodes have depth set.
    let mut depths = FxHashMap::default();
    let mut parents = FxHashMap::default();
    let mut lowest_discovery_depth = FxHashMap::default();
    let mut root_dfs_children_count = 0;

    while let Some(&node) = dfs_stack.last() {
        if let Vacant(entry) = depths.entry(node) {
            // Visiting the node for the first time.
            let depth = lowest_discovery_depth.len();
            lowest_discovery_depth.insert(node, depth);
            entry.insert(depth);

            let parent = parents.get(&node).copied();
            for edge in graph.edges(node) {
                if let Some(&target_depth) = depths.get(&edge.target()) {
                    // The node was already pre-visited. Updating discovery depth if needed.
                    if Some(edge.target()) != parent && target_depth < *u!(lowest_discovery_depth.get(&node)) {
                        lowest_discovery_depth.insert(node, target_depth);
                    }
                } else {
                    // The node was never pre-visited. Adding it on the stack and setting its parent.
                    // Note that its parent may be later overwritten.
                    dfs_stack.push(edge.target());
                    parents.insert(edge.target(), node);
                }
            }
        } else {
            // The node was already visited, so it is time to run its parent's after-child-visit code.
            // This code should be ran just once per node and the information about parent will not be needed later, so
            // we mark it by removing its parent information.
            if let Some(parent) = parents.remove(&node) {
                let depth = *u!(depths.get(&node));
                let low = *u!(lowest_discovery_depth.get(&node));
                let parent_low = *u!(lowest_discovery_depth.get(&parent));
                if low < parent_low {
                    lowest_discovery_depth.insert(parent, low);
                }
                if parent != root {
                    let parent_depth = *u!(depths.get(&parent));
                    if parent_depth <= low {
                        cut.insert(parent);
                    }
                } else {
                    root_dfs_children_count += 1;
                }
            }
            dfs_stack.pop();
        }
    }

    if root_dfs_children_count > 1 {
        cut.insert(root);
    }

    cut
}

#[cfg(test)]
mod tests {
    use crate::algorithms::vertex_cut::vertex_cut;
    use petgraph::stable_graph::{NodeIndex, StableGraph};

    #[test]
    fn test_vertex_cut1() {
        let mut graph = StableGraph::default();
        let n = (0..3).map(|_| graph.add_node(())).collect::<Vec<NodeIndex<u8>>>();
        graph.add_edge(n[0], n[1], ());
        graph.add_edge(n[1], n[2], ());

        assert_eq!(vertex_cut(&graph), [n[1]].into_iter().collect());
    }

    #[test]
    fn test_vertex_cut2() {
        let mut graph = StableGraph::default();
        let n = (0..3).map(|_| graph.add_node(())).collect::<Vec<NodeIndex<u8>>>();
        graph.add_edge(n[0], n[1], ());
        graph.add_edge(n[0], n[2], ());

        assert_eq!(vertex_cut(&graph), [n[0]].into_iter().collect());
    }

    #[test]
    fn test_vertex_cut3() {
        let mut graph = StableGraph::default();
        let n = (0..5).map(|_| graph.add_node(())).collect::<Vec<NodeIndex<u8>>>();
        graph.add_edge(n[0], n[1], ());
        graph.add_edge(n[0], n[2], ());
        graph.add_edge(n[2], n[3], ());
        graph.add_edge(n[2], n[4], ());
        graph.add_edge(n[3], n[4], ());

        assert_eq!(vertex_cut(&graph), [n[0], n[2]].into_iter().collect());
    }

    #[test]
    fn test_vertex_cut4() {
        let mut graph = StableGraph::default();
        let n = (0..6).map(|_| graph.add_node(())).collect::<Vec<NodeIndex<u8>>>();
        graph.add_edge(n[0], n[1], ());
        graph.add_edge(n[0], n[2], ());
        graph.add_edge(n[1], n[3], ());
        graph.add_edge(n[2], n[4], ());
        graph.add_edge(n[2], n[5], ());
        graph.add_edge(n[3], n[2], ());
        graph.add_edge(n[4], n[5], ());

        assert_eq!(vertex_cut(&graph), [n[2]].into_iter().collect());
    }
}
