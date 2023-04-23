use crate::algorithms::matrix_common::MatrixCommon;
use crate::geometry::rect::ball;
use crate::geometry::room_xy::RoomXYUtils;
use crate::unwrap;
use screeps::RoomXY;

#[inline]
pub fn distance_by_matrix<M, D>(distance_matrix: &M, target: RoomXY, target_circle_radius: u8) -> D
where
    M: MatrixCommon<D>,
    D: Copy + Ord,
{
    unwrap!(ball(target, target_circle_radius)
        .boundary()
        .map(|xy| distance_matrix.get(xy))
        .min())
}

#[inline]
pub fn closest_in_circle_by_matrix<M, D>(distance_matrix: &M, target: RoomXY, target_circle_radius: u8) -> (RoomXY, D)
where
    M: MatrixCommon<D>,
    D: Copy + Ord,
{
    unwrap!(ball(target, target_circle_radius)
        .boundary()
        .map(|xy| (xy, distance_matrix.get(xy)))
        .min_by_key(|&(_, d)| d))
}

/// Uses matrix produced by `distance_matrix` to find a shortest route from start wherever gradient goes, up to
/// distance `final_dist`, inclusive, or until it cannot decrease anymore.
pub fn shortest_path_by_distance_matrix<M, D>(distance_matrix: &M, start: RoomXY, final_dist: D) -> Vec<RoomXY>
where
    M: MatrixCommon<D>,
    D: Copy + Ord,
{
    let mut path = vec![start];
    let mut current = start;
    let mut current_dist = distance_matrix.get(current);
    'main_loop: while current_dist >= final_dist {
        for near in current.around() {
            let near_dist = distance_matrix.get(near);
            if near_dist < current_dist {
                current = near;
                current_dist = near_dist;
                path.push(near);
                continue 'main_loop;
            }
        }
        break;
    }
    path
}

/// Uses matrix produced by `distance_matrix` to find a shortest route from start wherever gradient goes until it cannot
/// decrease anymore. When faced with two equally good route options as far as distance goes, selects
/// the one with the smallest value from preference matrix.
pub fn shortest_path_by_matrix_with_preference<M, N, D, P>(
    distance_matrix: &M,
    preference_matrix: &N,
    start: RoomXY,
) -> Vec<RoomXY>
where
    M: MatrixCommon<D>,
    D: Copy + Ord,
    N: MatrixCommon<P>,
    P: Copy + Ord,
{
    let mut path = vec![start];
    let mut current = start;
    let mut current_dist = distance_matrix.get(current);
    let mut current_preference = preference_matrix.get(current);
    loop {
        let prev_dist = current_dist;
        for near in current.around() {
            let near_dist = distance_matrix.get(near);
            let near_preference = preference_matrix.get(near);
            if (near_dist, near_preference) < (current_dist, current_preference) {
                current = near;
                current_dist = near_dist;
                current_preference = near_preference;
            }
        }
        if current_dist < prev_dist {
            path.push(current);
        } else {
            break;
        }
    }
    path
}

#[cfg(test)]
mod tests {
    use crate::algorithms::matrix_common::MatrixCommon;
    use crate::algorithms::room_matrix::RoomMatrix;
    use crate::algorithms::shortest_path_by_distance_matrix::closest_in_circle_by_matrix;
    use crate::algorithms::weighted_distance_matrix::{obstacle_cost, unreachable_cost};

    #[test]
    fn test_closest_in_circle_by_matrix() {
        let mut matrix = RoomMatrix::new(10u16);
        matrix.set((10, 10).try_into().unwrap(), unreachable_cost());
        matrix.set((11, 10).try_into().unwrap(), 8);
        matrix.set((12, 10).try_into().unwrap(), 4);
        matrix.set((11, 11).try_into().unwrap(), 1);
        matrix.set((12, 11).try_into().unwrap(), obstacle_cost());

        assert_eq!(
            closest_in_circle_by_matrix(&matrix, (11, 11).try_into().unwrap(), 1),
            ((12, 10).try_into().unwrap(), 4)
        );
    }
}
