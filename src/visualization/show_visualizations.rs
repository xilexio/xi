use crate::game_time::game_tick;
use crate::kernel::sleep::sleep;
use crate::room_state::room_states::for_each_owned_room;
use crate::visualization::{Visualization, visualize};

pub async fn show_visualizations() {
    loop {
        for_each_owned_room(|room_name, room_state| {
            if let Some(plan) = room_state.plan.as_ref() {
                if game_tick() / 10 % 2 == 0 {
                    visualize(room_name, Visualization::Plan(plan.tiles.clone()));
                } else if let Some(current_rcl_structures) = room_state.current_rcl_structures.as_ref() {
                    visualize(room_name, Visualization::Structures(current_rcl_structures.clone()));
                }
            }
        });

        sleep(1).await;
    }
}