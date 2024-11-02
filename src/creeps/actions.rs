use screeps::{RawObjectId, Resource, ResourceType};
use wasm_bindgen::JsCast;
use crate::creeps::CreepRef;
use crate::errors::XiError;
use crate::game_tick::game_tick;
use crate::kernel::sleep::sleep;
use crate::utils::api_wrappers::erased_object_by_id;

// This module contains creep actions combined with waiting if not possible in the same tick.

/// Withdraws a resource the first tick it is able to do without conflicting with another action.
pub async fn withdraw_when_able(creep_ref: &CreepRef, target_id: RawObjectId, resource_type: ResourceType, amount: Option<u32>) -> Result<(), XiError> {
    loop {
        // TODO Switch to last_withdraw_tick after the code can handle computing whether there is enough resource this tick.
        if creep_ref.borrow().last_transfer_tick == game_tick() {
            creep_ref.borrow_mut().screeps_obj()?;
            erased_object_by_id(&target_id)?;
            sleep(1).await;
        } else {
            let target = erased_object_by_id(&target_id)?;
            creep_ref.borrow_mut().unchecked_withdraw(&target, resource_type, amount)?;
            creep_ref.borrow_mut().last_transfer_tick = game_tick();
            return Ok(());
        }
    }
}

/// Picks up a resource the first tick it is able to do without conflicting with another action.
pub async fn pickup_when_able(creep_ref: &CreepRef, target_id: RawObjectId) -> Result<(), XiError> {
    loop {
        // TODO Switch to last_pickup_tick after the code can handle computing whether there is enough resource this tick.
        if creep_ref.borrow().last_transfer_tick == game_tick() {
            creep_ref.borrow_mut().screeps_obj()?;
            erased_object_by_id(&target_id)?;
            sleep(1).await;
        } else {
            let resource = erased_object_by_id(&target_id)?.unchecked_into::<Resource>();
            creep_ref.borrow_mut().pickup(&resource)?;
            creep_ref.borrow_mut().last_transfer_tick = game_tick();
            return Ok(());
        }
    }
}

/// Stores a resource the first tick it is able to do without conflicting with another action.
pub async fn transfer_when_able(creep_ref: &CreepRef, target_id: RawObjectId, resource_type: ResourceType, amount: Option<u32>) -> Result<(), XiError> {
    loop {
        if creep_ref.borrow().last_transfer_tick == game_tick() {
            creep_ref.borrow_mut().screeps_obj()?;
            erased_object_by_id(&target_id)?;
            sleep(1).await;
        } else {
            let target = erased_object_by_id(&target_id)?;
            creep_ref.borrow_mut().unchecked_transfer(&target, resource_type, amount)?;
            creep_ref.borrow_mut().last_transfer_tick = game_tick();
            return Ok(());
        }
    }
}

/// Drops the first tick it is able to do without conflicting with another action.
pub async fn drop_when_able(creep_ref: &CreepRef, resource_type: ResourceType, amount: Option<u32>) -> Result<(), XiError> {
    loop {
        // TODO Switch to last_drop_tick after the code can handle computing whether there is enough resource this tick.
        if creep_ref.borrow().last_transfer_tick == game_tick() {
            creep_ref.borrow_mut().screeps_obj()?;
            sleep(1).await;
        } else {
            creep_ref.borrow_mut().drop(resource_type, amount)?;
            creep_ref.borrow_mut().last_transfer_tick = game_tick();
            return Ok(());
        }
    }
}