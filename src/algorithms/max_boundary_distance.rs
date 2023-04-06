use crate::algorithms::matrix_common::MatrixCommon;
use crate::algorithms::room_matrix::RoomMatrix;
use crate::geometry::rect::{room_rect, Rect};
use crate::geometry::room_xy::RoomXYUtils;
use std::cmp::max;

/// Returns a matrix with maximum distance to rect's boundary in each tile.
/// Can be used to find the distance from furthest point from given set if used in conjunction with rect::bounding_rect.
pub fn max_boundary_distance(rect: Rect) -> RoomMatrix<u8> {
    let mut result = RoomMatrix::new(0);

    let top_right = rect.top_right();
    let bottom_left = rect.bottom_left();

    for xy in room_rect().iter() {
        result.set(
            xy,
            max(
                max(xy.dist(rect.top_left), xy.dist(rect.bottom_right)),
                max(xy.dist(top_right), xy.dist(bottom_left)),
            ),
        );
    }

    result
}
