use crate::algorithms::matrix_common::MatrixCommon;
use crate::geometry::room_xy::RoomXYUtils;
use screeps::RoomXY;

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

pub fn shortest_path_by_matrix_with_preference<M, N, D, P>(distance_matrix: &M, preference_matrix: &N, start: RoomXY, final_dist: D) -> Vec<RoomXY>
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
    while current_dist > final_dist {
        let mut found_next = false;
        for near in current.around() {
            let near_dist = distance_matrix.get(near);
            let near_preference = preference_matrix.get(near);
            if (near_dist, near_preference) < (current_dist, current_preference) {
                current = near;
                current_dist = near_dist;
                current_preference = near_preference;
                path.push(near);
                found_next = true;
            }
        }
        if !found_next {
            break;
        }
    }
    path
}
