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

    fn iter_xy<'a, 'b>(&'a self) -> impl Iterator<Item = RoomXY> + 'b;

    fn iter(&self) -> impl Iterator<Item = (RoomXY, T)> + '_ {
        self.iter_xy().map(move |xy| (xy, self.get(xy)))
    }

    fn find_xy<'a>(&'a self, value: T) -> impl Iterator<Item = RoomXY> + 'a
    where
        T: 'a,
    {
        self.iter().filter_map(move |(xy, v)| (v == value).then_some(xy))
    }

    fn find_not_xy<'a>(&'a self, value: T) -> impl Iterator<Item = RoomXY> + 'a
    where
        T: 'a,
    {
        self.iter().filter_map(move |(xy, v)| (v != value).then_some(xy))
    }

    fn update<F>(&mut self, mut f: F)
    where
        F: FnMut(RoomXY, T) -> T,
    {
        for xy in self.iter_xy() {
            self.set(xy, f(xy, self.get(xy)));
        }
    }

    fn set_from<M>(&mut self, matrix: &M)
    where
        M: MatrixCommon<T>,
    {
        for (xy, value) in matrix.iter() {
            self.set(xy, value);
        }
    }

    fn merge_from<M, S, F>(&mut self, matrix: &M, mut merge: F)
    where
        M: MatrixCommon<S>,
        S: Clone + Copy + PartialEq,
        F: FnMut(T, S) -> T,
    {
        for (xy, value) in matrix.iter() {
            self.set(xy, merge(self.get(xy), value));
        }
    }

    fn min(&self) -> (RoomXY, T)
    where
        T: Ord,
    {
        let mut it = self.iter();
        let mut result = it.next().unwrap();
        it.for_each(|v| {
            if v.1 < result.1 {
                result = v;
            }
        });
        return result;
    }
}
