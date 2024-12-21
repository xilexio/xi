#![allow(unknown_lints)]
#![allow(incomplete_features)]
#![allow(enum_variant_names)]
#![feature(async_closure)]
#![feature(btree_cursors)]
#![feature(btree_extract_if)]
#![feature(extract_if)]

use js_sys::JsString;
use wasm_bindgen::prelude::wasm_bindgen;

mod algorithms;
mod config;
mod construction;
mod consts;
mod fresh_number;
mod game_loop;
mod geometry;
mod global_state;
mod kernel;
mod logging;
mod priorities;
mod profiler;
mod room_planning;
mod room_states;
mod spawning;
mod towers;
mod utils;
mod visualization;
mod errors;
mod hauling;
mod creeps;
mod economy;
mod room_maintenance;
mod travel;
mod defense;
mod flags;

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
