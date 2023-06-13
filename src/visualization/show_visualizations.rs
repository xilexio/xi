use crate::kernel::sleep::sleep;
use crate::room_state::room_states::for_each_owned_room;
use crate::visualization::{Visualization, visualize};

pub async fn show_visualizations() {
    loop {
        for_each_owned_room(|room_name, room_state| {
            if let Some(plan) = room_state.plan.as_ref() {
                visualize(room_name, Visualization::Plan(plan.tiles.clone()));
            }
        });

        sleep(1).await;
    }
}