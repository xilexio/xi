use rustc_hash::FxHashMap;
use screeps::{game, RoomName, RoomXY};
use log::{info, warn};
use std::rc::Rc;
use std::cell::RefCell;
use std::ops::DerefMut;
use regex::Regex;
use crate::creeps::creep::Creep;
use crate::creeps::creep_body::CreepBody;
use crate::creeps::creep_role::CreepRole;
use crate::fresh_number::fresh_number_if_some;
use crate::geometry::room_xy::RoomXYUtils;
use crate::kernel::sleep::sleep;
use crate::spawning::reserved_creep::{MaybeReservedCreep, ReservedCreep};
use crate::travel::traffic::register_creep_pos;
use crate::u;
use crate::utils::result_utils::ResultUtils;

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
                
                let creep_obj = u!(game::creeps().get(creep_name.clone()));

                let creep = Creep::new(
                    creep_name,
                    None,
                    role,
                    number,
                    creep_obj.body().into()
                );

                let creep_ref = Rc::new(RefCell::new(creep));

                creeps
                    .entry(role)
                    .or_default()
                    .insert(number, creep_ref.clone());
            } else {
                warn!("Could not parse role of creep {}. Killing it.", creep_name);
                let creep = u!(game::creeps().get(creep_name.clone()));
                creep
                    .suicide()
                    .warn_if_err(&format!("Failed to kill on creep {}.", creep_name));
            }
        }
    });

    loop {
        let game_creeps = game::creeps();

        with_creeps(|creeps| {
            for (_, role_creeps) in creeps.iter_mut() {
                role_creeps.retain(|_, creep_ref| {
                    if game_creeps.get(creep_ref.borrow().name.clone()).is_none() {
                        // The creep is dead.
                        // TODO inform its process
                        creep_ref.borrow_mut().dead = true;
                        false
                    } else {
                        register_creep_pos(creep_ref);
                        true
                    }
                });
            }
        });

        sleep(1).await;
    }
}

/// Registers a new creep within the creeps module. May be called on the tick the creep is spawned
/// after `cleanup_creeps`.
pub fn register_creep(role: CreepRole, body: CreepBody) -> CreepRef {
    with_creeps(|creeps| {
        // Note that it may not overlap with existing creeps after a reset, so UId is insufficient.
        let number = fresh_number_if_some(creeps.get(&role));
        let name = format!("{}{}", role.creep_name_prefix(), number);

        let creep = Creep::new(
            name,
            None,
            role,
            number,
            body
        );

        let creep_ref = Rc::new(RefCell::new(creep));

        creeps
            .entry(role)
            .or_default()
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
            let creep = creep_ref.borrow_mut();
            // TODO Why would there be a dead creep there?
            !creep.dead && !creep.is_reserved() && creep.body.eq(body)
        };
        let creep_ref = if let Some(preferred_xy) = preferred_xy {
            // TODO Check if this is guaranteed to have only alive creeps.
            role_creeps
                .values()
                .filter(creep_ref_check)
                .min_by_key(|&creep_ref| creep_ref.borrow_mut().travel_state.pos.xy().dist(preferred_xy))?
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