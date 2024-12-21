use log::{debug, trace};
use screeps::Part::{Claim, Move};
use screeps::{find, game, HasPosition, Position};
use crate::creeps::creep_role::CreepRole::Claimer;
use crate::geometry::position_utils::PositionUtils;
use crate::kernel::sleep::sleep;
use crate::room_states::room_state::RoomDesignation;
use crate::room_states::room_states::with_room_state;
use crate::spawning::scheduling_creeps::schedule_creep;
use crate::spawning::spawn_schedule::generic_base_spawn_request;
use crate::travel::nearest_room::find_nearest_owned_room;
use crate::travel::travel::travel;
use crate::travel::travel_spec::TravelSpec;
use crate::u;
use crate::utils::game_tick::game_tick;
use crate::utils::priority::Priority;

pub async fn claim_room(controller_pos: Position) {
    loop {
        let room_name = controller_pos.room_name();
        debug!("Trying to claim room {}.", room_name);
        if with_room_state(room_name, |room_state| room_state.designation) == Some(RoomDesignation::Owned) {
            debug!("Room {} is already owned.", room_name);
            return;
        }
        
        if let Some(claimer_provider_room_name) = find_nearest_owned_room(room_name, 3) {
            let spawn_request = u!(with_room_state(claimer_provider_room_name, |room_state| {
                let mut spawn_request = generic_base_spawn_request(room_state, Claimer);
                spawn_request.priority = Priority(120);
                spawn_request.tick = (game_tick(), game_tick() + 400);
                // TODO Only if there's at least 650 spawn capacity. If there's 850 capacity,
                //      prefer 5 Move. 
                spawn_request.body = vec![(Move, 1), (Claim, 1)].into();
                spawn_request
            }));
    
            debug!("Waiting until claimer is spawned in room {}.", claimer_provider_room_name);
            let spawn_promise = u!(schedule_creep(claimer_provider_room_name, spawn_request));
            while spawn_promise.borrow().is_pending() {
                trace!("{:?}", spawn_promise);
                sleep(1).await;
            }
            let claimer = spawn_promise.borrow_mut().creep.take();
            if let Some(claimer) = claimer {
                debug!("Moving the claimer towards the flag.");
                let travel_spec = TravelSpec::new(controller_pos, 1);
                while let Err(e) = travel(&claimer, travel_spec.clone()).await {
                    // TODO It's not accepting path since its only to room's edge.
                    e.warn(&format!("Failed to move claimer to {}.", controller_pos.f()));
                    sleep(1).await;
                }
                
                // TODO If the claimer dies, it may never finish waiting. Add timeout.
                debug!("Creep arrived. Locating the controller.");
                let room = u!(game::rooms().get(room_name));
                
                if let Some(controller) = room.controller() {
                    if controller.pos().get_range_to(claimer.borrow().travel_state.pos) == 1 {
                        match claimer.borrow_mut().claim(&controller) {
                            Ok(()) => {
                                debug!("Creep successfully claimed room {}.", room_name);
                                
                                for flag in room.find(find::FLAGS, None) {
                                    let flag_name = flag.name();
                                    if flag_name.starts_with("claim") {
                                        debug!("Removing flag {}.", flag_name);
                                        flag.remove();
                                    }
                                }
                                return;
                            },
                            Err(e) => {
                                e.warn(&format!("Failed to claim room {}", room_name));
                            }
                        }
                    } else {
                        debug!("The creep is not adjacent to the controller.");
                    }
                } else {
                    debug!("Failed to get the controller data.");
                }
            } else {
                debug!("Failed to spawn the claimer.");
            }
        }
        
        // Waiting until the task becomes possible.
        debug!("Waiting 100 ticks until next attempt.");
        sleep(100).await;
    }
}