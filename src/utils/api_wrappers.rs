use screeps::{RawObjectId, RoomObject};
use screeps::game::get_object_by_id_erased;
use crate::errors::XiError;

pub fn erased_object_by_id(id: RawObjectId) -> Result<RoomObject, XiError> {
    get_object_by_id_erased(&id).ok_or(XiError::ObjectDoesNotExist)
}