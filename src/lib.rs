#![feature(return_position_impl_trait_in_trait)]

use crate::algorithms::chunk_graph::chunk_graph;
use crate::algorithms::distance_matrix::distance_matrix;
use crate::algorithms::matrix_common::MatrixCommon;
use crate::config::*;
use crate::room_state::room_states::with_room_state;
use crate::room_state::scan::scan;
use crate::visualization::{Visualization, Visualizer};
use js_sys::Math::random;
use log::debug;
use profiler::measure_time;
use screeps::{game, ROOM_SIZE};
use wasm_bindgen::prelude::{wasm_bindgen, UnwrapThrowExt};
use crate::algorithms::distance_transform::distance_transform;
use tap::prelude::*;
use crate::room_planner::plan_room;

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
mod visualization;
mod room_planner;

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

    debug!("Tick: {} -- Bucket: {}", game::time(), game::cpu::bucket());

    if game::cpu::bucket() > 1000 {
        let spawn = game::spawns().values().next().unwrap_throw();
        let room_name = spawn.room().unwrap_throw().name();
        scan(room_name).unwrap_throw();
        let visualizer = Visualizer {};
        let plan = measure_time("plan_room", || {
            with_room_state(room_name, |state| {
                plan_room(state)
            }).unwrap()
        }).unwrap_throw();
        visualizer.visualize(
            room_name,
            &Visualization::Structures(plan.buildings)
        );
    }

    // trace!("test");
    // info!("test");
    // debug!("test");
    // warn!("test");
    // error!("test");
}
