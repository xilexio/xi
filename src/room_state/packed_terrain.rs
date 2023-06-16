use crate::algorithms::matrix_common::MatrixCommon;
use crate::algorithms::room_matrix::RoomMatrix;
use crate::consts::{OBSTACLE_COST, ROOM_AREA};
use num_traits::cast::FromPrimitive;
use screeps::Terrain::{Plain, Swamp, Wall};
use screeps::{RoomTerrain, RoomXY, Terrain, ROOM_SIZE};
use std::fmt::{Display, Formatter};
use crate::algorithms::weighted_distance_matrix::obstacle_cost;

pub const PACKED_TERRAIN_DATA_SIZE: usize = ROOM_AREA / 4;

#[derive(Copy, Clone, Debug)]
pub struct PackedTerrain {
    // Two bits required to store Terrain::Plain (0), Terrain::Wall (1) or Terrain::Swamp (2)
    pub data: [u8; PACKED_TERRAIN_DATA_SIZE],
}

impl PackedTerrain {
    pub fn new() -> Self {
        PackedTerrain {
            data: [0; PACKED_TERRAIN_DATA_SIZE],
        }
    }

    pub fn get(&self, xy: RoomXY) -> Terrain {
        let index = xy.x.u8() as usize + (ROOM_SIZE as usize) * (xy.y.u8() as usize);
        let offset = 2 * (index % 4);
        let terrain_u8 = (self.data[index / 4] >> offset) & 3;
        debug_assert!(terrain_u8 < 3);
        Terrain::from_u8(terrain_u8).unwrap()
    }

    pub fn set(&mut self, xy: RoomXY, terrain: Terrain) {
        let index = xy.x.u8() as usize + (ROOM_SIZE as usize) * (xy.y.u8() as usize);
        let offset = 2 * (index % 4);
        // Zero the data in that tile.
        self.data[index / 4] &= !(0x3 << offset);
        // Set the data in tha tile.
        self.data[index / 4] |= (terrain as u8) << offset;
    }

    pub fn walls(&self) -> impl Iterator<Item = RoomXY> + '_ {
        self.iter().filter_map(|(xy, t)| (t == Wall).then_some(xy))
    }

    pub fn not_walls(&self) -> impl Iterator<Item = RoomXY> + '_ {
        self.iter().filter_map(|(xy, t)| (t != Wall).then_some(xy))
    }

    pub fn iter(&self) -> impl Iterator<Item = (RoomXY, Terrain)> + '_ {
        (0..ROOM_AREA).map(|i| {
            let xy = unsafe {
                RoomXY::unchecked_new(
                    (i % ROOM_SIZE as usize) as u8,
                    (i / ROOM_SIZE as usize) as u8,
                )
            };
            let r = (xy, self.get(xy));
            r
        })
    }

    pub fn to_obstacle_matrix(&self, fill: u8) -> RoomMatrix<u8> {
        let mut result = RoomMatrix::new(fill);
        for (xy, t) in self.iter() {
            if t == Wall {
                result.set(xy, OBSTACLE_COST);
            }
        }
        result
    }

    pub fn to_cost_matrix(&self, multiplier: u8) -> RoomMatrix<u8> {
        let mut result = RoomMatrix::new(multiplier);
        for (xy, t) in self.iter() {
            let cost = match t {
                Plain => multiplier,
                Swamp => 5 * multiplier,
                Wall => obstacle_cost(),
            };
            result.set(xy, cost);
        }
        result
    }
}

impl Default for PackedTerrain {
    fn default() -> Self {
        PackedTerrain::new()
    }
}

impl From<RoomTerrain> for PackedTerrain {
    fn from(value: RoomTerrain) -> Self {
        let mut packed_terrain = PackedTerrain::new();
        for y in 0..ROOM_SIZE {
            for x in 0..ROOM_SIZE {
                let index = x as usize + (ROOM_SIZE as usize) * (y as usize);
                let offset = 2 * (index % 4);
                packed_terrain.data[index / 4] |= (value.get(x, y) as u8) << offset;
            }
        }
        packed_terrain
    }
}

impl Display for PackedTerrain {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        for y in 0..ROOM_SIZE {
            for x in 0..ROOM_SIZE {
                let xy = unsafe { RoomXY::unchecked_new(x, y) };
                match self.get(xy) {
                    Plain => write!(f, ".")?,
                    Swamp => write!(f, "~")?,
                    Wall => write!(f, "#")?,
                }
            }
            writeln!(f)?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use crate::consts::ROOM_AREA;
    use crate::room_state::packed_terrain::PackedTerrain;
    use screeps::Terrain::{Plain, Swamp, Wall};
    use screeps::{ROOM_SIZE, RoomXY, Terrain};
    use crate::geometry::rect::room_rect;

    #[test]
    fn test_set_get() {
        let mut terrain = PackedTerrain::new();
        terrain.set((1, 0).try_into().unwrap(), Plain);
        assert_eq!(terrain.get((1, 0).try_into().unwrap()), Plain);
        terrain.set((2, 1).try_into().unwrap(), Swamp);
        assert_eq!(terrain.get((2, 1).try_into().unwrap()), Swamp);
        terrain.set((3, 2).try_into().unwrap(), Wall);
        assert_eq!(terrain.get((3, 2).try_into().unwrap()), Wall);
        terrain.set((4, 3).try_into().unwrap(), Swamp);
        assert_eq!(terrain.get((4, 3).try_into().unwrap()), Swamp);

        terrain.set((0, 0).try_into().unwrap(), Plain);
        terrain.set((1, 0).try_into().unwrap(), Swamp);
        terrain.set((2, 0).try_into().unwrap(), Wall);
        terrain.set((3, 0).try_into().unwrap(), Swamp);
        assert_eq!(terrain.get((0, 0).try_into().unwrap()), Plain);
        assert_eq!(terrain.get((1, 0).try_into().unwrap()), Swamp);
        assert_eq!(terrain.get((2, 0).try_into().unwrap()), Wall);
        assert_eq!(terrain.get((3, 0).try_into().unwrap()), Swamp);

        terrain.set((ROOM_SIZE - 1, 0).try_into().unwrap(), Wall);
        assert_eq!(terrain.get((ROOM_SIZE - 1, 0).try_into().unwrap()), Wall);
        terrain.set((ROOM_SIZE - 1, 1).try_into().unwrap(), Wall);
        assert_eq!(terrain.get((ROOM_SIZE - 1, 1).try_into().unwrap()), Wall);
    }

    #[test]
    fn test_set_get_large() {
        let mut terrain = PackedTerrain::new();

        fn some_terrain(xy: RoomXY) -> Terrain {
            match (xy.x.u8() + xy.y.u8()) % 13 {
                0 => Plain,
                2 => Plain,
                4 => Plain,
                5 => Swamp,
                8 => Swamp,
                9 => Plain,
                11 => Plain,
                12 => Swamp,
                _ => Wall,
            }
        }

        for xy in room_rect().iter() {
            terrain.set(xy, some_terrain(xy))
        }
        for xy in room_rect().iter() {
            assert_eq!(terrain.get(xy), some_terrain(xy));
        }
    }

    #[test]
    fn test_iter() {
        let mut terrain = PackedTerrain::new();
        terrain.set((10, 10).try_into().unwrap(), Wall);
        terrain.set((11, 10).try_into().unwrap(), Wall);
        terrain.set((10, 11).try_into().unwrap(), Wall);
        let mut number_of_tiles = 0;
        let mut walls = Vec::new();
        for (xy, t) in terrain.iter() {
            number_of_tiles += 1;
            if t == Wall {
                walls.push(xy);
            }
        }
        assert_eq!(number_of_tiles, ROOM_AREA);
        assert_eq!(
            walls,
            vec![
                (10, 10).try_into().unwrap(),
                (11, 10).try_into().unwrap(),
                (10, 11).try_into().unwrap(),
            ]
        );
    }
}
