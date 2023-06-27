use crate::creep::{Creep, CreepRole};
use crate::fresh_number::fresh_number_if_some;
use rustc_hash::FxHashMap;
use screeps::game;
use std::cell::RefCell;
use std::ops::DerefMut;
use std::rc::Rc;

pub type CreepRef = Rc<RefCell<Creep>>;

thread_local! {
    static CREEPS: RefCell<FxHashMap<CreepRole, FxHashMap<u32, CreepRef>>> = RefCell::new(FxHashMap::default());
}

fn with_creeps<F, R>(mut f: F) -> R
where
    F: FnMut(&mut FxHashMap<CreepRole, FxHashMap<u32, CreepRef>>) -> R,
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
        // for creep_name in game_creeps.keys() {
        //     match creeps.entry(CreepRole::Craftsman) {
        //         Entry::Occupied(_) => {}
        //         Entry::Vacant(_) => {
        //             // The creep is not registered in the bot. Most likely it is freshly after a reset.
        //             // TODO register the creep
        //         }
        //     }
        // }

        for role_creeps in creeps.values() {
            for creep in role_creeps.values() {
                if game_creeps.get(creep.borrow().name.clone()).is_none() {
                    // The creep is dead.
                    // TODO inform its process
                }
            }
        }
    });
}

pub fn register_creep(role: CreepRole) -> CreepRef {
    with_creeps(|creeps| {
        let number = fresh_number_if_some(creeps.get(&role));
        let name = format!("{}{}", role.creep_name_prefix(), number);

        let creep = Creep {
            name,
            role,
            number,
        };

        let creep_ref = Rc::new(RefCell::new(creep));

        creeps.entry(role).or_insert_with(|| FxHashMap::default()).insert(number, creep_ref.clone());

        creep_ref
    })
}

fn fresh_creep_name(role: CreepRole) -> String {
    with_creeps(|creeps| {
        let creep_number = fresh_number_if_some(creeps.get(&role));
        format!("{}{}", role.creep_name_prefix(), creep_number)
    })
}