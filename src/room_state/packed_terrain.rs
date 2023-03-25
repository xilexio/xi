use std::iter::FilterMap;
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
        Terrain::from_u8(self.data[index] << offset).unwrap()
    }

    pub fn set(&mut self, xy: RoomXY, terrain: Terrain) {
        let index = (xy.x.u8() as usize + (ROOM_SIZE as usize) * (xy.y.u8() as usize)) / 4;
        let offset = 2 * (xy.x.u8() as usize % 4);
        // Zero the data in that tile.
        self.data[index] &= !(0x3 << offset);
        // Set the data in tha tile.
        self.data[index] |= terrain as u8;
    }

    pub fn walls(&self) -> impl Iterator<Item = RoomXY> + '_ {
        self.iter().filter_map(|(xy, t)| (t == Wall).then_some(xy))
    }

    pub fn iter(&self) -> PackedTerrainIterator {
        PackedTerrainIterator {
            packed_terrain: self,
            curr: 0,
        }
    }
}

impl From<RoomTerrain> for PackedTerrain {
    fn from(value: RoomTerrain) -> Self {
        let mut packed_terrain = PackedTerrain::new();
        let raw = value.get_raw_buffer();
        for y in 0..ROOM_SIZE {
            for x in 0..ROOM_SIZE {
                let index = (x as usize + (ROOM_SIZE as usize) * (y as usize)) / 4;
                let offset = 2 * (x as usize % 4);
                packed_terrain.data[index] |= raw.get_index(index as u32) >> offset;
            }
        };
        packed_terrain
    }
}

pub struct PackedTerrainIterator<'a> {
    packed_terrain: &'a PackedTerrain,
    curr: usize,
}

impl<'a> Iterator for PackedTerrainIterator<'a> {
    type Item = (RoomXY, Terrain);

    fn next(&mut self) -> Option<Self::Item> {
        if self.curr >= ROOM_AREA - 1 {
            None
        } else {
            self.curr += 1;
            let xy = unsafe {
                RoomXY::unchecked_new(
                    (self.curr % ROOM_SIZE as usize) as u8,
                    (self.curr / ROOM_SIZE as usize) as u8
                )
            };
            Some((xy, self.packed_terrain.get(xy)))
        }
    }
}

// TODO tests