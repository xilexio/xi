use crate::algorithms::matrix_common::MatrixCommon;
use crate::consts::ROOM_AREA;
use crate::geometry::rect::room_rect;
use crate::geometry::room_xy::RoomXYUtils;
use screeps::{RoomXY, ROOM_SIZE};
use std::fmt::{Display, Formatter, LowerHex};
use std::mem::size_of;

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

    pub fn boundary(&self) -> impl Iterator<Item = (RoomXY, T)> + '_ {
        room_rect().boundary().map(|xy| (xy, self.get(xy)))
    }

    pub fn map<F, S>(&self, mut f: F) -> RoomMatrix<S>
    where
        F: FnMut(RoomXY, T) -> S,
        S: Clone + Copy + PartialEq + Default,
    {
        let mut data = [S::default(); ROOM_AREA];
        for (xy, value) in self.iter() {
            data[xy.to_index()] = f(xy, value);
        }
        RoomMatrix { data }
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

    fn iter_xy<'a, 'b>(&'a self) -> impl Iterator<Item = RoomXY> + 'b {
        (0..ROOM_AREA).map(|i| unsafe {
            RoomXY::unchecked_new((i % (ROOM_SIZE as usize)) as u8, (i / (ROOM_SIZE as usize)) as u8)
        })
    }
}

impl<T> Default for RoomMatrix<T>
where
    T: Clone + Copy + PartialEq + Default,
{
    fn default() -> Self {
        RoomMatrix::new(T::default())
    }
}

impl<T> Display for RoomMatrix<T>
where
    T: Clone + Copy + PartialEq + LowerHex + Sized,
{
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        for y in 0..ROOM_SIZE {
            for x in 0..ROOM_SIZE {
                unsafe {
                    write!(
                        f,
                        "{:0>size$x}",
                        self.get(RoomXY::unchecked_new(x, y)),
                        size = 2 * size_of::<T>()
                    )?;
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
