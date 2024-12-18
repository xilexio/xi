use rustc_hash::FxHashMap;
use screeps::RoomName;
use std::cell::RefCell;
use std::ops::DerefMut;
use crate::room_states::room_state::{RoomDesignation, RoomState};
#[cfg(test)]
use crate::room_states::room_state::empty_unowned_room_state;

pub type RoomStates = FxHashMap<RoomName, RoomState>;

thread_local! {
    static ROOM_STATES: RefCell<RoomStates> = RefCell::new(FxHashMap::default());
}

pub fn with_room_states<F, R>(f: F) -> R
where
    F: FnOnce(&mut RoomStates) -> R,
{
    ROOM_STATES.with(|states| f(states.borrow_mut().deref_mut()))
}

pub fn with_room_state<F, R>(room_name: RoomName, f: F) -> Option<R>
where
    F: FnOnce(&mut RoomState) -> R,
{
    ROOM_STATES.with(|states| states.borrow_mut().get_mut(&room_name).map(f))
}

pub fn map_and_replace_room_state<F, R>(room_name: RoomName, mut f: F) -> R
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

#[cfg(test)]
pub fn test_room_states() -> RoomStates {
    let room_states = [
        empty_unowned_room_state(),
    ];
    
    room_states.into_iter().map(|room_state| (room_state.room_name, room_state)).collect()
}