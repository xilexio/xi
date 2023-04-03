use crate::room_state::RoomState;
use screeps::RoomName;
use std::cell::RefCell;
use rustc_hash::FxHashMap;

thread_local! {
    pub static ROOM_STATES: RefCell<FxHashMap<RoomName, RoomState>> = RefCell::new(FxHashMap::default());
}

pub fn with_room_state<F, R>(room_name: RoomName, f: F) -> Option<R>
where
    F: FnOnce(&RoomState) -> R,
{
    ROOM_STATES.with(|states| states.borrow().get(&room_name).map(f))
}

pub fn replace_room_state<F, R>(room_name: RoomName, f: F) -> R
where
    F: FnOnce(&mut RoomState) -> R,
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
