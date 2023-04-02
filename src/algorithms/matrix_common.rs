use screeps::RoomXY;

pub trait MatrixCommon<T>
where
    T: Clone + Copy + PartialEq,
{
    fn get(&self, xy: RoomXY) -> T;
    fn set(&mut self, xy: RoomXY, value: T);

    /// `x` and `y` are required to be within room bounds.
    #[inline]
    unsafe fn get_xy(&self, x: u8, y: u8) -> T {
        self.get(RoomXY::unchecked_new(x, y))
    }

    /// `x` and `y` are required to be within room bounds.
    #[inline]
    unsafe fn set_xy(&mut self, x: u8, y: u8, value: T) {
        self.set(RoomXY::unchecked_new(x, y), value)
    }

    fn iter(&self) -> impl Iterator<Item = (RoomXY, T)> + '_;

    fn find_xy<'a>(&'a self, value: T) -> impl Iterator<Item = RoomXY> + 'a
    where
        T: 'a,
    {
        self.iter()
            .filter_map(move |(xy, v)| (v == value).then_some(xy))
    }

    fn find_not_xy<'a>(&'a self, value: T) -> impl Iterator<Item = RoomXY> + 'a
    where
        T: 'a,
    {
        self.iter()
            .filter_map(move |(xy, v)| (v != value).then_some(xy))
    }
}
