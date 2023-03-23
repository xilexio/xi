use crate::room_state::RoomState;
use screeps::RoomName;
use std::collections::HashMap;
use std::cell::RefCell;

thread_local! {
    pub static ROOM_STATES: RefCell<HashMap<RoomName, RoomState>> = RefCell::new(HashMap::new());
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
