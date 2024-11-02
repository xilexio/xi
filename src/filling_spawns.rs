use log::debug;
use rustc_hash::FxHashMap;
use crate::hauling::schedule_store;
use crate::room_state::room_states::with_room_state;
use crate::u;
use screeps::game::get_object_by_id_typed;
use screeps::ResourceType::Energy;
use screeps::{HasId, HasPosition, HasStore, ObjectId, RoomName, Transferable};
use wasm_bindgen::{JsCast, JsValue};
use crate::hauling::requests::{StoreRequest, StoreRequestId};
use crate::room_state::utils::loop_until_structures_change;

/// Keeps spawns filled by requesting haulers to fill them.
pub async fn fill_spawns(room_name: RoomName) {
    loop {
        // TODO Maybe do not drop all store requests, just the ones that changed?
        let mut spawn_store_request_ids = FxHashMap::default();
        let mut extension_store_request_ids = FxHashMap::default();

        loop_until_structures_change(room_name, 4, || {
            with_room_state(room_name, |room_state| {
                for spawn_data in room_state.spawns.iter() {
                    if let Some(request_id) = schedule_missing_energy_store(room_name, spawn_data.id) {
                        spawn_store_request_ids.insert(spawn_data.id, request_id);
                    }
                }
                for extension_data in room_state.extensions.iter() {
                    if let Some(request_id) = schedule_missing_energy_store(room_name, extension_data.id) {
                        extension_store_request_ids.insert(extension_data.id, request_id);
                    }
                }
            });

            true
        }).await;
    }
}

pub fn schedule_missing_energy_store<T>(room_name: RoomName, id: ObjectId<T>) -> Option<StoreRequestId>
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
            xy: Some(spawn.pos()),
            resource_type: Energy,
            amount: Some(missing_energy),
            priority: 0, // TODO far away extensions less important
            // preferred_tick: (game_tick(), FAR_FUTURE),
        }, None))
    } else {
        None
    }
}
