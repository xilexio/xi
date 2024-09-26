use crate::creeps::CreepRef;
use screeps::{ResourceType, Transferable};

pub async fn travel_and_transfer<T>(creep_ref: &CreepRef, target: &T, resource_type: ResourceType, amount: u32)
where
    T: Transferable,
{
}
