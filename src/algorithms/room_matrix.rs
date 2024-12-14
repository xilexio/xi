use crate::algorithms::matrix_common::MatrixCommon;
use crate::consts::ROOM_AREA;
use crate::geometry::rect::room_rect;
use crate::geometry::room_xy::RoomXYUtils;
use screeps::{RoomXY, ROOM_SIZE};
use serde::de::{Error, SeqAccess, Visitor};
use serde::ser::SerializeSeq;
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use std::fmt::{Display, Formatter, LowerHex};
use std::mem::size_of;

/// A `ROOM_SIZE` x `ROOM_SIZE` matrix backed by an array with size known at compile time.
#[derive(Debug, Clone)]
pub struct RoomMatrix<T> {
    pub data: [T; ROOM_AREA],
}

impl<T> RoomMatrix<T>
where
    T: Copy + PartialEq,
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
        S: Copy + PartialEq + Default,
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
    T: Copy + PartialEq,
{
    #[inline]
    fn get(&self, xy: RoomXY) -> T {
        self.data[xy.to_index()]
    }

    fn get_mut(&mut self, xy: RoomXY) -> &mut T {
        &mut self.data[xy.to_index()]
    }

    fn clone_filled(&self, fill: T) -> Self {
        RoomMatrix {
            data: [fill; ROOM_AREA]
        }
    }

    fn around_xy(&self, xy: RoomXY) -> impl Iterator<Item=RoomXY> {
        xy.around()
    }

    fn iter_xy<'it>(&self) -> impl Iterator<Item = RoomXY> + 'it {
        (0..ROOM_AREA).map(|i| unsafe {
            RoomXY::unchecked_new((i % (ROOM_SIZE as usize)) as u8, (i / (ROOM_SIZE as usize)) as u8)
        })
    }
}

impl<T> Default for RoomMatrix<T>
where
    T: Copy + PartialEq + Default,
{
    fn default() -> Self {
        RoomMatrix::new(T::default())
    }
}

impl<T> Display for RoomMatrix<T>
where
    T: Copy + PartialEq + LowerHex + Sized,
{
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "   ")?;
        for x in 0..ROOM_SIZE {
            write!(f, "{:>size$}", x, size = 2 * size_of::<T>())?;
            if x != ROOM_SIZE - 1 {
                write!(f, " ")?;
            }
        }
        writeln!(f)?;
        for y in 0..ROOM_SIZE {
            write!(f, "{:>size$} ", y, size = 2)?;

            for x in 0..ROOM_SIZE {
                unsafe {
                    write!(
                        f,
                        "{:0>size$x}",
                        self.get(RoomXY::unchecked_new(x, y)),
                        size = 2 * size_of::<T>()
                    )?;
                }
                if x != ROOM_SIZE - 1 {
                    write!(f, " ")?;
                }
            }
            writeln!(f)?;
        }
        Ok(())
    }
}

impl<T> Serialize for RoomMatrix<T>
where
    T: Serialize + Copy,
{
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let mut seq_serializer = serializer.serialize_seq(Some(2500))?;
        self.data
            .iter()
            .try_for_each(|val| seq_serializer.serialize_element(val))?;
        seq_serializer.end()
    }
}

impl<'de, T> Deserialize<'de> for RoomMatrix<T>
where
    T: Deserialize<'de> + Default + Serialize + Copy + PartialEq,
{
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        deserializer.deserialize_seq(RoomMatrixVisitor::default())
    }
}

#[derive(Default)]
struct RoomMatrixVisitor<T>
where
    T: Default + Copy + PartialEq,
{
    /// Buffer in which to place deserialized `RoomMatrix`. Starts with default values.
    buffer: RoomMatrix<T>,
    /// The number of elements of the buffer that are already filled.
    filled: usize,
}

impl<'de, T> Visitor<'de> for RoomMatrixVisitor<T>
where
    T: Deserialize<'de> + Default + Copy + PartialEq,
{
    type Value = RoomMatrix<T>;

    fn expecting(&self, formatter: &mut Formatter) -> std::fmt::Result {
        write!(formatter, "a sequence of {} serialized values", ROOM_AREA)
    }

    fn visit_seq<A>(mut self, mut seq: A) -> Result<Self::Value, A::Error>
    where
        A: SeqAccess<'de>,
    {
        for i in 0..ROOM_AREA {
            let val = seq.next_element()?.ok_or(Error::invalid_length(ROOM_AREA, &self))?;
            self.buffer.data[i] = val;
            self.filled += 1;
        }
        if seq.next_element::<T>()?.is_some() {
            return Err(Error::invalid_length(ROOM_AREA, &self));
        }
        Ok(self.buffer)
    }
}
