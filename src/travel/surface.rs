use screeps::{Terrain, MOVE_COST_PLAIN, MOVE_COST_ROAD, MOVE_COST_SWAMP};
use crate::travel::surface::Surface::*;

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum Surface {
    Road,
    Plain,
    Swamp,
    Obstacle
}

impl Surface {
    pub fn move_cost(&self) -> u8 {
        match self {
            Road => MOVE_COST_ROAD as u8,
            Plain => MOVE_COST_PLAIN as u8,
            Swamp => MOVE_COST_SWAMP as u8,
            Obstacle => u8::MAX
        }
    }
}

impl From<Terrain> for Surface {
    fn from(terrain: Terrain) -> Self {
        match terrain {
            Terrain::Plain => Plain,
            Terrain::Wall => Obstacle,
            Terrain::Swamp => Swamp
        }
    }
}

impl Default for Surface {
    fn default() -> Self {
        Plain
    }
}