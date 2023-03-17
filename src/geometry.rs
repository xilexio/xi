use std::ops;

#[derive(Eq, PartialEq, Clone, Copy, Hash, Debug)]
pub struct Point {
    pub x: i8,
    pub y: i8,
}

impl Point {
    pub fn new(x: i8, y: i8) -> Self {
        Point {
            x,
            y,
        }
    }
}

impl ops::Add<Point> for Point {
    type Output = Point;

    fn add(self, rhs: Point) -> Self::Output {
        Point {
            x: self.x + rhs.x,
            y: self.y + rhs.y,
        }
    }
}

impl ops::Sub<Point> for Point {
    type Output = Point;

    fn sub(self, rhs: Point) -> Self::Output {
        Point {
            x: self.x - rhs.x,
            y: self.y - rhs.y,
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::geometry::Point;

    #[test]
    fn test_something() {
        let p1 = Point::new(1, 1);
        let p2 = Point::new(-1, 2);
        let p3 = Point::new(0, 3);
        let p = p1 + p2 - p3;
        assert_eq!(p, Point::new(0, 0));
    }
}