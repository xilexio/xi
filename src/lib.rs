#![allow(incomplete_features)]
#![feature(return_position_impl_trait_in_trait)]
#![feature(async_closure)]
#![feature(once_cell)]
#![feature(core_intrinsics)]
#![feature(btree_cursors)]
#![feature(local_key_cell_methods)]

use wasm_bindgen::prelude::wasm_bindgen;

mod algorithms;
mod blueprint;
mod config;
mod construction;
mod consts;
mod cost_approximation;
mod creep;
mod creeps;
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
