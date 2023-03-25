use crate::config::*;
use wasm_bindgen::prelude::*;
use nanorand::{Rng, WyRand};
use screeps::{RoomXY, ROOM_SIZE, game};
use crate::algorithms::grid_bfs_distances::grid_bfs_distances;
use crate::algorithms::matrix_common::MatrixCommon;
use crate::profiler::measure_time;
use log::debug;

mod config;
mod geometry;
mod kernel;
mod logging;
mod test_process;
mod room_state;
mod blueprint;
mod algorithms;
mod consts;
mod profiler;
mod tile_graph;

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
        let mut rng = nanorand::WyRand::new_seed(game::time() as u64);
        let obstacles: Vec<RoomXY> = (0..number_of_obstacles).map(|_| RoomXY::try_from((rng.generate_range(1..ROOM_SIZE), rng.generate_range(1..ROOM_SIZE))).unwrap()).collect();
        let start = [RoomXY::try_from((0, 0)).unwrap()];
        measure_time("grid_bfs_distances", || {
            let mut total = 0.0;
            for i in 0..n {
                let result = grid_bfs_distances(start.iter(), obstacles.iter());
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
