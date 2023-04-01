#![feature(type_alias_impl_trait)]
#![feature(return_position_impl_trait_in_trait)]

use crate::config::*;
use log::debug;
use screeps::game;
use wasm_bindgen::prelude::*;

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

    // trace!("test");
    // info!("test");
    // debug!("test");
    // warn!("test");
    // error!("test");
}
