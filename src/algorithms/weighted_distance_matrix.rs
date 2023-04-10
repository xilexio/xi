use crate::algorithms::matrix_common::MatrixCommon;
use crate::algorithms::room_matrix::RoomMatrix;
use crate::geometry::room_xy::RoomXYUtils;
use num_traits::PrimInt;
use screeps::RoomXY;
use std::collections::BTreeMap;

// TODO is there any practical reason to differentiate between obstacles and unreachable?
pub fn obstacle_cost<T>() -> T
where
    T: PrimInt,
{
    T::max_value()
}

pub fn unreachable_cost<T>() -> T
where
    T: PrimInt,
{
    T::max_value() - T::one()
}

/// Implementation of Dijkstra algorithm from multiple starting points.
pub fn weighted_distance_matrix<T>(cost_matrix: &RoomMatrix<T>, start: impl Iterator<Item = RoomXY>) -> RoomMatrix<T>
where
    T: PrimInt,
{
    let mut distances = RoomMatrix::new(unreachable_cost());
    let mut queue: BTreeMap<T, Vec<RoomXY>> = BTreeMap::new();

    for xy in start {
        distances.set(xy, T::zero());
        for near in xy.around() {
            let cost = cost_matrix.get(near);
            if cost != obstacle_cost() {
                distances.set(near, cost);
                queue.entry(cost).and_modify(|xys| xys.push(near)).or_insert(vec![near]);
            }
        }
    }

    while !queue.is_empty() {
        let mut first = queue.first_entry().unwrap();
        let xys = first.get_mut();
        if let Some(xy) = xys.pop() {
            let dist = *first.key();
            if distances.get(xy) == dist {
                for near in xy.around() {
                    let near_cost = cost_matrix.get(near);
                    let new_dist = dist.saturating_add(near_cost);
                    let near_dist = distances.get(near);
                    if near_cost != obstacle_cost() && new_dist < near_dist {
                        distances.set(near, new_dist);
                        queue
                            .entry(new_dist)
                            .and_modify(|xys| xys.push(near))
                            .or_insert(vec![near]);
                    }
                }
            }
        } else {
            first.remove();
        }
    }

    distances
}

#[cfg(test)]
mod tests {
    use crate::algorithms::matrix_common::MatrixCommon;
    use crate::algorithms::room_matrix::RoomMatrix;
    use crate::algorithms::weighted_distance_matrix::weighted_distance_matrix;
    use screeps::ROOM_SIZE;

    #[test]
    fn test_weighted_distance_matrix() {
        let mut cost_matrix = RoomMatrix::new(8);
        for y in 0..ROOM_SIZE {
            for x in 0..ROOM_SIZE {
                if x % 2 == y % 2 {
                    cost_matrix.set((x, y).try_into().unwrap(), 6);
                }
            }
        }
        let distances = weighted_distance_matrix(
            &cost_matrix,
            [(0, 0).try_into().unwrap(), (5, 4).try_into().unwrap()].into_iter(),
        );

        assert_eq!(distances.get((0, 0).try_into().unwrap()), 0);
        assert_eq!(distances.get((1, 1).try_into().unwrap()), 6);
        assert_eq!(distances.get((2, 0).try_into().unwrap()), 12);
        assert_eq!(distances.get((4, 4).try_into().unwrap()), 6);
        assert_eq!(distances.get((3, 2).try_into().unwrap()), 16);
    }
}
