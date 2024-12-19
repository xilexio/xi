use screeps::{ObjectId, RawObjectId, RoomObject, Structure, StructureObject};
use screeps::game::{get_object_by_id_erased, get_object_by_id_typed};
use crate::errors::XiError;

pub fn erased_object_by_id(id: RawObjectId) -> Result<RoomObject, XiError> {
    get_object_by_id_erased(&id).ok_or(XiError::ObjectDoesNotExist)
}

pub fn structure_object_by_id(id: ObjectId<Structure>) -> Result<StructureObject, XiError> {
    match get_object_by_id_typed(&id) {
        Some(obj) => Ok(obj.into()),
        None => Err(XiError::ObjectDoesNotExist),
    }
}