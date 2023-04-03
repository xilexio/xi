use crate::algorithms::matrix_common::MatrixCommon;
use crate::consts::ROOM_AREA;
use crate::geometry::rect::room_rect;
use crate::geometry::room_xy::RoomXYUtils;
use screeps::{RoomXY, ROOM_SIZE};
use std::fmt::{Display, Formatter, LowerHex};

/// A `ROOM_SIZE` x `ROOM_SIZE` matrix backed by an array with size known at compile time.
#[derive(Clone)]
pub struct RoomMatrix<T> {
    pub data: [T; ROOM_AREA],
}

impl<T> RoomMatrix<T>
where
    T: Clone + Copy + PartialEq,
{
    pub fn new(fill: T) -> Self {
        RoomMatrix {
            data: [fill; ROOM_AREA],
        }
    }

    pub fn exits(&self) -> impl Iterator<Item = (RoomXY, T)> + '_ {
        room_rect().boundary().map(|xy| (xy, self.get(xy)))
    }

    pub fn map<F, S>(&self, f: F) -> RoomMatrix<S>
    where
        F: FnMut(T) -> S,
        S: Clone + Copy + PartialEq,
    {
        RoomMatrix {
            data: self.data.map(f),
        }
    }
}

impl<T> MatrixCommon<T> for RoomMatrix<T>
where
    T: Clone + Copy + PartialEq,
{
    #[inline]
    fn get(&self, xy: RoomXY) -> T {
        self.data[xy.to_index()]
    }

    #[inline]
    fn set(&mut self, xy: RoomXY, value: T) {
        self.data[xy.to_index()] = value;
    }

    fn iter(&self) -> impl Iterator<Item = (RoomXY, T)> + '_ {
        (0..ROOM_AREA).map(|i| {
            let xy = unsafe {
                RoomXY::unchecked_new(
                    (i % (ROOM_SIZE as usize)) as u8,
                    (i / (ROOM_SIZE as usize)) as u8,
                )
            };
            (xy, self.get(xy))
        })
    }
}

impl<T> Display for RoomMatrix<T>
where
    T: Clone + Copy + PartialEq + LowerHex,
{
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        for y in 0..ROOM_SIZE {
            for x in 0..ROOM_SIZE {
                unsafe {
                    write!(f, "{:02x}", self.get(RoomXY::unchecked_new(x, y)))?;
                    if x != ROOM_SIZE - 1 {
                        write!(f, " ")?;
                    }
                }
            }
            writeln!(f)?;
        }
        Ok(())
    }
}
