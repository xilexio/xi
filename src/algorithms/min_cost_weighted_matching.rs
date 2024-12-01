use std::collections::hash_map::Entry;
use std::fmt::Debug;
use std::ops::{AddAssign, Sub, SubAssign};
use log::debug;
use num_traits::{Bounded, Zero};
use rustc_hash::{FxHashMap, FxHashSet};

const DEBUG: bool = false;

/// Takes a bipartite graph G=(S+T, E), with c(s,t) being a cost of an edge and uses
/// the Hungarian algorithm to find a minimum cost perfect matching.
/// The `costs[i]` is a slice with (j, c(s_i, t_j)), where s_i is i-th S vertex and t_j is j-th
/// T vertex.
/// This implementation is optimized for S vertices not having too many edges (specifically up to
/// N).
///
/// `None` is returned if not all S vertices can be matched to some T vertex.
/// Otherwise, `Some(v)` is returned with |v| = |S| and with `j == v[i]` indicating that the found
/// perfect matching contains an edge between s_i and t_j.
///
/// This implementation is based on the implementation from
/// the Wikipedia[https://en.wikipedia.org/wiki/Hungarian_algorithm]. The main modification is
/// the ability to handle a limited number of vertex edges to speed it up.
///
/// The Hungarian algorithm keeps track of potentials of vertices them and preserves the invariant
/// delta_st = c(s, t) - potential(s) - potential(t) >= 0.
/// When delta_st = 0, the edge is tight and it is a prerequisite for being in the matching.
/// Normally the Hungarian algorithm expects us to find Z, the set of vertices reachable
/// from all unmatched S vertices in the graph of tight edges directed from T except when
/// it is between matched vertices. Then add minimum delta of an edge from Z to outside Z
/// to all potentials in Z * S and remove it form all potentials in Z * T. This increases
/// the number of vertices in Z while not changing the current matching (since one potential
/// in the matching's edge increases and the other decreases by the same amount). When a path
/// is found from an unmatched vertex in S to an unmatched vertex in T, the matchings flip
/// along the way, increasing the number of matched S vertices by one.
/// The faster version of this algorithm introduces S vertices one by one. We start with
/// exactly one unmatched S vertex (s) with potential zero. The nonnegative delta invariant
/// may be violated at the beginning (before the first increase of potentials), but computing
/// Z is easier. The computed Z is a subset of the original one, i.e., only a single T vertex
/// is added to it each time along with its matching vertex (if there is none, the matchings
/// along the way are flipped and the iteration for s ends).
/// This approach has another advantage that if the newly added S vertex has a neighbor
/// unmatched T vertex, the iteration for this S vertex ends with just processing its
/// immediate neighbors.
/// This approach makes progress because it eventually finds a path to an unmatched T vertex
/// since Z keeps increasing until it covers the whole T. This is because edges within Z
/// do not change (as potential is increased and decreased on both sides of the edge by the
/// same amount), but the potential in some Z * S vertex increases enough to make an edge
/// from it to T - Z tight. This approach is also correct because in each iteration of
/// increasing potentials and expanding Z exactly one whole matching gets added.
pub fn min_cost_weighted_matching<W, C>(costs: &[W]) -> Option<(Vec<usize>, C)>
where
    W: AsRef<[(usize, C)]> + Debug,
    C: Zero + Bounded + PartialOrd + Sub<Output = C> + AddAssign + SubAssign + Clone + Copy + Debug,
{
    // Computing |S| and |T|.
    let s_size = costs.len();
    let t_size = costs
        .iter()
        .filter_map(|s_costs| s_costs.as_ref().into_iter().map(|(j, _)| j).max())
        .max()
        .map(|max_j| max_j + 1)
        .unwrap_or(0);
    // matching[j] = i means that s_i is matched to t_j.
    // usize::MAX means no matching yet.
    // Vertices are represented everywhere by their index in the input.
    // Additionally, T has additional vertex used to make the number of edge cases smaller.
    // Specifically, instead of taking into account that the s vertex is unmatched,
    // we add a sentinel T vertex connected and matched just with it.
    let mut matching = vec![usize::MAX; t_size + 1];
    // Potentials of vertices in S and T.
    // We keep track of the potential of the sentinel T vertex because this happens to be the
    // negation of the total cost of the matching.
    // We keep T potential negated as to allow unsigned numbers as cost.
    let mut s_potentials = vec![C::zero(); s_size];
    let mut neg_t_potentials = vec![C::zero(); t_size + 1];
    // We solve the problem iteratively by adding one unmatched S vertex at a time and matching it
    // (potentially changing other matches) until all S vertices are matched.
    // Every previous S vertex starts with a matching and the matching stays constant until the end
    // of the iteration.
    for (unmatched_s, s_costs) in costs.iter().enumerate() {
        // This is the Z set. We only store T vertices since Z * S is equal to all vertices matched
        // with Z * T and the matching is one-to-one until an unmatched T vertex is found (and thus
        // the iteration for s ends). For this reason, we also ignore S vertices in other places,
        // except potentials.
        let mut z = FxHashSet::default();
        // We grow Z one vertex at a time. We focus on last added T vertex, t. In the beginning,
        // it is the artificial T vertex.
        let mut t = t_size;
        // The artificial T vertex is matched with unmatched_s.
        matching[t] = unmatched_s;
        // In each iteration, potentials are increased by the minimum delta outgoing from Z.
        // One of the T vertices with an edge with that minimum delta is the new t.
        // To find it, for each processed T - Z vertex, we store the minimum delta of edges ingoing
        // into it in `min_ingoing` as well as the S vertex from which it came in `previous`.
        // The `previous` map is also used to find the path from `unmatched_s` to unmatched T
        // vertex, should one be found, simply by following `t` backwards using it.
        // The `min_ingoing` does not track vertices after being added to Z for efficiency reasons. 
        let mut min_ingoing: FxHashMap<usize, C> = FxHashMap::default();
        let mut previous: FxHashMap<usize, usize> = FxHashMap::default();

        if DEBUG {
            debug!("Processing s_{} with edges to {:?}.", unmatched_s, s_costs);
            debug!(
                "Current matching T-S: {}.",
                matching
                .iter()
                .enumerate()
                .filter_map(|(j, &i)| (i != usize::MAX).then_some(format!("{}-{}", j, i)))
                .collect::<Vec<_>>()
                .join(", ")
            );
            debug!("Current S potentials: {:?}.", s_potentials);
            debug!("Current T potentials: {:?}.", neg_t_potentials);
        }

        // Continuing to jump over T vertices while growing Z until an unmatched one is
        // found. The condition checks that t is matched.
        while matching[t] != usize::MAX {
            // Adding t to Z. It is guaranteed to not be in it yet.
            z.insert(t);
            min_ingoing.remove(&t);
            // Getting its matching S vertex, s. Initially that is `unmatched_s` and it is
            // guaranteed to exist since the loop condition checks that.
            let s = matching[t];
            if DEBUG {
                debug!("Processing t_{} and its matching s_{}.", t, s);
            }
            for &(t_outside_z, cost) in costs[s].as_ref() {
                // Processing an edge from s to a vertex from T - Z.
                // Cost being the maximum means the edge does not exist.
                if cost != C::max_value() && !z.contains(&t_outside_z) {
                    // Computing the delta for the edge.
                    let edge_delta = cost - s_potentials[s] + neg_t_potentials[t_outside_z];
                    // Updating the minimum delta ingoing into t_outside_z and recording that we
                    // came from S vertex matched with t.
                    match min_ingoing.entry(t_outside_z) {
                        Entry::Occupied(mut e) => {
                            if edge_delta < *e.get() {
                                e.insert(edge_delta);
                                previous.insert(t_outside_z, t);
                            }
                        },
                        Entry::Vacant(e) => {
                            e.insert(edge_delta);
                            previous.insert(t_outside_z, t);
                        }
                    }
                }
            }
            // The next t that is to be from T - Z connected to s.
            let mut next_t = usize::MAX;
            // Computing the minimum delta for all edges between S * Z and T - Z.
            let mut min_delta = C::max_value();
            for (&t_outside_z, &delta) in min_ingoing.iter() {
                // Z does not contain t_outside_z since it was checked upon insertion and
                // would be removed the moment it was added to Z.
                if delta < min_delta {
                    min_delta = delta;
                    next_t = t_outside_z;
                }
            }
            if DEBUG {
                debug!(
                    "Minimum delta and source s_i is {:?} on the edge going to t_{}.",
                    min_ingoing.get(&next_t),
                    next_t
                );
                debug!(
                    "Updating potentials in Z * T = {:?} and Z * S = {:?}.",
                    z,
                    z.iter().map(|&j| matching[j]).collect::<Vec<_>>()
                );
            }
            // Updating the potentials in Z.
            for &t_in_z in z.iter() {
                s_potentials[matching[t_in_z]] += min_delta;
                neg_t_potentials[t_in_z] += min_delta;
            }
            if DEBUG {
                debug!("Current S potentials: {:?}.", s_potentials);
                debug!("Current T potentials: {:?}.", neg_t_potentials);
            }
            // Updating the `min_ingoing` map since the deltas between Z and T - Z have changed.
            for (_, delta) in min_ingoing.iter_mut() {
                *delta -= min_delta;
            }
            // `next_t` being the initial MAX would indicate that there are no edges between Z and
            // T - Z. This means that not all vertices in S can be matched.
            if next_t == usize::MAX {
                return None;
            }
            t = next_t;
        }
        // Updating matching along the alternating path between S and T, starting from the unmatched
        // T vertex, t, and going backwards towards the sentinel (t_size) using the `previous` map.
        // and finishing at the sentinel.
        while t != t_size {
            let prev_t = previous.get(&t).cloned().unwrap();
            if DEBUG {
                debug!(
                    "Changing matching t_{}-s_{} to t_{}-s_{}.",
                    prev_t,
                    matching[prev_t],
                    t,
                    matching[prev_t]
                );
            }
            matching[t] = matching[prev_t];
            t = prev_t;
        }
    }
    // Removing the sentinel from the matching.
    matching.pop();
    let mut rev_matching = vec![usize::MAX; s_size];
    for (t_ix, s_ix) in matching.into_iter().enumerate() {
        // If |T| > |S| then not all T vertices have a matching.
        if s_ix != usize::MAX {
            // All S vertices have a matching because otherwise Z would not have been able to
            // increase at some point and the function would have returned None.
            rev_matching[s_ix] = t_ix;
        }
    }
    Some((rev_matching, C::zero() + neg_t_potentials[t_size]))
}

#[cfg(test)]
mod tests {
    use log::LevelFilter::Trace;
    use crate::algorithms::min_cost_weighted_matching::min_cost_weighted_matching;
    use crate::logging::init_logging;
    use crate::u;
    use crate::utils::permutation::reverse_permutation;

    #[test]
    fn test_one_to_one_matching() {
        init_logging(Trace);
        let costs = [[(0, 42)]];
        let (matching, total_cost) = u!(min_cost_weighted_matching(&costs[..]));
        assert_eq!(matching, vec![0]);
        assert_eq!(total_cost, 42);
    }

    #[test]
    fn test_empty_matching() {
        init_logging(Trace);
        let costs: [[(usize, i32); 1]; 0] = [];
        let (matching, total_cost) = u!(min_cost_weighted_matching(&costs[..]));
        assert!(matching.is_empty());
    }

    #[test]
    fn test_two_to_two_clique_matching() {
        init_logging(Trace);
        let costs = [[(0, 1), (1, 2)], [(0, 2), (1, 1)]];
        let (matching, total_cost) = u!(min_cost_weighted_matching(&costs[..]));
        assert_eq!(matching, vec![0, 1]);
        assert_eq!(total_cost, 2);
    }

    #[test]
    fn test_one_to_two_matching() {
        init_logging(Trace);
        let costs = [[(0, 2), (1, 1)]];
        let (matching, total_cost) = u!(min_cost_weighted_matching(&costs[..]));
        assert_eq!(matching, vec![1]);
        assert_eq!(total_cost, 1);
    }

    #[test]
    fn test_two_to_two_matching_with_flipping() {
        init_logging(Trace);
        let costs = [[(0, 1), (1, 2)], [(0, 1), (1, 3)]];
        let (matching, total_cost) = u!(min_cost_weighted_matching(&costs[..]));
        assert_eq!(matching, vec![1, 0]);
        assert_eq!(total_cost, 3);
    }

    #[test]
    fn test_three_to_three_matching_with_flipping() {
        init_logging(Trace);
        let costs = [[(0, 1), (1, 2), (2, 3)], [(0, 1), (1, 3), (2, 5)], [(0, 1), (1, 7), (2, 9)]];
        let (matching, total_cost) = u!(min_cost_weighted_matching(&costs[..]));
        assert_eq!(matching, vec![2, 1, 0]);
        assert_eq!(total_cost, 7);
    }

    #[test]
    fn test_two_to_three_matching_with_flipping() {
        init_logging(Trace);
        let costs = [[(0, 2), (1, 1)], [(1, 1), (2, 3)]];
        let (matching, total_cost) = u!(min_cost_weighted_matching(&costs[..]));
        assert_eq!(matching, vec![0, 1]);
        assert_eq!(total_cost, 3);
    }

    #[test]
    fn test_two_to_three_matching2() {
        init_logging(Trace);
        // Example from Wikipedia.
        let costs = [[(0, 8), (1, 5), (2, 9)], [(0, 4), (1, 2), (2, 4)], [(0, 7), (1, 3), (2, 8)]];
        let (matching, total_cost) = u!(min_cost_weighted_matching(&costs[..]));
        assert_eq!(matching, vec![0, 2, 1]);
    }

    #[test]
    fn test_two_to_three_matching3() {
        init_logging(Trace);
        let costs = [[(0, 4), (1, 2), (2, 8)], [(0, 3), (1, 2), (2, 5)], [(0, 5), (1, 1), (2, 3)]];
        let (matching, total_cost) = u!(min_cost_weighted_matching(&costs[..]));
        assert_eq!(matching, vec![1, 0, 2]);
    }

    #[test]
    fn test_random_four_to_four_matching() {
        init_logging(Trace);
        let costs = [[(0, 100), (1, 682), (2, 289), (3, 456)], [(0, 239), (1, 211), (2, 768), (3, 180)], [(0, 612), (1, 50), (2, 940), (3, 142)], [(0, 62), (1, 479), (2, 51), (3, 778)]];
        let (matching, total_cost) = u!(min_cost_weighted_matching(&costs[..]));
        assert_eq!(matching, vec![0, 3, 1, 2]);
        assert_eq!(total_cost, 381);
    }

    #[test]
    fn test_random_five_to_five_matching() {
        init_logging(Trace);
        let costs = [[(0, 667), (1, 757), (2, 611), (3, 305), (4, 148)], [(0, 411), (1, 862), (2, 470), (3, 543), (4, 42)], [(0, 976), (1, 379), (2, 571), (3, 255), (4, 790)], [(0, 509), (1, 58), (2, 325), (3, 745), (4, 125)], [(0, 109), (1, 11), (2, 32), (3, 492), (4, 473)]];
        let (matching, total_cost) = u!(min_cost_weighted_matching(&costs[..]));
        assert_eq!(matching, reverse_permutation(&[1, 3, 4, 2, 0]));
    }

    #[test]
    fn test_random_six_to_six_matching() {
        init_logging(Trace);
        let costs = [[(0, 171), (1, 584), (2, 514), (3, 426), (4, 96), (5, 962)], [(0, 733), (1, 902), (2, 69), (3, 48), (4, 831), (5, 648)], [(0, 539), (1, 93), (2, 179), (3, 522), (4, 259), (5, 305)], [(0, 238), (1, 941), (2, 232), (3, 795), (4, 498), (5, 658)], [(0, 749), (1, 329), (2, 215), (3, 970), (4, 234), (5, 400)], [(0, 234), (1, 758), (2, 337), (3, 748), (4, 184), (5, 785)]];
        let (matching, total_cost) = u!(min_cost_weighted_matching(&costs[..]));
        assert_eq!(matching, reverse_permutation(&[5, 2, 3, 1, 0, 4]));
    }
    
    #[test]
    fn test_random_seven_to_seven_matching() {
        init_logging(Trace);
        let costs = [[(0, 256), (1, 179), (2, 841), (3, 433), (4, 203), (5, 740), (6, 82)], [(0, 565), (1, 989), (2, 138), (3, 869), (4, 111), (5, 878), (6, 657)], [(0, 511), (1, 3), (2, 906), (3, 829), (4, 933), (5, 878), (6, 799)], [(0, 746), (1, 12), (2, 660), (3, 596), (4, 668), (5, 981), (6, 711)], [(0, 551), (1, 634), (2, 826), (3, 159), (4, 165), (5, 667), (6, 592)], [(0, 368), (1, 408), (2, 26), (3, 934), (4, 397), (5, 516), (6, 803)], [(0, 860), (1, 394), (2, 813), (3, 371), (4, 398), (5, 719), (6, 200)]];
        let (matching, total_cost) = u!(min_cost_weighted_matching(&costs[..]));
        assert_eq!(matching, reverse_permutation(&[2, 3, 5, 4, 1, 6, 0]));
    }

    #[test]
    fn test_random_eight_to_eight_matching() {
        init_logging(Trace);
        let costs = [[(0, 355), (1, 310), (2, 516), (3, 563), (4, 606), (5, 60), (6, 446), (7, 142)], [(0, 656), (1, 813), (2, 692), (3, 810), (4, 367), (5, 564), (6, 242), (7, 980)], [(0, 51), (1, 663), (2, 624), (3, 664), (4, 428), (5, 306), (6, 12), (7, 496)], [(0, 374), (1, 15), (2, 235), (3, 396), (4, 115), (5, 263), (6, 527), (7, 822)], [(0, 574), (1, 395), (2, 385), (3, 532), (4, 455), (5, 183), (6, 675), (7, 112)], [(0, 997), (1, 719), (2, 274), (3, 716), (4, 283), (5, 868), (6, 697), (7, 686)], [(0, 884), (1, 673), (2, 350), (3, 312), (4, 979), (5, 362), (6, 808), (7, 353)], [(0, 729), (1, 395), (2, 101), (3, 845), (4, 659), (5, 628), (6, 19), (7, 585)]];
        let (matching, total_cost) = u!(min_cost_weighted_matching(&costs[..]));
        assert_eq!(matching, reverse_permutation(&[2, 3, 7, 6, 5, 0, 1, 4]));
    }
}