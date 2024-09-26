#![allow(incomplete_features)]
#![feature(return_position_impl_trait_in_trait)]
#![feature(async_closure)]
#![feature(core_intrinsics)]
#![feature(btree_cursors)]
#![feature(local_key_cell_methods)]

use js_sys::JsString;
use wasm_bindgen::prelude::wasm_bindgen;

mod algorithms;
mod blueprint;
mod config;
mod construction;
mod consts;
mod cost_approximation;
mod fresh_number;
mod game_loop;
mod game_time;
mod geometry;
mod global_state;
mod kernel;
mod logging;
mod maintenance;
mod priorities;
mod profiler;
mod random;
mod resources;
mod role;
mod room_planner;
mod room_state;
mod spawning;
mod towers;
mod utils;
mod visualization;
mod travel;
mod errors;
mod mining;
mod hauling;
mod spawn_pool;
mod filling_spawns;
mod reserved_creep;
mod unchecked_transferable;
mod creeps;

// `wasm_bindgen` to expose the function to JS.
#[wasm_bindgen]
pub fn setup() {
    game_loop::setup();
}

// `js_name` to use a reserved name as a function name.
#[wasm_bindgen(js_name = loop)]
pub fn game_loop() {
    game_loop::game_loop();
}

#[wasm_bindgen(js_name = take_log)]
pub fn take_log() -> JsString {
    logging::take_log().join("\n").into()
}
