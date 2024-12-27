use std::cell::RefCell;
use std::ops::{Deref, DerefMut};
use log::{debug, trace, warn};
use rustc_hash::FxHashMap;
use screeps::{RoomName, RoomXY};
use crate::creeps::creep::Creep;
use crate::creeps::creeps::CreepRef;
use crate::{a, u};
use crate::creeps::creep_role::CreepRole;
use crate::geometry::room_xy::RoomXYUtils;
use crate::travel::nearest_room::find_nearest_owned_room;

// TODO Remove in cleanup instead of garbage collecting. This will also simplify finding.
// TODO Debug print unassigned creeps.
thread_local! {
    /// A collection of unassigned creeps, i.e., ones that are not reserved. May include dead
    /// creeps, which are garbage collected when finding unassigned creeps.
    static UNASSIGNED_CREEPS: RefCell<FxHashMap<RoomName, FxHashMap<CreepRole, FxHashMap<u32, CreepRef>>>> = RefCell::new(FxHashMap::default());
}

pub fn with_unassigned_creeps<F, R>(f: F) -> R
where
    F: FnOnce(&mut FxHashMap<RoomName, FxHashMap<CreepRole, FxHashMap<u32, CreepRef>>>) -> R,
{
    UNASSIGNED_CREEPS.with(|reserved_creeps| {
        let mut borrowed_reserved_creeps = reserved_creeps.borrow_mut();
        f(borrowed_reserved_creeps.deref_mut())
    })
}

pub trait MaybeReserved {
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

impl Drop for ReservedCreep {
    fn drop(&mut self) {
        with_unassigned_creeps(|unassigned_creeps| {
            let creep = self.creep_ref.borrow();
            if !creep.dead {
                register_unassigned_creep(unassigned_creeps, &self.creep_ref);
            } else {
                trace!("Dropping reservation for dead creep {}.", creep.name);
            }
        })
    }
}

pub fn register_unassigned_creep(unassigned_creeps: &mut FxHashMap<RoomName, FxHashMap<CreepRole, FxHashMap<u32, CreepRef>>>, creep_ref: &CreepRef) {
    let creep = creep_ref.borrow();
    if let Some(room_name) = find_nearest_owned_room(creep.travel_state.pos.room_name(), 0) {
        debug!("Registering unregistered creep {} as unassigned in room {}.", creep.name, room_name);
        let previous_data = unassigned_creeps
            .entry(room_name)
            .or_default()
            .entry(creep.role)
            .or_default()
            .insert(creep.number, creep_ref.clone());
        a!(previous_data.is_none());
    } else {
        warn!("No owned room remaining.");
    }
}

/// Finds an unreserved creep with given role. Any alive creep can be returned, even a currently
/// spawning one.
// TODO Option with min_ttl.
pub fn find_unassigned_creep(
    room_name: RoomName,
    role: CreepRole,
    preferred_xy: Option<RoomXY>,
) -> Option<ReservedCreep> {
    with_unassigned_creeps(|creeps| {
        let role_creeps = creeps.get_mut(&room_name)?.get_mut(&role)?;
        while !role_creeps.is_empty() {
            let creep_number = if let Some(preferred_xy) = preferred_xy {
                let min_dist_creep_data = role_creeps
                    .iter()
                    .min_by_key(|(creep_number, creep_ref)| {
                        creep_ref.borrow_mut().travel_state.pos.xy().dist(preferred_xy)
                    });
                *u!(min_dist_creep_data).0
            } else {
                *u!(role_creeps.keys().next())
            };
            let creep_ref = u!(role_creeps.remove(&creep_number));
            if creep_ref.borrow().dead {
                continue;
            }
            return Some(ReservedCreep::new(creep_ref.clone()));
        }
        None
    })
}