use crate::config::*;
use log::{debug, error, info, trace, warn};
use screeps::{game, RoomXY, StructureType};
use wasm_bindgen::prelude::*;
use room_visual_ext::RoomVisualExt;

mod config;
mod geometry;
mod kernel;
mod logging;
mod test_process;
mod room_state;
mod blueprint;
mod algorithms;
mod consts;
mod error;

// add wasm_bindgen to any function you would like to expose for call from js
#[wasm_bindgen]
pub fn setup() {
    logging::init_logging(LOG_LEVEL);
    kernel::init_kernel();
}

// to use a reserved name as a function name, use `js_name`:
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

    // trace!("test");
    // info!("test");
    // debug!("test");
    // warn!("test");
    // error!("test");
}
