use log::debug;
use rustc_hash::FxHashMap;
use crate::hauling::issuing_requests::schedule_store;
use crate::room_states::room_states::with_room_state;
use crate::u;
use screeps::game::get_object_by_id_typed;
use screeps::ResourceType::Energy;
use screeps::{HasId, HasPosition, HasStore, ObjectId, RoomName, Transferable};
use wasm_bindgen::{JsCast, JsValue};
use crate::hauling::issuing_requests::{StoreRequest, StoreRequestHandle};
use crate::hauling::issuing_requests::RequestAmountChange::NoChange;
use crate::room_states::utils::loop_until_structures_change;
use crate::utils::priority::Priority;

/// Keeps spawns filled by requesting haulers to fill them.
pub async fn fill_spawns(room_name: RoomName) {
    loop {
        let mut spawn_store_request_handles = FxHashMap::default();
        let mut extension_store_request_handles = FxHashMap::default();

        // TODO Maybe don't drop all store requests on change, just the ones that changed?
        loop_until_structures_change(room_name, 4, || {
            with_room_state(room_name, |room_state| {
                // TODO When the request is already underway, don't schedule another one.
                for spawn_data in room_state.spawns.iter() {
                    let handle = schedule_missing_energy_store(
                        room_name,
                        spawn_data.id,
                        spawn_store_request_handles.remove(&spawn_data.id)
                    );
                    if let Some(handle) = handle {
                        spawn_store_request_handles.insert(spawn_data.id, handle);
                    }
                }
                for extension_data in room_state.extensions.iter() {
                    let handle = schedule_missing_energy_store(
                        room_name,
                        extension_data.id,
                        extension_store_request_handles.remove(&extension_data.id)
                    );
                    if let Some(handle) = handle {
                        extension_store_request_handles.insert(extension_data.id, handle);
                    }
                }
            });

            true
        }).await;
    }
}

pub fn schedule_missing_energy_store<T>(
    room_name: RoomName,
    id: ObjectId<T>,
    replaced_request_handle: Option<StoreRequestHandle>
) -> Option<StoreRequestHandle>
where
    T: HasStore + HasId + Transferable + From<JsValue> + JsCast,
{
    let spawn = u!(get_object_by_id_typed(&id));
    let missing_energy = spawn.store().get_free_capacity(Some(Energy)) as u32;
    if missing_energy > 0 {
        debug!("Scheduling haul of missing {missing_energy} energy for {id} in {room_name}.");
        // The previous store request is replaced by this one.
        Some(schedule_store(StoreRequest {
            room_name,
            target: spawn.id(),
            pos: Some(spawn.pos()),
            resource_type: Energy,
            amount: missing_energy,
            amount_change: NoChange,
            priority: Priority(1), // TODO far away extensions less important
            // preferred_tick: (game_tick(), FAR_FUTURE),
        }, replaced_request_handle))
    } else {
        None
    }
}
