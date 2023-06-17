use crate::room_state::room_states::{with_room_states, RoomStates};
use crate::u;
use js_sys::JsString;
use log::{error, trace};
use screeps::{raw_memory, MEMORY_SIZE_LIMIT};
use serde::{Deserialize, Serialize};
use serde_with::{serde_as, FromInto, PickFirst};

/// References to parts of the global state to avoid copying them.
#[derive(Serialize)]
struct GlobalStateSer<'a> {
    room_states: &'a RoomStates,
}

type OldRoomStates = RoomStates;

/// A structure holding parts of the global state.
/// Serialization of each part combines `PickFirst` and `FromInto` so that a migration may be written after its format
/// change. The migration consists of copying the structure with the old format to the type marking old version of given
/// part and implementing `From` to convert it to the new version. After the migration has been applied, the type should
/// be reverted back to the current one.
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
            trace!("{}", serialized_global_state);
            // TODO Keep in mind that base32768 is an option to increase the capacity of memory almost 2x.
            let len = serialized_global_state.len() as u32;
            raw_memory::set(&JsString::from(serialized_global_state));
            trace!(
                "Serialized the global state. Using approximately {}B ({}%).",
                len,
                len / MEMORY_SIZE_LIMIT
            );
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
        Ok(()) => {
            trace!("Deserialized the global state.");
        }
        Err(e) => {
            error!("Failed to deserialize global state: {:?}.", e);
        }
    }
}

/// Deserializes the global state from a string.
fn deserialize_global_state(raw_memory_str: &str) -> Result<(), serde_json::Error> {
    let deserialized_global_state: GlobalStateDe = serde_json::from_str(raw_memory_str)?;
    with_room_states(move |room_states| {
        let GlobalStateDe {
            room_states: room_states_de,
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
