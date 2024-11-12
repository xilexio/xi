#![allow(incomplete_features)]
#![feature(async_closure)]
#![feature(btree_cursors)]
#![feature(btree_extract_if)]
#![feature(extract_if)]

use js_sys::JsString;
use wasm_bindgen::prelude::wasm_bindgen;

mod algorithms;
mod blueprint;
mod config;
mod construction;
mod consts;
mod fresh_number;
mod game_loop;
mod game_tick;
mod geometry;
mod global_state;
mod kernel;
mod logging;
mod maintenance;
mod priorities;
mod profiler;
mod random;
mod role;
mod room_planning;
mod room_states;
mod spawning;
mod towers;
mod utils;
mod visualization;
mod travel;
mod errors;
mod mining;
mod hauling;
mod filling_spawns;
mod reserved_creep;
mod creeps;
mod upgrade_controller;
mod economy;

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
