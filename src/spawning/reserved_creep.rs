use std::cell::RefCell;
use std::ops::{Deref, DerefMut};
use log::trace;
use rustc_hash::FxHashSet;
use crate::creeps::creep::Creep;
use crate::creeps::creeps::CreepRef;
use crate::a;
use crate::creeps::creep_role::CreepRole;

thread_local! {
    static RESERVED_CREEPS: RefCell<FxHashSet<(CreepRole, u32)>> = RefCell::new(FxHashSet::default());
}

fn with_reserved_creeps<F, R>(f: F) -> R
where
    F: FnOnce(&mut FxHashSet<(CreepRole, u32)>) -> R,
{
    RESERVED_CREEPS.with(|reserved_creeps| {
        let mut borrowed_reserved_creeps = reserved_creeps.borrow_mut();
        f(borrowed_reserved_creeps.deref_mut())
    })
}

pub trait MaybeReservedCreep {
    fn is_reserved(&self) -> bool;
}

/// Structure that is a wrapper around CreepRef that reserves the creep upon creation and
/// releases it to the pool of not reserved creeps when dropped.
#[derive(Debug)]
pub struct ReservedCreep {
    creep_ref: CreepRef,
}

impl ReservedCreep {
    pub fn new(creep_ref: CreepRef) -> Self {
        with_reserved_creeps(|reserved_creeps| {
            let creep = creep_ref.borrow();
            trace!("Reserving creep {}.", creep.name);
            // TODO This assertion has failed after spawning a creep just after another one suicided.
            //      xi::spawning::reserved_creep: Reserving creep upgrader36.
            //      [ERROR] xi::utils::assert: Assertion failed in src\spawning\reserved_creep.rs:41,13 in xi::spawning::reserved_creep.
            a!(!reserved_creeps.contains(&(creep.role, creep.number)));
            reserved_creeps.insert((creep.role, creep.number));
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
            reserved_creeps.contains(&(self.role, self.number))
        })
    }
}

impl Drop for ReservedCreep {
    fn drop(&mut self) {
        with_reserved_creeps(|reserved_creeps| {
            let creep = self.creep_ref.borrow();
            trace!("Dropping reservation for creep {}.", creep.name);
            a!(reserved_creeps.remove(&(creep.role, creep.number)));
        })
    }
}