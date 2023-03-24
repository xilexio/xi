use screeps::RoomXY;

pub trait MatrixCommon<T> {
    fn get(&self, xy: RoomXY) -> T;
    fn set(&mut self, xy: RoomXY, value: T);
}
