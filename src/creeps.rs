use crate::creep::{Creep, CreepRole};
use crate::fresh_number::fresh_number_if_some;
use rustc_hash::FxHashMap;
use screeps::game;
use std::cell::RefCell;
use std::collections::hash_map::Entry;
use std::ops::DerefMut;

thread_local! {
    static CREEPS: RefCell<FxHashMap<CreepRole, FxHashMap<u32, Creep>>> = RefCell::new(FxHashMap::default());
}

fn with_creeps<F, R>(mut f: F) -> R
where
    F: FnMut(&mut FxHashMap<CreepRole, FxHashMap<u32, Creep>>) -> R,
{
    CREEPS.with(|creeps| {
        let mut borrowed_creeps = creeps.borrow_mut();
        f(borrowed_creeps.deref_mut())
    })
}

pub struct CreepManager {
    creeps: FxHashMap<String, Creep>,
}

pub fn cleanup_creeps() {
    let game_creeps = game::creeps();

    with_creeps(|creeps| {
        for creep_name in game_creeps.keys() {
            match creeps.entry(CreepRole::Craftsman) {
                Entry::Occupied(_) => {}
                Entry::Vacant(_) => {
                    // The creep is not registered in the bot. Most likely it is freshly after a reset.
                    // TODO register the creep
                }
            }
        }

        for creep_name in creeps.keys() {
            if game_creeps.get("name".to_string()).is_none() {
                // The creep is dead.
                // TODO inform its process
            }
        }
    });
}

pub fn fresh_creep_name(role: CreepRole) -> String {
    with_creeps(|creeps| {
        let creep_number = fresh_number_if_some(creeps.get(&role));
        format!("{}{}", role.creep_name_prefix(), creep_number)
    })
}
