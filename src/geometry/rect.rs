use crate::geometry::room_coordinate::RoomCoordinateUtils;
use crate::geometry::room_xy::RoomXYUtils;
use screeps::{RoomXY, ROOM_SIZE};
use std::cmp::{max, min};
use std::error::Error;
use std::fmt::{Display, Formatter};

#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub struct Rect {
    pub top_left: RoomXY,
    pub bottom_right: RoomXY,
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub struct InvalidRectError;

impl Display for InvalidRectError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "Rect does not have a positive area")
    }
}

impl Error for InvalidRectError {}

impl Rect {
    pub fn new(top_left: RoomXY, bottom_right: RoomXY) -> Result<Self, InvalidRectError> {
        let result = Rect { top_left, bottom_right };
        if result.is_valid() {
            Ok(result)
        } else {
            Err(InvalidRectError)
        }
    }

    pub fn new_unordered(xy1: RoomXY, xy2: RoomXY) -> Self {
        unsafe {
            Rect {
                top_left: RoomXY::unchecked_new(min(xy1.x.u8(), xy2.x.u8()), min(xy1.y.u8(), xy2.y.u8())),
                bottom_right: RoomXY::unchecked_new(max(xy1.x.u8(), xy2.x.u8()), max(xy1.y.u8(), xy2.y.u8())),
            }
        }
    }

    pub unsafe fn unchecked_new(top_left: RoomXY, bottom_right: RoomXY) -> Self {
        Rect { top_left, bottom_right }
    }

    pub fn top_right(self) -> RoomXY {
        (self.bottom_right.x, self.top_left.y).into()
    }

    pub fn bottom_left(self) -> RoomXY {
        (self.top_left.x, self.bottom_right.y).into()
    }

    pub fn is_valid(self) -> bool {
        self.top_left.x <= self.bottom_right.x && self.top_left.y <= self.bottom_right.y
    }

    pub fn width(self) -> u8 {
        self.bottom_right.x.u8() - self.top_left.x.u8() + 1
    }

    pub fn height(self) -> u8 {
        self.bottom_right.y.u8() - self.top_left.y.u8() + 1
    }

    pub fn area(self) -> usize {
        (self.width() as usize) * (self.height() as usize)
    }

    /// A tile with minimal distance to the center of the rectangle, top-left one if there are multiple choices.
    pub fn center(self) -> RoomXY {
        self.top_left.midpoint(self.bottom_right)
    }

    /// All tiles with minimal distance to the center of the rectangle, clockwise starting from top-left.
    pub fn centers(self) -> Vec<RoomXY> {
        let mut result = Vec::new();
        let c = self.center();
        result.push(c);
        if self.width() % 2 == 0 {
            result.push(unsafe { c.add_diff((1, 0)) });
            if self.height() % 2 == 0 {
                result.push(unsafe { c.add_diff((1, 1)) });
                result.push(unsafe { c.add_diff((0, 1)) });
            }
        } else if self.height() % 2 == 0 {
            result.push(unsafe { c.add_diff((0, 1)) });
        }
        result
    }

    /// Returns four points on the corners: top left, top right, bottom right, bottom left.
    /// These may be duplicates if the rectangle is small enough.
    pub fn corners(self) -> [RoomXY; 4] {
        [self.top_left, self.top_right(), self.bottom_right, self.bottom_left()]
    }

    /// Returns index of the corner closest to given point.
    pub fn closest_corner(self, xy: RoomXY) -> usize {
        let corners = self.corners();
        let mut closest_ix = 0;
        let mut min_dist = corners[0].dist(xy);
        for (i, corner) in corners.iter().skip(1).copied().enumerate() {
            if corner.dist(xy) < min_dist {
                closest_ix = i;
                min_dist = corner.dist(xy);
            }
        }
        closest_ix
    }

    pub fn contains(self, xy: RoomXY) -> bool {
        self.top_left.x <= xy.x && xy.x <= self.bottom_right.x && self.top_left.y <= xy.y && xy.y <= self.bottom_right.y
    }

    pub fn contains_i8xy(self, x: i8, y: i8) -> bool {
        self.top_left.x.u8() as i8 <= x
            && x <= self.bottom_right.x.u8() as i8
            && self.top_left.y.u8() as i8 <= y
            && y <= self.bottom_right.y.u8() as i8
    }

    pub fn extended(self, xy: RoomXY) -> Rect {
        Rect {
            top_left: (min(self.top_left.x, xy.x), min(self.top_left.y, xy.y)).into(),
            bottom_right: (max(self.bottom_right.x, xy.x), max(self.bottom_right.y, xy.y)).into(),
        }
    }

    pub fn boundary(self) -> impl Iterator<Item = RoomXY> {
        unsafe {
            let top = (0..self.width()).map(move |dx| (self.top_left.x.add_diff(dx as i8), self.top_left.y).into());
            let right =
                (1..self.height() - 1).map(move |dy| (self.bottom_right.x, self.top_left.y.add_diff(dy as i8)).into());
            let bottom = (0..if self.height() > 1 { self.width() } else { 0 })
                .map(move |dx| (self.bottom_right.x.add_diff(-(dx as i8)), self.bottom_right.y).into());
            let left = (1..if self.width() > 1 { self.height() - 1 } else { 1 })
                .map(move |dy| (self.top_left.x, self.bottom_right.y.add_diff(-(dy as i8))).into());

            top.chain(right).chain(bottom).chain(left)
        }
    }

    pub fn intersection(self, other: Rect) -> Result<Rect, InvalidRectError> {
        let left = max(self.top_left.x, other.top_left.x);
        let top = max(self.top_left.y, other.top_left.y);
        let right = min(self.bottom_right.x, other.bottom_right.x);
        let bottom = min(self.bottom_right.y, other.bottom_right.y);

        Rect::new((left, top).into(), (right, bottom).into())
    }

    pub fn iter(self) -> impl Iterator<Item = RoomXY> {
        let tlx = self.top_left.x.u8();
        let tly = self.top_left.y.u8();
        let w = self.width() as u16;
        let h = self.height() as u16;
        (0..(w * h)).map(move |i| unsafe { RoomXY::unchecked_new(tlx + ((i % w) as u8), tly + ((i / w) as u8)) })
    }
}

impl Default for Rect {
    fn default() -> Self {
        unsafe { Rect::unchecked_new(RoomXY::unchecked_new(0, 0), RoomXY::unchecked_new(0, 0)) }
    }
}

impl TryFrom<(RoomXY, RoomXY)> for Rect {
    type Error = InvalidRectError;

    fn try_from(xy_pair: (RoomXY, RoomXY)) -> Result<Self, Self::Error> {
        Rect::new(xy_pair.0, xy_pair.1)
    }
}

pub fn room_rect() -> Rect {
    unsafe {
        Rect::unchecked_new(
            RoomXY::unchecked_new(0, 0),
            RoomXY::unchecked_new(ROOM_SIZE - 1, ROOM_SIZE - 1),
        )
    }
}

/// A ball (square) with given center and radius (r=0 is a single tile, r=1 is 3x3).
pub fn ball(center: RoomXY, r: u8) -> Rect {
    unsafe {
        Rect {
            top_left: RoomXY::unchecked_new(
                if center.x.u8() <= r { 0 } else { center.x.u8() - r },
                if center.y.u8() <= r { 0 } else { center.y.u8() - r },
            ),
            bottom_right: RoomXY::unchecked_new(
                min(center.x.u8() + r, ROOM_SIZE - 1),
                min(center.y.u8() + r, ROOM_SIZE - 1),
            ),
        }
    }
}

/// Minimum rectangle that contains all given points.
pub fn bounding_rect<T>(mut points: T) -> Rect
where
    T: Iterator<Item = RoomXY>,
{
    let first = points.next().unwrap();
    let mut result = unsafe { Rect::unchecked_new(first, first) };
    for xy in points {
        result = result.extended(xy);
    }
    result
}

#[cfg(test)]
mod tests {
    use crate::geometry::rect::{InvalidRectError, Rect};
    use screeps::{RoomXY, ROOM_SIZE};

    #[test]
    fn test_iter() {
        let rect = unsafe { Rect::unchecked_new(RoomXY::unchecked_new(0, 0), RoomXY::unchecked_new(ROOM_SIZE - 1, 5)) };
        let mut number_of_points = 0;
        for xy in rect.iter() {
            number_of_points += 1
        }
        assert_eq!(number_of_points, rect.area());
        assert_eq!(rect.iter().next(), Some((0, 0).try_into().unwrap()));
    }

    #[test]
    fn test_intersection() {
        let rect1 = Rect::new_unordered((0, 0).try_into().unwrap(), (5, 5).try_into().unwrap());
        let rect2 = Rect::new_unordered((1, 4).try_into().unwrap(), (3, 6).try_into().unwrap());
        let rect3 = Rect::new_unordered((4, 4).try_into().unwrap(), (6, 6).try_into().unwrap());

        assert_eq!(
            rect1.intersection(rect2),
            Ok(Rect::new_unordered(
                (1, 4).try_into().unwrap(),
                (3, 5).try_into().unwrap()
            ))
        );
        assert_eq!(rect2.intersection(rect3), Err(InvalidRectError));
        assert_eq!(
            rect3.intersection(rect1),
            Ok(Rect::new_unordered(
                (4, 4).try_into().unwrap(),
                (5, 5).try_into().unwrap()
            ))
        );
    }

    #[test]
    fn test_boundary() {
        let rect1 = Rect::new_unordered((0, 0).try_into().unwrap(), (0, 0).try_into().unwrap());
        let rect2 = Rect::new_unordered((1, 1).try_into().unwrap(), (1, 2).try_into().unwrap());
        let rect3 = Rect::new_unordered((1, 1).try_into().unwrap(), (2, 1).try_into().unwrap());
        let rect4 = Rect::new_unordered((1, 1).try_into().unwrap(), (2, 2).try_into().unwrap());
        let rect5 = Rect::new_unordered((1, 1).try_into().unwrap(), (3, 3).try_into().unwrap());

        assert_eq!(rect1.boundary().collect::<Vec<_>>(), vec![(0, 0).try_into().unwrap()]);
        assert_eq!(
            rect2.boundary().collect::<Vec<_>>(),
            vec![(1, 1).try_into().unwrap(), (1, 2).try_into().unwrap()]
        );
        assert_eq!(
            rect3.boundary().collect::<Vec<_>>(),
            vec![(1, 1).try_into().unwrap(), (2, 1).try_into().unwrap()]
        );
        assert_eq!(
            rect4.boundary().collect::<Vec<_>>(),
            vec![
                (1, 1).try_into().unwrap(),
                (2, 1).try_into().unwrap(),
                (2, 2).try_into().unwrap(),
                (1, 2).try_into().unwrap(),
            ]
        );
        assert_eq!(
            rect5.boundary().collect::<Vec<_>>(),
            vec![
                (1, 1).try_into().unwrap(),
                (2, 1).try_into().unwrap(),
                (3, 1).try_into().unwrap(),
                (3, 2).try_into().unwrap(),
                (3, 3).try_into().unwrap(),
                (2, 3).try_into().unwrap(),
                (1, 3).try_into().unwrap(),
                (1, 2).try_into().unwrap(),
            ]
        );
    }
}
