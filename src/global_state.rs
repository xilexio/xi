use crate::room_state::room_states::{with_room_states, RoomStates};
use crate::u;
use js_sys::JsString;
use log::{error, trace};
use screeps::raw_memory;
use serde::{Deserialize, Serialize};
use serde_with::{serde_as, PickFirst, FromInto};

#[derive(Serialize)]
struct GlobalStateSer<'a> {
    room_states: &'a RoomStates,
}

type OldRoomStates = RoomStates;

#[serde_as]
#[derive(Deserialize)]
struct GlobalStateDe {
    #[serde_as(as = "PickFirst<(_, FromInto<OldRoomStates>)>")]
    room_states: RoomStates,
}

/// Saves the serialized global state into Memory.
pub fn save_global_state() {
    match serialize_global_state() {
        Ok(serialized_global_state) => {
            raw_memory::set(&JsString::from(serialized_global_state));
            trace!("Serialized the global state.");
        }
        Err(e) => {
            error!("Failed to serialize global state: {:?}.", e);
        }
    }
}

/// Serializes the global state into a string.
fn serialize_global_state() -> Result<String, serde_json::Error> {
    with_room_states(|room_states| {
        let global_state = GlobalStateSer { room_states };
        serde_json::to_string(&global_state)
    })
}

/// Loads and deserializes the global state from Memory.
pub fn load_global_state() {
    let raw_memory_str = u!(raw_memory::get().as_string());
    match deserialize_global_state(&raw_memory_str) {
        Ok(global_state) => {
            trace!("Deserialized the global state.");
        }
        Err(e) => {
            error!("Failed to deserialize global state: {:?}.", e);
        }
    }
}

/// Deserializes the global state from a string.
fn deserialize_global_state(raw_memory_str: &String) -> Result<(), serde_json::Error> {
    let deserialized_global_state: GlobalStateDe = serde_json::from_str(raw_memory_str)?;
    with_room_states(move |room_states| {
        let GlobalStateDe {
                room_states: room_states_de
            } = deserialized_global_state;
        {
            *room_states = room_states_de;
        }
    });
    Ok(())
}

#[cfg(test)]
mod tests {
    use crate::global_state::{deserialize_global_state, serialize_global_state};

    #[test]
    fn serialize_and_deserialize_global_state() {
        let serialized_global_state = serialize_global_state().unwrap();
        deserialize_global_state(&serialized_global_state).unwrap();
    }
}