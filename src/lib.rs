use screeps::game;
use wasm_bindgen::prelude::*;
use crate::config::*;
use crate::kernel::process::{ProcessMeta};
use crate::test_process::TestProcess;

mod logging;
mod config;
mod kernel;
mod test_process;
mod geometry;

// add wasm_bindgen to any function you would like to expose for call from js
#[wasm_bindgen]
pub fn setup() {
    logging::init_logging(LOG_LEVEL);
    kernel::init_kernel();
}

// to use a reserved name as a function name, use `js_name`:
#[wasm_bindgen(js_name = loop)]
pub fn game_loop() {
    let new_process = TestProcess {
        meta: ProcessMeta {
            pid: game::time(),
            priority: 0,
        },
    };

    let kern = kernel::kernel();
    kern.schedule(Box::new(new_process));
    kern.run();
}