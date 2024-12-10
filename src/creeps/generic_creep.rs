use screeps::ObjectId;
use crate::errors::XiError;
use crate::travel::surface::Surface;
use crate::travel::travel_state::TravelState;

/// A trait containing various getters appropriate for a Creep.
/// Used to make code with creeps testable.
pub trait GenericCreep {
    fn get_name(&self) -> &String;
    fn get_screeps_id(&mut self) -> Result<ObjectId<screeps::Creep>, XiError>;
    fn get_travel_state(&self) -> &TravelState;
    fn get_travel_state_mut(&mut self) -> &mut TravelState;
    fn get_ticks_per_tile(&self, surface: Surface) -> u8;
    fn get_fatigue(&mut self) -> Result<u32, XiError>;
}