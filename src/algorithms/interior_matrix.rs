use crate::algorithms::matrix_common::MatrixCommon;
use crate::algorithms::room_matrix::RoomMatrix;
use crate::geometry::rect::room_rect;
use screeps::RoomXY;
use crate::geometry::room_xy::RoomXYUtils;

/// Returns a matrix with information what is outside or an obstacle (false) and what inside or on the cut (true),
/// given a list of obstacles and a cut supposed to divide exit tiles from the interior.
pub fn interior_matrix<O, C>(obstacles: O, cut: C, obstacles_included: bool, cut_included: bool) -> RoomMatrix<bool>
where
    O: Iterator<Item = RoomXY>,
    C: Iterator<Item = RoomXY>,
{
    let mut result = RoomMatrix::new(true);
    let mut obstacles_vec = Vec::new();
    for xy in obstacles {
        result.set(xy, false);
        if obstacles_included {
            obstacles_vec.push(xy);
        }
    }

    let not_obstacle_matrix = result.clone();

    let mut layer = room_rect()
        .boundary()
        .filter(|&xy| not_obstacle_matrix.get(xy))
        .collect::<Vec<_>>();

    for &xy in layer.iter() {
        result.set(xy, false);
    }

    let cut_vec = cut.collect::<Vec<_>>();

    for &xy in cut_vec.iter() {
        result.set(xy, false);
    }

    while !layer.is_empty() {
        let mut next_layer = Vec::new();

        for xy in layer.into_iter() {
            for near in xy.around() {
                if not_obstacle_matrix.get(near) && result.get(near) {
                    next_layer.push(near);
                    result.set(near, false);
                }
            }
        }

        layer = next_layer;
    }

    if cut_included {
        for xy in cut_vec.into_iter() {
            result.set(xy, true);
        }
    }

    if obstacles_included {
        for xy in obstacles_vec.into_iter() {
            result.set(xy, true);
        }
    }

    result
}
