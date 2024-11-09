use log::trace;
use screeps::{RoomName, ENERGY_REGEN_TIME, SOURCE_ENERGY_CAPACITY};
use serde::{Deserialize, Serialize};
use crate::hauling::haul_resources::hauler_body;
use crate::kernel::sleep::sleep;
use crate::room_state::room_states::with_room_state;

pub mod cost_approximation;

/// A structure gathering energy, transportation throughput and other statistics to decide on
/// the distribution of resources in the room, e.g., on the number of haulers, upgraders, etc. 
#[derive(Debug, Deserialize, Serialize)]
pub struct RoomResourceDistribution {
    pub required_hauling_throughput: f32,
    pub required_haulers: u32,
}

impl Default for RoomResourceDistribution {
    fn default() -> Self {
        RoomResourceDistribution {
            required_hauling_throughput: 0.0,
            required_haulers: 1,
        }
    }
}

pub async fn update_resource_distribution(room_name: RoomName) {
    loop {
        trace!("Updating resource distribution.");

        with_room_state(room_name, |room_state| {
            let body = hauler_body(room_state);
            let hauler_body_capacity = body.store_capacity();
            let hauler_ticks_per_tile = body.ticks_per_tile(false);
            let hauler_throughput = hauler_body_capacity as f32 / (2 * hauler_ticks_per_tile) as f32;

            let source_energy_production = SOURCE_ENERGY_CAPACITY as f32 / ENERGY_REGEN_TIME as f32;

            let average_hauling_distance = 15.0;

            let required_hauling_throughput = source_energy_production * average_hauling_distance;
            let required_haulers = (required_hauling_throughput / hauler_throughput).ceil() as u32;
            room_state.resource_distribution = RoomResourceDistribution {
                required_hauling_throughput,
                required_haulers,
            };

            trace!("Updated resource distribution: {:?}", room_state.resource_distribution);
        });

        sleep(20).await;
    }
}