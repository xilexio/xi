use log::debug;
use rustc_hash::FxHashMap;
use crate::room_states::room_states::with_room_state;
use crate::u;
use screeps::game::get_object_by_id_typed;
use screeps::{HasId, HasPosition, HasStore, ObjectId, ResourceType, RoomName, Transferable};
use wasm_bindgen::{JsCast, JsValue};
use crate::hauling::requests::{HaulRequest, HaulRequestHandle};
use crate::hauling::requests::HaulRequestKind::StoreRequest;
use crate::hauling::scheduling_hauls::schedule_haul;
use crate::hauling::transfers::get_free_capacity;
use crate::hauling::transfers::TransferStage::AfterAllTransfers;
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
                // TODO Make use of current_tick_change to adjust required energy.
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
    replaced_request_handle: Option<HaulRequestHandle>
) -> Option<HaulRequestHandle>
where
    T: HasStore + HasId + Transferable + From<JsValue> + JsCast,
{
    let obj = u!(get_object_by_id_typed(&id));
    let missing_energy = u!(get_free_capacity(id, Some(ResourceType::Energy), AfterAllTransfers));
    if missing_energy > 0 {
        debug!("Scheduling haul of missing {missing_energy} energy for {id} in {room_name}.");
        // The previous store request is replaced by this one.
        let mut store_request = HaulRequest::new(
            StoreRequest,
            room_name,
            ResourceType::Energy,
            id,
            obj.pos()
        );
        store_request.amount = missing_energy as u32;
        // TODO Far away extensions less important.
        store_request.priority = Priority(1);
        Some(schedule_haul(store_request, replaced_request_handle))
    } else {
        None
    }
}
