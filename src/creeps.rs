use crate::creep::{Creep, CreepRole};
use crate::fresh_number::fresh_number_if_some;
use rustc_hash::FxHashMap;
use screeps::{game, ReturnCode, RoomName, RoomXY};
use std::cell::RefCell;
use std::ops::DerefMut;
use std::rc::Rc;
use log::{info, warn};
use regex::Regex;
use crate::kernel::sleep::sleep;
use crate::creep::CreepBody;
use crate::travel::TravelState;
use crate::u;
use crate::reserved_creep::{MaybeReservedCreep, ReservedCreep};

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
        let caps = creep_name_regex.captures(&creep_name)?;
        let role = CreepRole::from_creep_name_prefix(&caps[1])?;
        let number = caps[2].parse::<u32>().ok()?;
        Some((role, number))
    };

    // Creeps not assigned anywhere should be possible only on the first tick in the event of a restart.
    with_creeps(|creeps| {
        for creep_name in game::creeps().keys() {
            if let Some((role, number)) = parse_creep_name(&creep_name) {
                info!("Found existing unregistered {:?} creep {}. Registering it.", role, creep_name);

                let creep = Creep {
                    name: creep_name,
                    role,
                    number,
                    travel_state: TravelState::default(),
                };

                let creep_ref = Rc::new(RefCell::new(creep));

                creeps
                    .entry(role)
                    .or_insert_with(FxHashMap::default)
                    .insert(number, creep_ref.clone());
            } else {
                warn!("Could not parse role of creep {}. Killing it.", creep_name);
                let creep = u!(game::creeps().get(creep_name.clone()));
                if creep.suicide() != ReturnCode::Ok {
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
        let number = fresh_number_if_some(creeps.get(&role));
        let name = format!("{}{}", role.creep_name_prefix(), number);

        let creep = Creep {
            name,
            role,
            number,
            travel_state: TravelState::default(),
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
pub fn find_idle_creep(room_name: RoomName, role: CreepRole, body: &CreepBody, preferred_xy: Option<RoomXY>) -> Option<ReservedCreep> {
    // TODO Improve efficiency and do not return creeps that are about to expire.
    with_creeps(|creeps| {
        let role_creeps = creeps.get_mut(&role)?;
        let creep_number = role_creeps.values().find(|&creep_ref| {
            let creep = creep_ref.borrow();
            !creep.is_reserved() && creep.body().eq(body)
        })?.borrow().number;
        role_creeps.get(&creep_number).cloned().map(ReservedCreep::new)
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

fn fresh_creep_name(role: CreepRole) -> String {
    with_creeps(|creeps| {
        let creep_number = fresh_number_if_some(creeps.get(&role));
        format!("{}{}", role.creep_name_prefix(), creep_number)
    })
}
