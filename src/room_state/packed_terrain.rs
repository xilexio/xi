use screeps::{ROOM_SIZE, RoomTerrain, RoomXY, Terrain};
use num_traits::cast::FromPrimitive;
use screeps::Terrain::Wall;
use crate::consts::ROOM_AREA;

pub const PACKED_TERRAIN_DATA_SIZE: usize = ROOM_AREA / 4;

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
        let index = (xy.x.u8() as usize + (ROOM_SIZE as usize) * (xy.y.u8() as usize)) / 4;
        let offset = 2 * (xy.x.u8() as usize % 4);
        Terrain::from_u8((self.data[index] >> offset) & 3).unwrap()
    }

    pub fn set(&mut self, xy: RoomXY, terrain: Terrain) {
        let index = (xy.x.u8() as usize + (ROOM_SIZE as usize) * (xy.y.u8() as usize)) / 4;
        let offset = 2 * (xy.x.u8() as usize % 4);
        // Zero the data in that tile.
        self.data[index] &= !(0x3 << offset);
        // Set the data in tha tile.
        self.data[index] |= (terrain as u8) << offset;
    }

    pub fn walls(&self) -> impl Iterator<Item = RoomXY> + '_ {
        self.iter().filter_map(|(xy, t)| (t == Wall).then_some(xy))
    }

    pub fn iter(&self) -> impl Iterator<Item = (RoomXY, Terrain)> + '_ {
        (0..ROOM_AREA).map(|i| {
            let xy = unsafe {
                RoomXY::unchecked_new(
                    (i % ROOM_SIZE as usize) as u8,
                    (i / ROOM_SIZE as usize) as u8
                )
            };
            (xy, self.get(xy))
        })
    }
}

impl From<RoomTerrain> for PackedTerrain {
    fn from(value: RoomTerrain) -> Self {
        let mut packed_terrain = PackedTerrain::new();
        let raw = value.get_raw_buffer();
        for y in 0..ROOM_SIZE {
            for x in 0..ROOM_SIZE {
                let index = x as usize + (ROOM_SIZE as usize) * (y as usize);
                let offset = 2 * (x % 4);
                packed_terrain.data[index / 4] |= raw.get_index(index as u32) << offset;
            }
        };
        packed_terrain
    }
}

#[cfg(test)]
mod tests {
    use screeps::Terrain::{Plain, Swamp, Wall};
    use crate::consts::ROOM_AREA;
    use crate::room_state::packed_terrain::PackedTerrain;

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
        assert_eq!(walls, vec![
            (10, 10).try_into().unwrap(),
            (11, 10).try_into().unwrap(),
            (10, 11).try_into().unwrap(),
        ]);
    }
}