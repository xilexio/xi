#![feature(type_alias_impl_trait)]
#![feature(return_position_impl_trait_in_trait)]

use crate::algorithms::distance_matrix::distance_matrix;
use crate::config::*;
use log::debug;
use screeps::{game, RoomXY, ROOM_SIZE};
use wasm_bindgen::prelude::*;
use crate::algorithms::matrix_common::MatrixCommon;
use profiler::measure_time;
use js_sys::Math::random;

mod algorithms;
mod blueprint;
mod config;
mod consts;
mod geometry;
mod kernel;
mod logging;
mod profiler;
mod room_state;
mod test_process;

// `wasm_bindgen` to expose the function to JS.
#[wasm_bindgen]
pub fn setup() {
    logging::init_logging(LOG_LEVEL);
    kernel::init_kernel();
}

// `js_name` to use a reserved name as a function name.
#[wasm_bindgen(js_name = loop)]
pub fn game_loop() {
    // let new_process = TestProcess {
    //     meta: ProcessMeta {
    //         pid: game::time(),
    //         priority: 0,
    //     },
    // };
    //
    // let kern = kernel::kernel();
    // kern.schedule(Box::new(new_process));
    // kern.run();

    debug!("Tick {}", game::time());

    if game::cpu::bucket() > 500 {
        let n = 100;
        let number_of_obstacles = 1000;
        let obstacles: Vec<RoomXY> = (0..number_of_obstacles)
            .map(|_| {
                RoomXY::try_from((
                    (1.0 + random() * (ROOM_SIZE as f64 - 1.0)) as u8,
                    (1.0 + random() * (ROOM_SIZE as f64 - 1.0)) as u8,
                ))
                .unwrap()
            })
            .collect();
        let start = [RoomXY::try_from((0, 0)).unwrap()];
        measure_time("distance_matrix", || {
            let mut total = 0.0;
            for i in 0..n {
                let result = distance_matrix(start.into_iter(), obstacles.iter().copied());
                total += result.get(RoomXY::try_from((25, 25)).unwrap()) as f64;
            }
            debug!("avg dist (0, 0) - (25, 25): {}", total / (n as f64));
        });
    }

    // trace!("test");
    // info!("test");
    // debug!("test");
    // warn!("test");
    // error!("test");
}
