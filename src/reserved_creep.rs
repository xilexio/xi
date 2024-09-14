use std::cell::RefCell;
use std::ops::{Deref, DerefMut};
use log::debug;
use rustc_hash::{FxHashMap, FxHashSet};
use crate::creep::{Creep, CreepRole};
use crate::creeps::CreepRef;
use crate::{a, u};

thread_local! {
    // TODO Why not make this number globally unique?
    static RESERVED_CREEPS: RefCell<FxHashMap<CreepRole, FxHashSet<u32>>> = RefCell::new(FxHashMap::default());
}

fn with_reserved_creeps<F, R>(f: F) -> R
where
    F: FnOnce(&mut FxHashMap<CreepRole, FxHashSet<u32>>) -> R,
{
    RESERVED_CREEPS.with(|reserved_creeps| {
        let mut borrowed_reserved_creeps = reserved_creeps.borrow_mut();
        f(borrowed_reserved_creeps.deref_mut())
    })
}

pub trait MaybeReservedCreep {
    fn is_reserved(&self) -> bool;
}

#[derive(Debug)]
pub struct ReservedCreep {
    creep_ref: CreepRef,
}

impl ReservedCreep {
    pub fn new(creep_ref: CreepRef) -> Self {
        with_reserved_creeps(|reserved_creeps| {
            let creep = creep_ref.borrow();
            debug!("Reserving creep {}.", creep.name);
            let entry = reserved_creeps.entry(creep.role).or_default();
            a!(!entry.contains(&creep.number));
            entry.insert(creep.number);
        });

        ReservedCreep {
            creep_ref
        }
    }

    pub fn as_ref(&self) -> CreepRef {
        self.creep_ref.clone()
    }
}

impl Deref for ReservedCreep {
    type Target = RefCell<Creep>;

    fn deref(&self) -> &Self::Target {
        self.creep_ref.as_ref()
    }
}

impl MaybeReservedCreep for Creep {
    fn is_reserved(&self) -> bool {
        with_reserved_creeps(|reserved_creeps| {
            if let Some(role_creeps) = reserved_creeps.get(&self.role) {
                role_creeps.contains(&self.number)
            } else {
                false
            }
        })
    }
}

impl Drop for ReservedCreep {
    fn drop(&mut self) {
        with_reserved_creeps(|reserved_creeps| {
            let creep = self.creep_ref.borrow();
            debug!("Dropping reservation for creep {}.", creep.name);
            a!(u!(reserved_creeps.get_mut(&creep.role)).remove(&creep.number));
        })
    }
}