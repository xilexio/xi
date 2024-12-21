use std::cell::RefCell;
use rustc_hash::{FxHashMap, FxHashSet};
use screeps::{HasPosition, ObjectId, Position};
use std::collections::hash_map::Entry;
use std::hash::Hash;
use std::iter::zip;
use std::rc::Rc;
use enum_iterator::all;
use log::warn;
use crate::geometry::position_utils::PositionUtils;
use crate::kernel::sleep::sleep;
use crate::room_states::room_states::{with_room_states, RoomStates};
use crate::{a, local_debug, u};
use crate::algorithms::matrix_common::MatrixCommon;
use crate::algorithms::min_cost_weighted_matching::min_cost_weighted_matching;
use crate::algorithms::room_matrix_slice::RoomMatrixSlice;
use crate::algorithms::weighted_distance_matrix::{obstacle_cost, weighted_distance_matrix};
use crate::creeps::creeps::{for_each_creep, CreepRef};
use crate::creeps::generic_creep::GenericCreep;
use crate::geometry::grid_direction::{direction_to_offset, GridDirection};
use crate::geometry::rect::{ball, Rect};
use crate::geometry::room_xy::RoomXYUtils;
use crate::travel::surface::Surface;
use crate::travel::travel::find_path;
use crate::utils::result_utils::ResultUtils;

const DEBUG: bool = true;

enum RepathData {
    Fatigued,
    Adjusted {
        distance_matrix: RoomMatrixSlice<u32>,
        target_rect: Rect,
        path_reused: bool
    },
}

pub fn register_creep_pos(creep_ref: &CreepRef) {
    let mut creep = creep_ref.borrow_mut();
    let creep_pos = u!(creep.screeps_obj()).pos();
    creep.travel_state.pos = creep_pos;
    
    let mut repath_required = false;
    if let Some(&expected_pos) = creep.travel_state.path.last() {
        if creep.travel_state.pos == expected_pos {
            creep.travel_state.path.pop();

            if let Some(&next_pos) = creep.travel_state.path.last() {
                let next_pos_dist = next_pos.get_range_to(creep.travel_state.pos);
                if next_pos_dist != 1 {
                    warn!(
                        "Creep {} is {} tiles from the next position on the path, {}.",
                        creep.name, next_pos_dist, next_pos.f()
                    );
                    repath_required = true;
                }
            } else if creep.travel_state.pos.xy().is_on_boundary() {
                // Multi-room travel is split by rooms and has separate repathing after reaching
                // next room.
                // TODO Remove after multi-room pathing is implemented.
                repath_required = true;
            }
        } else {
            // Sometimes the creep may fail to move somewhere as a result of external interference.
            local_debug!(
                "Creep {} failed to move from {} to {}.",
                creep.name, creep_pos.f(), expected_pos.f()
            );
            repath_required = true;
        }
    }
    
    if let Some(travel_spec) = creep.travel_state.spec.as_ref() {
        if travel_spec.is_in_target_rect(creep_pos) {
            local_debug!(
                "Creep {} arrived at {} (target was {} with range {}).",
                creep.name,
                creep_pos.f(),
                travel_spec.target.f(),
                travel_spec.range
            );
            creep.travel_state.path.clear();
            creep.travel_state.arrived = true;
            creep.travel_state.arrival_broadcast.broadcast(Ok(creep_pos));
        } else if repath_required {
            local_debug!("Repathing.");
            match find_path(creep_pos, travel_spec) {
                Ok(path) => {
                    // Reusing the existing broadcast.
                    local_debug!("Chosen path: {:?}.", creep.travel_state.path);
                    creep.travel_state.path = path;
                }
                Err(e) => {
                    local_debug!("Failed to repath creep {}.", creep.name);
                    creep.travel_state.arrival_broadcast.broadcast(Err(e));
                }
            }
        }
    }
}

pub async fn move_creeps() {
    loop {
        // Trying to minimize the amount of work for non-conflicted creeps, so first checking which
        // ones can just move where they want.
        let mut creeps_by_target_pos: FxHashMap<Position, (ObjectId<screeps::Creep>, CreepRef)> = FxHashMap::default();
        // Creeps that may be unable to move where they want. Does not include fatigued creeps.
        let mut conflicted_creeps = FxHashMap::default();
        // TODO Take into account that fatigued creeps will move, albeit not immediately.
        //      In this case, it might be preferable to just wait and go at a fraction of the speed,
        //      e.g., when the alternative is swamp.
        let mut fatigued_creeps = FxHashSet::default();
        // TODO Also include immovable creeps.
        let mut fatigued_creeps_pos = FxHashSet::default();

        for_each_creep(|creep_ref| {
            let mut creep = creep_ref.borrow_mut();
            let creep_id = u!(creep.screeps_id());

            let current_pos = creep.travel_state.pos;
            let mut target_pos = creep.travel_state.next_pos();
            let fatigue = u!(creep.fatigue());
            if fatigue > 0 {
                target_pos = current_pos;
                fatigued_creeps.insert(creep_id);
                fatigued_creeps_pos.insert(current_pos);
            }

            match creeps_by_target_pos.entry(target_pos) {
                Entry::Occupied(entry) => {
                    // Only non-fatigued creeps need to be added to the conflict.
                    if fatigue == 0 {
                        conflicted_creeps.insert(creep_id, creep_ref.clone());
                    }
                    if !fatigued_creeps.contains(&entry.get().0) {
                        conflicted_creeps.insert(entry.get().0, entry.get().1.clone());
                    }
                }
                Entry::Vacant(entry) => {
                    entry.insert((creep_id, creep_ref.clone()));
                }
            }
        });

        with_room_states(|room_states| {
            resolve_conflicts(room_states, creeps_by_target_pos, conflicted_creeps, fatigued_creeps_pos);
        });

        // TODO Visualization of creep paths.
        for_each_creep(|creep_ref| {
            let mut creep = creep_ref.borrow_mut();

            if let Some(&next_pos) = creep.travel_state.path.last() {
                let creep_id = u!(creep.screeps_id());
                let fatigued = fatigued_creeps.contains(&creep_id);
                if DEBUG {
                    if let Some(travel_spec) = creep.travel_state.spec.as_ref() {
                        local_debug!(
                            "Moving creep {} towards {} (range {}). Current tile is {} and next tile is {}. Fatigued: {}.",
                            creep.name,
                            travel_spec.target.f(),
                            travel_spec.range,
                            creep.travel_state.pos.f(),
                            next_pos.f(),
                            fatigued
                        );
                    } else {
                        local_debug!(
                            "Moving creep {} out of the way from {} to {}. Fatigued: {}.",
                            creep.name,
                            creep.travel_state.pos.f(),
                            next_pos.f(),
                            fatigued
                        );
                    }
                }
                if next_pos == creep.travel_state.pos {
                    // `creep_pos` being equal to the current position means the creep is supposed
                    // to stay put for a tick as a result of conflict resolution. In this case,
                    // the position is simply removed from the path.
                    creep.travel_state.path.pop();
                } else if !fatigued {
                    // If the creep is fatigued, it cannot move. Returning next position to
                    // the path. Otherwise, the creep moves along the path.
                    let direction = u!(creep.travel_state.pos.get_direction_to(next_pos));
                    let result = creep.move_direction(direction);
                    if result.is_err() {
                        result.warn_if_err(&format!(
                            "Could not move creep {} to {}",
                            creep.name,
                            direction
                        ));
                        // If the move failed, returning the pos to the next position.
                        creep.travel_state.path.push(next_pos);
                    }
                }
            }
        });

        sleep(1).await;
    }
}

fn resolve_conflicts<I, C>(
    room_states: &RoomStates,
    creeps_by_target_pos: FxHashMap<Position, (I, Rc<RefCell<C>>)>,
    mut conflicted_creeps: FxHashMap<I, Rc<RefCell<C>>>,
    extra_obstacles: FxHashSet<Position>,
)
where
    I: Hash + PartialEq + Eq + Copy,
    C: GenericCreep,
{
    // Starting to resolve conflicts from here. Recursively increasing the set of conflicted
    // creeps at the same time as building the map with movement costs for each creep.
    // The core of the algorithm is a minimum cost bipartite matching problem between
    // creeps and possible positions.

    // We build up the encoded bipartite graph with costs of given creep going to given field.
    let mut creeps_movement_costs = Vec::new();
    // The graph built from `creeps_movement_costs` is not labelled, so we store creeps
    // in the same order in a separate vector along with a small distance matrix used to compute
    // these costs. The distance matrix is important later to modify the path without
    // recomputing missing parts.
    let mut creeps_and_repath_data = Vec::new();
    // Positions encoded as consecutive indices.
    let mut pos_to_index = FxHashMap::default();

    let mut get_pos_index = |pos: Position| {
        pos_to_index
            .get(&pos)
            .cloned()
            .unwrap_or_else(|| {
                let i = pos_to_index.len();
                pos_to_index.insert(pos, i);
                i
            })
    };

    // Creeps that are in conflict can potentially move in any direction and cause conflicts
    // with any creeps that want to move in one of these nine fields.
    let mut conflict_queue = conflicted_creeps.values().cloned().collect::<Vec<_>>();
    while let Some(conflicted_creep_ref) = conflict_queue.pop() {
        let creep = conflicted_creep_ref.borrow_mut();
        let creep_pos = creep.get_travel_state().pos;

        // Indexes equal to the ones in GridGraphDirection or screeps::Direction + 0 for
        // the middle.
        // It does not matter that the indices are repeated as long as the cost is MAX.
        let mut costs = [
            (0, u32::MAX),
            (0, u32::MAX),
            (0, u32::MAX),
            (0, u32::MAX),
            (0, u32::MAX),
            (0, u32::MAX),
            (0, u32::MAX),
            (0, u32::MAX),
            (0, u32::MAX),
        ];
        
        // The cost of a move intent.
        let intent_cost = 1000u32;
        // The cost of one tick worth of travelling or waiting.
        let ttl_cost = 1000u32;
        
        // A shortest path in 3 x 3 square can have at most 4 moves. This can cost as much as
        // 4 * 49 * 5 * TTL cost. This value has to be higher than that to take priority.
        // On the other hand, this times maximum priority times 5 must fit in the type.
        let no_progress_cost = 1000 * ttl_cost;

        let room_state = u!(room_states.get(&creep_pos.room_name()));
        // Creating the matrix of costs.
        // It depends on travel spec, body, terrain, structures and is a weighted sum of
        // the intent cost (number of intents used for movement) and the TTL cost
        // (number of TTL lost on the movement outside target).
        // TTL spent within the target range are not counted, but intents still
        // are.
        // As a balance between efficiency and complexity, only the next two moves from
        // the path are taken into consideration, so if the target is more than 2 tiles
        // away, the field on the path 2 tiles away is the next single-tile target.
        // We start from a 3x3 matrix centered on a creep.
        let creep_xy = creep_pos.xy();
        let mut slice = ball(creep_xy, 1);

        // We extend to a 3x4 or 4x4 matrix if the path is at least 2 tiles long
        // (excluding the current tile).
        let path_len = creep.get_travel_state().path.len();
        if DEBUG && path_len != 0 {
            let dist = creep.get_travel_state().path[path_len - 1].get_range_to(creep.get_travel_state().pos);
            if dist != 1 {
                warn!(
                    "Creep {} at {} has path that is {} tiles away, starting from {}.",
                    creep.get_name(),
                    creep.get_travel_state().pos.f(),
                    dist,
                    creep.get_travel_state().path[path_len - 1].f()
                );
            }
        }
        
        let mut target_rect;
        let mut path_reused = true;
        // TODO Handle cross-room movement.
        if path_len >= 2 {
            let two_tiles_further = creep.get_travel_state().path[path_len - 2].xy();
            slice = slice.extended(two_tiles_further);
            target_rect = ball(two_tiles_further, 0);
        } else if path_len == 1 {
            target_rect = ball(creep.get_travel_state().path[0].xy(), 0);
        } else {
            // An idle creep can just go wherever.
            target_rect = ball(creep_xy, 1);
            path_reused = false;
        }

        let target_rect_priority;
        let progress_priority;
        if let Some(travel_spec) = creep.get_travel_state().spec.as_ref() {
            target_rect_priority = travel_spec.target_rect_priority.into();
            progress_priority = travel_spec.progress_priority.into();
            
            // Repathing if the target is on creep's or adjacent tile.
            if travel_spec.target.get_range_to(creep_pos) <= 1 + travel_spec.range as u32 {
                // Unless something is wrong with pathfinding, if the target has not
                // been reached, the creep should have a path towards it.
                target_rect = travel_spec.target_rect();
                path_reused = false;
            } else if creep.get_travel_state().path.is_empty() {
                warn!(
                    "Target {} has not been reached by creep {}, but its path is empty.",
                    travel_spec.target.f(),
                    creep.get_name()
                );
            }
        } else {
            target_rect_priority = 0;
            progress_priority = 0;
        }

        local_debug!("{:?} {} {:?} {:?}", creep.get_travel_state(), creep_pos.f(), target_rect, slice);
        target_rect = u!(target_rect.intersection(slice));

        // The traffic costs are a 3x3 slice of the distance matrix towards the target
        // plus the intent that needs to be taken to that position.
        // To obtain it, we must first compute the distance matrix on up to 4x4 slice
        // containing the target position up to 2 tiles away.
        // To compute this, we need to set movement costs of each tile.
        let mut movement_costs = RoomMatrixSlice::new(slice, u32::MAX);
        for xy in slice.iter() {
            let surface = room_state.tile_surface(xy);
            if surface != Surface::Obstacle && !extra_obstacles.contains(&xy.to_pos(creep_pos.room_name())) {
                if target_rect.contains(xy) {
                    // Being within the target area does not use up TTL since
                    // the creep is still able to do what it needs to do.
                    // It also has zero cost of intents or progress to get to the target area.
                    movement_costs.set(xy, 0);
                } else {
                    let tile_ttl = creep.get_ticks_per_tile(surface) as u32;
                    let no_progress_cost_multiplier = if target_rect.contains(creep_pos.xy()) {
                        target_rect_priority
                    } else {
                        progress_priority
                    };
                    movement_costs.set(xy, no_progress_cost_multiplier as u32 * no_progress_cost + intent_cost + tile_ttl * ttl_cost);
                }
            }
        }
        // Note that this distance matrix contains TTL costs of terrain of given field
        // on that very field, so the TTL cost of entering the tile is already included
        // and the TTL cost of entering the target tile is excluded.
        let dm = weighted_distance_matrix(&movement_costs, target_rect.iter());
        // Computing the cost based on the distance matrix.
        // Also, propagating the conflicts.
        for direction in all::<GridDirection>() {
            let offset = direction_to_offset(direction);
            let maybe_xy = creep_xy.try_add_diff(offset);
            let xy = if let Ok(xy) = maybe_xy {
                xy
            } else {
                continue;
            };
            let surface = room_state.tile_surface(xy);
            if surface != Surface::Obstacle && !extra_obstacles.contains(&xy.to_pos(creep_pos.room_name())) {
                a!(dm.get(xy) != obstacle_cost::<u32>());
                let pos = xy.to_pos(creep_pos.room_name());
                // TTL cost of movement into the given tile.
                let tile_cost = if direction != GridDirection::Center {
                    // The cost of movement is an intent and number of TTL lost.
                    if target_rect.contains(xy) {
                        // Simply moving to another location where work can be done.
                        // Slightly preferring tiles from which the creep can move faster.
                        intent_cost + creep.get_ticks_per_tile(surface) as u32
                    } else {
                        // Wasting a number of ticks on travel instead of work.
                        intent_cost + creep.get_ticks_per_tile(surface) as u32 * ttl_cost
                    }
                } else if target_rect.contains(xy) {
                    // The creep is already at the target
                    0
                } else {
                    // The creep intends to not move towards its goal. When staying put,
                    // the minimal cost is a single TTL lost.
                    // However, this could lead to waiting forever in the case when the tile
                    // the creep was waiting for is not being emptied by itself.
                    // To avoid this, TTL cost is prioritized.
                    // TODO Sometimes it is okay to wait. For each field around that contains
                    //      a fatigued creep, consider "moving" into it with a cost of staying put
                    //      and wasting a few TTL. To avoid deadlocks, creep needs to have
                    //      "patience" to not do this after it failed for a few ticks in a row.
                    //      It may be preferable to go behind a slow creep instead of going into
                    //      a swamp and still being behind.
                    // TODO Do something so that if there is a 3x3 ball of upgraders, a hauler can
                    //      still shove one of them away to deliver the energy. Maybe some kind of
                    //      priority over creeps that are not in their target? Somehow make cost of
                    //      moving smaller if no progress can be made anyway. Maybe using two
                    //      passes?
                    ttl_cost
                };
                // Cost of movement after.
                let pos = xy.to_pos(creep_pos.room_name());
                costs[direction as usize] = (
                    get_pos_index(pos),
                    dm.get(xy) + tile_cost
                );
                
                local_debug!(
                    "Movement costs for {} from {} to {}: {}.",
                    creep.get_name(), creep_pos.f(), pos.f(), dm.get(xy) + tile_cost
                );

                // If there is some other creep willing to travel to this tile,
                // it may clash with this creep and thus is also in the conflict.
                if let Some((other_creep_id, other_creep_ref)) = creeps_by_target_pos.get(&pos) {
                    conflicted_creeps.entry(*other_creep_id).or_insert_with(|| {
                        conflict_queue.push(other_creep_ref.clone());
                        other_creep_ref.clone()
                    });
                }
            }
        }

        let repath_data = RepathData::Adjusted {
            distance_matrix: dm,
            target_rect,
            path_reused,
        };
        
        local_debug!(
            "Movement costs for {} from {}: {:?}.",
            creep.get_name(), creep_pos.f(), costs
        );
        
        drop(creep);
        creeps_and_repath_data.push((conflicted_creep_ref, repath_data));
        creeps_movement_costs.push(costs);
    }

    // Should never fail because each creep should be able to not move.
    local_debug!("min_cost_weighted_matching({:?})", creeps_movement_costs);
    let creep_tile_ixs = u!(min_cost_weighted_matching(&creeps_movement_costs[..])).0;

    let index_to_pos = pos_to_index
        .into_iter()
        .map(|(pos, i)| (i, pos))
        .collect::<FxHashMap<_, _>>();
    
    local_debug!("index_to_pos = {:?}", index_to_pos);

    // Adjusting the paths of conflicted creeps. The path may contain the position at which the
    // creep already is, indicating no move this tick.
    for ((creep_ref, repath_data), pos_ix) in zip(creeps_and_repath_data.into_iter(), creep_tile_ixs.into_iter()) {
        let next_pos = *u!(index_to_pos.get(&pos_ix));
        let room_name = next_pos.room_name();

        let mut creep = creep_ref.borrow_mut();

        // Updating the creep's path, starting from the computed position (possibly equal to
        // the current position, indicating no movement).
        let mut path = vec![next_pos];
        // Creeps without travel spec are just moved out of the way. They also shouldn't have a
        // path to preserve. However, creeps with travel spec need a new path.
        if let RepathData::Adjusted { distance_matrix, target_rect, path_reused } = repath_data {
            // There are two possible situations. Either a path is being partially preserved or
            // completely replaced (e.g. when the creep is moving out of target area).
            // Both involve pathing to the target area first.
            let mut xy = next_pos.xy();

            local_debug!(
                "Adjusting path of {} at {} going into {} and then by distance matrix. distance_matrix=\n{}\ntarget_rect={:?}\npath_reused={}",
                creep.get_name(),
                creep.get_travel_state().pos.f(),
                next_pos.f(),
                distance_matrix,
                target_rect,
                path_reused
            );

            // TODO Handle multi-room movement.
            while !target_rect.contains(xy) {
                local_debug!("xy={}", xy);
                // The next position is an adjacent one with minimal distance in the
                // distance matrix.
                xy = u!(distance_matrix
                    .around_xy(xy)
                    .map(|near| (near, distance_matrix.get(near)))
                    .min_by_key(|(_, dist)| *dist)
                    .map(|(near, _)| near));
                let pos = xy.to_pos(room_name);

                if DEBUG {
                    a!(!path.contains(&pos));
                }

                path.push(pos);
            }

            if path_reused {
                // This is the case where the creep needs to get back on path.
                // Computing the next point on the path that is further away than 2 tiles.
                // Adding the rest of the path, keeping in mind that it is a stack.
                path.extend(creep.get_travel_state().path.iter().rev().skip(2));
                // TODO Check if continuous.
            }

            // The path is supposed to be a stack, so reversing it.
            path.reverse();
        }
        creep.get_travel_state_mut().path = path;
    }
}

#[cfg(test)]
mod tests {
    use std::cell::RefCell;
    use std::rc::Rc;
    use std::str::FromStr;
    use log::LevelFilter::Trace;
    use log::trace;
    use rustc_hash::{FxHashMap, FxHashSet};
    use screeps::{Part, Position, RoomName};
    use screeps::Terrain::{Swamp};
    use crate::creeps::generic_creep::GenericCreep;
    use crate::creeps::test_creep::TestCreep;
    use crate::geometry::position_utils::PositionUtils;
    use crate::logging::init_logging;
    use crate::room_states::room_state::test_empty_unowned_room_name;
    use crate::room_states::room_states::test_room_states;
    use crate::travel::traffic::resolve_conflicts;
    use crate::travel::travel_spec::TravelSpec;

    #[test]
    fn test_collision_with_equally_good_route() {
        init_logging(Trace);

        let mut creeps_by_target_pos = FxHashMap::default();
        let mut conflicted_creeps = FxHashMap::default();
        
        let test_room_name = RoomName::from_str("W1N1").unwrap();
        
        let test_creep_1 = Rc::new(RefCell::new(TestCreep::new(
            1,
            Position::new_from_raw(10, 10, test_room_name),
            vec![Part::Work, Part::Move].into()
        )));
        test_creep_1.borrow_mut().get_travel_state_mut().path = vec![
            Position::new_from_raw(12, 10, test_room_name),
            Position::new_from_raw(11, 10, test_room_name),
        ];
        test_creep_1.borrow_mut().get_travel_state_mut().spec = Some(TravelSpec::new(
            Position::new_from_raw(12, 10, test_room_name),
            0
        ));

        let test_creep_2 = Rc::new(RefCell::new(TestCreep::new(
            2,
            Position::new_from_raw(12, 10, test_room_name),
            vec![Part::Work, Part::Move].into()
        )));
        test_creep_2.borrow_mut().get_travel_state_mut().path = vec![
            Position::new_from_raw(10, 10, test_room_name),
            Position::new_from_raw(11, 10, test_room_name),
        ];
        test_creep_2.borrow_mut().get_travel_state_mut().spec = Some(TravelSpec::new(
            Position::new_from_raw(10, 10, test_room_name),
            0
        ));

        creeps_by_target_pos.insert(test_creep_1.borrow().get_travel_state().pos, (1, test_creep_1.clone()));
        creeps_by_target_pos.insert(test_creep_2.borrow().get_travel_state().pos, (2, test_creep_2.clone()));

        conflicted_creeps.insert(1, test_creep_1.clone());
        conflicted_creeps.insert(2, test_creep_2.clone());

        let mut room_states = test_room_states();
        let room_state = room_states.get_mut(&test_empty_unowned_room_name()).unwrap();

        room_state.terrain.set((10, 11).try_into().unwrap(), Swamp);
        room_state.terrain.set((11, 11).try_into().unwrap(), Swamp);
        room_state.terrain.set((12, 11).try_into().unwrap(), Swamp);

        resolve_conflicts(&room_states, creeps_by_target_pos, conflicted_creeps, FxHashSet::default());

        trace!("creep1 path: {:?}", test_creep_1.borrow().get_travel_state().path.iter().map(|pos| pos.f()).collect::<Vec<_>>());
        trace!("creep2 path: {:?}", test_creep_2.borrow().get_travel_state().path.iter().map(|pos| pos.f()).collect::<Vec<_>>());

        assert!(
            test_creep_1.borrow().get_travel_state().path
            ==
            vec![
                Position::new_from_raw(12, 10, test_room_name),
                Position::new_from_raw(11, 9, test_room_name),
            ]
            ||
            test_creep_2.borrow().get_travel_state().path
            ==
            vec![
                Position::new_from_raw(10, 10, test_room_name),
                Position::new_from_raw(11, 9, test_room_name),
            ]
        );
    }

    #[test]
    fn test_collision_of_idle_with_moving() {
        init_logging(Trace);

        let mut creeps_by_target_pos = FxHashMap::default();
        let mut conflicted_creeps = FxHashMap::default();

        let test_room_name = RoomName::from_str("W1N1").unwrap();

        let test_idle_creep = Rc::new(RefCell::new(TestCreep::new(
            1,
            Position::new_from_raw(10, 10, test_room_name),
            vec![Part::Work, Part::Move].into()
        )));
        test_idle_creep.borrow_mut().get_travel_state_mut().spec = Some(TravelSpec::new(
            Position::new_from_raw(10, 10, test_room_name),
            1
        ));

        let test_moving_creep = Rc::new(RefCell::new(TestCreep::new(
            2,
            Position::new_from_raw(11, 10, test_room_name),
            vec![Part::Work, Part::Move].into()
        )));
        test_moving_creep.borrow_mut().get_travel_state_mut().path = vec![
            Position::new_from_raw(10, 10, test_room_name)
        ];
        test_moving_creep.borrow_mut().get_travel_state_mut().spec = Some(TravelSpec::new(
            Position::new_from_raw(10, 10, test_room_name),
            0
        ));

        creeps_by_target_pos.insert(test_idle_creep.borrow().get_travel_state().pos, (1, test_idle_creep.clone()));
        creeps_by_target_pos.insert(test_moving_creep.borrow().get_travel_state().pos, (2, test_moving_creep.clone()));

        conflicted_creeps.insert(1, test_idle_creep.clone());
        conflicted_creeps.insert(2, test_moving_creep.clone());

        let mut room_states = test_room_states();
        let room_state = room_states.get_mut(&test_empty_unowned_room_name()).unwrap();

        room_state.terrain.set((9, 11).try_into().unwrap(), Swamp);
        room_state.terrain.set((10, 11).try_into().unwrap(), Swamp);
        room_state.terrain.set((11, 11).try_into().unwrap(), Swamp);
        room_state.terrain.set((9, 9).try_into().unwrap(), Swamp);
        room_state.terrain.set((10, 9).try_into().unwrap(), Swamp);
        room_state.terrain.set((11, 9).try_into().unwrap(), Swamp);
        room_state.terrain.set((11, 10).try_into().unwrap(), Swamp);

        resolve_conflicts(&room_states, creeps_by_target_pos, conflicted_creeps, FxHashSet::default());

        trace!("creep1 path: {:?}", test_idle_creep.borrow().get_travel_state().path.iter().map(|pos| pos.f()).collect::<Vec<_>>());
        trace!("creep2 path: {:?}", test_moving_creep.borrow().get_travel_state().path.iter().map(|pos| pos.f()).collect::<Vec<_>>());

        // Moving to the only tile without swamp.
        assert_eq!(
            test_idle_creep.borrow().get_travel_state().path,
            vec![Position::new_from_raw(9, 10, test_room_name)]
        );
        // Moving to the target tile.
        assert_eq!(
            test_moving_creep.borrow().get_travel_state().path,
            vec![Position::new_from_raw(10, 10, test_room_name)]
        );
    }
}