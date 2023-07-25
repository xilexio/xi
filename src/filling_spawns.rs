use crate::consts::FAR_FUTURE;
use crate::game_time::game_tick;
use crate::hauling::{schedule_store, StoreRequest};
use crate::kernel::sleep::sleep;
use crate::room_state::room_states::with_room_state;
use crate::u;
use screeps::game::{get_object_by_id_typed, rooms};
use screeps::ResourceType::Energy;
use screeps::{HasPosition, HasStore, HasTypedId, ObjectId, RoomName, Transferable};
use wasm_bindgen::JsValue;

/// Keeps spawns filled by requesting haulers to fill them.
pub async fn fill_spawns(room_name: RoomName) {
    let mut structures_broadcast = u!(with_room_state(room_name, |room_state| {
        room_state.structures_broadcast.clone()
    }));

    loop {
        while structures_broadcast.check().is_none() {
            with_room_state(room_name, |room_state| {
                let room = u!(rooms().get(room_name));
                if room.energy_available() < room.energy_capacity_available() {
                    for spawn_data in room_state.spawns.iter() {
                        schedule_missing_energy_store(room_name, spawn_data.id);
                    }
                    for extension_data in room_state.extensions.iter() {
                        schedule_missing_energy_store(room_name, extension_data.id);
                    }
                }
            });
            sleep(4).await;
        }
    }
}

pub fn schedule_missing_energy_store<T>(room_name: RoomName, id: ObjectId<T>)
where
    T: HasStore + HasTypedId<T> + Transferable + From<JsValue>,
{
    let spawn = u!(get_object_by_id_typed(&id));
    let missing_energy = spawn.store().get_free_capacity(Some(Energy));
    if missing_energy > 0 {
        // The previous store request is replaced by this one.
        schedule_store(StoreRequest {
            room_name,
            target: spawn.id(),
            xy: Some(spawn.pos()),
            amount: missing_energy as u32,
            priority: 0, // TODO far away extensions less important
            preferred_tick: (game_tick(), FAR_FUTURE),
        });
    }
}
