use crate::room_state::{RoomDesignation, RoomState};
use rustc_hash::FxHashMap;
use screeps::{raw_memory, RoomName};
use std::cell::RefCell;
use std::ops::Deref;
use js_sys::JsString;
use log::error;

thread_local! {
    static ROOM_STATES: RefCell<FxHashMap<RoomName, RoomState>> = RefCell::new(FxHashMap::default());
}

pub fn with_room_state<F, R>(room_name: RoomName, f: F) -> Option<R>
where
    F: Fn(&RoomState) -> R,
{
    ROOM_STATES.with(|states| states.borrow().get(&room_name).map(f))
}

pub fn replace_room_state<F, R>(room_name: RoomName, mut f: F) -> R
where
    F: FnMut(&mut RoomState) -> R,
{
    ROOM_STATES.with(|states| {
        let mut s = states.borrow_mut();
        match s.get_mut(&room_name) {
            Some(rs) => f(rs),
            None => {
                let mut room_state = RoomState::new(room_name);
                let result = f(&mut room_state);
                s.insert(room_name, room_state);
                result
            }
        }
    })
}

pub fn for_each_room<F>(mut f: F)
where
    F: FnMut(RoomName, &mut RoomState),
{
    ROOM_STATES.with(|states| {
        for (&room_name, room_state) in states.borrow_mut().iter_mut() {
            f(room_name, room_state);
        }
    });
}

pub fn for_each_owned_room<F>(mut f: F)
where
    F: FnMut(RoomName, &mut RoomState),
{
    for_each_room(|room_name, room_state| {
        if room_state.designation == RoomDesignation::Owned {
            f(room_name, room_state);
        }
    });
}

pub fn save_room_states() {
    ROOM_STATES.with(|states| {
        match serde_json::to_string(states.borrow().deref()) {
            Ok(serlialized) => {
                raw_memory::set(&JsString::from(serlialized));
            }
            Err(e) => {
                error!("Failed to serialize room states: {:?}.", e);
            }
        }
    });
}

pub fn load_room_states() {

}