use std::cell::RefCell;
use std::cmp::max;
use rustc_hash::FxHashMap;
use screeps::{HasStore, MaybeHasId, ObjectId, RawObjectId, ResourceType};
use screeps::game::get_object_by_id_typed;
use wasm_bindgen::JsCast;
use crate::errors::XiError;
use crate::hauling::transfers::TransferStage::*;
use crate::utils::single_tick_cache::SingleTickCache;

#[derive(Debug, Default, Clone, Eq, PartialEq)]
struct ResourceTransfers {
    incoming: u32,
    outgoing: u32,
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum TransferStage {
    BeforeAnyTransfers,
    AfterIncomingTransfers,
    AfterOutgoingTransfers,
    AfterAllTransfers,
}

thread_local! {
    static TRANSFERS: RefCell<SingleTickCache<FxHashMap<RawObjectId, FxHashMap<ResourceType, ResourceTransfers>>>> = RefCell::new(SingleTickCache::default());
}

fn with_transfers<F, R>(f: F) -> R
where
    F: FnOnce(&mut FxHashMap<RawObjectId, FxHashMap<ResourceType, ResourceTransfers>>) -> R,
{
    TRANSFERS.with(|transfers| {
        let mut borrowed_transfers = transfers.borrow_mut();
        let transfers = borrowed_transfers.get_or_insert_with(FxHashMap::default);
        f(transfers)
    })
}

pub fn register_transfer(object_id: RawObjectId, resource_type: ResourceType, change: i32) {
    with_transfers(|transfers| {
        let resource_transfers = transfers
            .entry(object_id)
            .or_default()
            .entry(resource_type)
            .or_default();
        if change > 0 {
            resource_transfers.incoming += change as u32;
        } else {
            resource_transfers.outgoing += (-change) as u32;
        }
    });
}

fn current_tick_transfers(object_id: RawObjectId, resource_type: ResourceType) -> ResourceTransfers {
    let result = with_transfers(|transfers| {
        Some(transfers.get(&object_id)?.get(&resource_type)?.clone())
    });
    result.unwrap_or_default()
}

fn with_current_tick_all_transfers<F, R>(object_id: RawObjectId, f: F) -> Option<R>
where
    F: FnOnce(&FxHashMap<ResourceType, ResourceTransfers>) -> R
{
    with_transfers(|transfers| {
        Some(f(transfers.get(&object_id)?))
    })
}

/// Gets the amount of given resource available in given object in the current tick.
/// Depending on `transfer_stage`, the amount is either before or after incoming or outgoing
/// transfers.
pub fn get_used_capacity<T>(object_id: ObjectId<T>, resource_type: Option<ResourceType>, transfer_stage: TransferStage) -> Result<u32, XiError>
where
    T: HasStore + MaybeHasId + JsCast,
{
    let object = get_object_by_id_typed(&object_id).ok_or(XiError::ObjectDoesNotExist)?;
    Ok(get_used_capacity_with_object(&object, object_id.into(), resource_type, transfer_stage))
}

pub fn get_used_capacity_with_object<T>(object: &T, object_id: RawObjectId, resource_type: Option<ResourceType>, transfer_stage: TransferStage) -> u32
where
    T: ?Sized + HasStore,
{
    let mut amount = object.store().get_used_capacity(resource_type) as i32;
    if transfer_stage != BeforeAnyTransfers {
        if let Some(resource_type) = resource_type {
            let transfers = current_tick_transfers(object_id.into(), resource_type);
            if transfer_stage == AfterOutgoingTransfers || transfer_stage == AfterAllTransfers {
                amount -= transfers.outgoing as i32;
            }
            if transfer_stage == AfterIncomingTransfers || transfer_stage == AfterAllTransfers {
                amount += transfers.incoming as i32;
            }
        } else {
            with_current_tick_all_transfers(object_id.into(), |all_transfers| {
                for transfers in all_transfers.values() {
                    if transfer_stage == AfterOutgoingTransfers || transfer_stage == AfterAllTransfers {
                        amount -= transfers.outgoing as i32;
                    }
                    if transfer_stage == AfterIncomingTransfers || transfer_stage == AfterAllTransfers {
                        amount += transfers.incoming as i32;
                    }
                }
            });
        }
    }
    max(0, amount) as u32
}

pub fn get_used_capacities<T>(
    object_id: ObjectId<T>,
    transfer_stage: TransferStage
) -> Result<FxHashMap<ResourceType, u32>, XiError>
where
    T: HasStore + MaybeHasId + JsCast,
{
    let object = get_object_by_id_typed(&object_id).ok_or(XiError::ObjectDoesNotExist)?;
    Ok(get_used_capacities_with_object(&object, object_id.into(), transfer_stage))
}

pub fn get_used_capacities_with_object<T>(
    object: &T,
    object_id: RawObjectId,
    transfer_stage: TransferStage
) -> FxHashMap<ResourceType, u32>
where
    T: ?Sized + HasStore,
{
    let mut amounts = FxHashMap::default();
    let store = object.store();
    with_current_tick_all_transfers(object_id.into(), |all_transfers| {
        for resource_type in store.store_types() {
            let mut amount = store.get_used_capacity(Some(resource_type)) as i32;
            if let Some(transfers) = all_transfers.get(&resource_type) {
                if transfer_stage == AfterOutgoingTransfers || transfer_stage == AfterAllTransfers {
                    amount -= transfers.outgoing as i32;
                }
                if transfer_stage == AfterIncomingTransfers || transfer_stage == AfterAllTransfers {
                    amount += transfers.incoming as i32;
                }
            }
            amounts.insert(resource_type, max(0, amount) as u32);
        }
    }).unwrap_or_else(|| {
        for resource_type in store.store_types() {
            amounts.insert(resource_type, store.get_used_capacity(Some(resource_type)));
        }
    });
    amounts
}

/// Gets the amount of free space available for given resource available in given object in the
/// current tick. For specialized stores, adding one resource does not decrease the free space
/// for another (e.g., lab, power spawn).
/// Depending on `transfer_stage`, the amount is either before or after incoming or outgoing
/// transfers.
pub fn get_free_capacity<T>(object_id: ObjectId<T>, resource_type: Option<ResourceType>, transfer_stage: TransferStage) -> Result<u32, XiError>
where
    T: HasStore + MaybeHasId + JsCast,
{
    let object = get_object_by_id_typed(&object_id).ok_or(XiError::ObjectDoesNotExist)?;
    Ok(get_free_capacity_with_object(&object, object_id.into(), resource_type, transfer_stage))
}

pub fn get_free_capacity_with_object<T>(object: &T, object_id: RawObjectId, resource_type: Option<ResourceType>, transfer_stage: TransferStage) -> u32
where
    T: ?Sized + HasStore,
{
    // A generic store will have positive generic capacity.
    if object.store().get_capacity(None) == 0 {
        if let Some(resource_type) = resource_type {
            // In a specialized store, only the given resource counts towards used capacity.
            let mut free_capacity = object.store().get_free_capacity(Some(resource_type));
            let transfers = current_tick_transfers(object_id, resource_type);
            if transfer_stage == AfterOutgoingTransfers || transfer_stage == AfterAllTransfers {
                free_capacity += transfers.outgoing as i32;
            }
            if transfer_stage == AfterIncomingTransfers || transfer_stage == AfterAllTransfers {
                free_capacity -= transfers.incoming as i32;
            }
            max(0, free_capacity) as u32
        } else {
            0
        }
    } else {
        // In a generic store, everything counts towards used capacity.
        let mut free_capacity = object.store().get_free_capacity(None);
        with_current_tick_all_transfers(object_id.into(), |all_transfers| {
            for transfers in all_transfers.values() {
                if transfer_stage == AfterOutgoingTransfers || transfer_stage == AfterAllTransfers {
                    free_capacity += transfers.outgoing as i32;
                }
                if transfer_stage == AfterIncomingTransfers || transfer_stage == AfterAllTransfers {
                    free_capacity -= transfers.incoming as i32;
                }
            }
        });
        max(0, free_capacity) as u32
    }
}