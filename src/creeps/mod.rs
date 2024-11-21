use crate::creeps::creep::Creep;
use crate::fresh_number::fresh_number_if_some;
use crate::geometry::room_xy::RoomXYUtils;
use crate::kernel::sleep::sleep;
use crate::spawning::reserved_creep::{MaybeReservedCreep, ReservedCreep};
use crate::u;
use creep_body::CreepBody;
use log::{info, warn};
use regex::Regex;
use rustc_hash::FxHashMap;
use screeps::{game, RoomName, RoomXY};
use std::cell::RefCell;
use std::ops::DerefMut;
use std::rc::Rc;
use creep_role::CreepRole;

pub mod creep;
pub mod actions;
pub mod creep_body;
pub mod creep_role;

pub type CreepRef = Rc<RefCell<Creep>>;

thread_local! {
    static CREEPS: RefCell<FxHashMap<CreepRole, FxHashMap<u32, CreepRef>>> = RefCell::new(FxHashMap::default());
}

fn with_creeps<F, R>(f: F) -> R
where
    F: FnOnce(&mut FxHashMap<CreepRole, FxHashMap<u32, CreepRef>>) -> R,
{
    CREEPS.with(|creeps| {
        let mut borrowed_creeps = creeps.borrow_mut();
        f(borrowed_creeps.deref_mut())
    })
}

pub async fn cleanup_creeps() {
    let creep_name_regex = u!(Regex::new(r"^([a-z]+)([1-9][0-9]*)$"));

    let parse_creep_name = |creep_name: &str| -> Option<(CreepRole, u32)> {
        let caps = creep_name_regex.captures(creep_name)?;
        let role = CreepRole::from_creep_name_prefix(&caps[1])?;
        let number = caps[2].parse::<u32>().ok()?;
        Some((role, number))
    };

    // Creeps not assigned anywhere should be possible only on the first tick in the event of a restart.
    with_creeps(|creeps| {
        for creep_name in game::creeps().keys() {
            if let Some((role, number)) = parse_creep_name(&creep_name) {
                info!(
                    "Found existing unregistered {} creep {}. Registering it.",
                    role, creep_name
                );

                let creep = Creep {
                    name: creep_name,
                    role,
                    number,
                    ..Creep::default()
                };

                let creep_ref = Rc::new(RefCell::new(creep));

                creeps
                    .entry(role)
                    .or_insert_with(FxHashMap::default)
                    .insert(number, creep_ref.clone());
            } else {
                warn!("Could not parse role of creep {}. Killing it.", creep_name);
                let creep = u!(game::creeps().get(creep_name.clone()));
                if let Err(_) = creep.suicide() {
                    warn!("Failed to kill on creep {}.", creep_name);
                }
            }
        }
    });

    loop {
        let game_creeps = game::creeps();

        with_creeps(|creeps| {
            for role_creeps in creeps.values() {
                for creep in role_creeps.values() {
                    if game_creeps.get(creep.borrow().name.clone()).is_none() {
                        // The creep is dead.
                        // TODO inform its process
                        creep.borrow_mut().dead = true;
                    }
                }
            }
        });

        sleep(1).await;
    }
}

/// Registers a new creep within the creeps module. May be called on the tick the creep is spawned
/// after `cleanup_creeps`.
pub fn register_creep(role: CreepRole) -> CreepRef {
    with_creeps(|creeps| {
        // Note that it may not overlap with existing creeps after a reset, so UId is insufficient.
        let number = fresh_number_if_some(creeps.get(&role));
        let name = format!("{}{}", role.creep_name_prefix(), number);

        let creep = Creep {
            name,
            role,
            number,
            ..Creep::default()
        };

        let creep_ref = Rc::new(RefCell::new(creep));

        creeps
            .entry(role)
            .or_insert_with(FxHashMap::default)
            .insert(number, creep_ref.clone());

        creep_ref
    })
}

/// Finds a creep free to be assigned to any task.
/// Any alive creep can be returned, even a currently spawning one.
pub fn find_idle_creep(
    room_name: RoomName,
    role: CreepRole,
    body: &CreepBody,
    preferred_xy: Option<RoomXY>,
) -> Option<ReservedCreep> {
    // TODO Only return creeps assigned to given room.
    // TODO Improve efficiency and do not return creeps that are about to expire.
    with_creeps(|creeps| {
        let role_creeps = creeps.get_mut(&role)?;
        let creep_ref_check = |&creep_ref: &&CreepRef| {
            let mut creep = creep_ref.borrow_mut();
            // TODO Why would there be a dead creep there?
            !creep.dead && !creep.is_reserved() && u!(creep.body()).eq(body)
        };
        let creep_ref = if let Some(preferred_xy) = preferred_xy {
            // TODO Check if this is guaranteed to have only alive creeps.
            role_creeps
                .values()
                .filter(creep_ref_check)
                .min_by_key(|&creep_ref| u!(creep_ref.borrow_mut().pos()).xy().dist(preferred_xy))?
        } else {
            role_creeps.values().find(creep_ref_check)?
        };
        Some(ReservedCreep::new(creep_ref.clone()))
    })
}

pub fn for_each_creep<F>(mut f: F)
where
    F: FnMut(&CreepRef),
{
    with_creeps(|creeps| {
        for (_, role_creeps) in creeps.iter() {
            for (_, creep_ref) in role_creeps.iter() {
                f(creep_ref);
            }
        }
    });
}
