use crate::algorithms::matrix_common::MatrixCommon;
use crate::geometry::room_xy::RoomXYUtils;
use screeps::RoomXY;

/// Uses matrix produced by `distance_matrix` to find a shortest route from start wherever gradient goes, up to
/// distance `final_dist`.
pub fn shortest_path_by_matrix<M, D>(distance_matrix: &M, start: RoomXY, final_dist: D) -> Vec<RoomXY>
where
    M: MatrixCommon<D>,
    D: Copy + Ord,
{
    let mut path = vec![start];
    let mut current = start;
    let mut current_dist = distance_matrix.get(current);
    'main_loop: while current_dist > final_dist {
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

/// Uses matrix produced by `distance_matrix` to find a shortest route from start wherever gradient goes, up to
/// distance `final_dist`. When faced with two equally good route options as far as distance goes, selects the one with
/// the smallest value from preference matrix.
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
