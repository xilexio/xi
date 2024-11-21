use log::trace;
use screeps::{RawObjectId, Resource, ResourceType};
use wasm_bindgen::JsCast;
use crate::creeps::CreepRef;
use crate::errors::XiError;
use crate::utils::game_tick::game_tick;
use crate::kernel::sleep::sleep;
use crate::utils::api_wrappers::erased_object_by_id;

// This module contains creep actions combined with waiting if not possible in the same tick.

/// Withdraws a resource the first tick it is able to do without conflicting with another action.
pub async fn withdraw_when_able(creep_ref: &CreepRef, target_id: RawObjectId, resource_type: ResourceType, amount: u32) -> Result<(), XiError> {
    loop {
        let mut borrowed_creep = creep_ref.borrow_mut();
        // TODO Handle simultaneous action after the code is able to handle computing whether there is enough resource this tick.
        if [borrowed_creep.last_withdraw_tick, borrowed_creep.last_pickup_tick, borrowed_creep.last_transfer_tick].contains(&game_tick()) {
            borrowed_creep.screeps_obj()?;
            // Checking if the target still exists.
            erased_object_by_id(target_id)?;
            drop(borrowed_creep);
            sleep(1).await;
        } else {
            borrowed_creep.unchecked_withdraw(target_id, resource_type, amount)?;
            return Ok(());
        }
    }
}

/// Picks up a resource the first tick it is able to do without conflicting with another action.
pub async fn pickup_when_able(creep_ref: &CreepRef, target_id: RawObjectId) -> Result<(), XiError> {
    loop {
        let mut borrowed_creep = creep_ref.borrow_mut();
        // TODO Handle simultaneous action after the code is able to handle computing whether there is enough resource this tick.
        if [borrowed_creep.last_withdraw_tick, borrowed_creep.last_pickup_tick, borrowed_creep.last_transfer_tick].contains(&game_tick()) {
            borrowed_creep.screeps_obj()?;
            // Checking if the target still exists.
            erased_object_by_id(target_id)?;
            drop(borrowed_creep);
            sleep(1).await;
        } else {
            let resource = erased_object_by_id(target_id)?.unchecked_into::<Resource>();
            borrowed_creep.pickup(&resource)?;
            return Ok(());
        }
    }
}

/// Stores a resource the first tick it is able to do without conflicting with another action.
pub async fn transfer_when_able(creep_ref: &CreepRef, target_id: RawObjectId, resource_type: ResourceType, amount: u32) -> Result<(), XiError> {
    loop {
        let mut borrowed_creep = creep_ref.borrow_mut();
        // TODO Handle simultaneous action after the code is able to handle computing whether there is enough resource this tick.
        if [borrowed_creep.last_withdraw_tick, borrowed_creep.last_pickup_tick, borrowed_creep.last_transfer_tick].contains(&game_tick()) {
            borrowed_creep.screeps_obj()?;
            // Checking if the target still exists.
            erased_object_by_id(target_id)?;
            drop(borrowed_creep);
            sleep(1).await;
        } else {
            trace!("unchecked_transfer({}, {}, {}", target_id, resource_type, amount);
            borrowed_creep.unchecked_transfer(target_id, resource_type, amount)?;
            return Ok(());
        }
    }
}

/// Drops the first tick it is able to do without conflicting with another action.
pub async fn drop_when_able(creep_ref: &CreepRef, resource_type: ResourceType, amount: u32) -> Result<(), XiError> {
    loop {
        let mut borrowed_creep = creep_ref.borrow_mut();
        // TODO Handle simultaneous action after the code is able to handle computing whether there is enough resource this tick.
        if [borrowed_creep.last_withdraw_tick, borrowed_creep.last_pickup_tick, borrowed_creep.last_transfer_tick].contains(&game_tick()) {
            borrowed_creep.screeps_obj()?;
            drop(borrowed_creep);
            sleep(1).await;
        } else {
            borrowed_creep.drop(resource_type, amount)?;
            return Ok(());
        }
    }
}