use screeps::{Creep, ObjectId, Position, RawObjectId};
use crate::creeps::creep_body::CreepBody;
use crate::creeps::generic_creep::GenericCreep;
use crate::errors::XiError;
use crate::travel::surface::Surface;
use crate::travel::travel_state::TravelState;

pub struct TestCreep {
    pub name: String,
    pub id: u32,
    pub travel_state: TravelState,
    pub body: CreepBody,
    pub fatigue: u32,
}

impl TestCreep {
    pub fn new(id: u32, pos: Position, body: CreepBody) -> Self {
        TestCreep {
            name: format!("creep{}", id),
            id,
            travel_state: TravelState::new(pos),
            body,
            fatigue: 0,
        }
    }
}

impl GenericCreep for TestCreep {
    fn get_name(&self) -> &String {
        &self.name
    }

    fn get_screeps_id(&mut self) -> Result<ObjectId<Creep>, XiError> {
        Ok(RawObjectId::from_packed(self.id as u128).into())
    }

    fn get_travel_state(&self) -> &TravelState {
        &self.travel_state
    }

    fn get_travel_state_mut(&mut self) -> &mut TravelState {
        &mut self.travel_state
    }

    fn get_ticks_per_tile(&self, surface: Surface) -> u8 {
        self.body.ticks_per_tile(surface)
    }

    fn get_fatigue(&mut self) -> Result<u32, XiError> {
        Ok(self.fatigue)
    }
}