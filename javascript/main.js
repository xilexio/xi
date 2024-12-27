"use strict";
let wasmModule;
let initialized = false;

function displayError(...args) {
    const message = args.join(' ');
    console.log('<span style="color: #f99">' + message + '</span>');
    Game.notify(args.join(' '));
}

let restartNextTick = false;

function wrap(f) {
    return function(...args) {
        try {
            if (wasmModule && wasmModule.__wasm) {
                f(...args);
            } else {
                displayError('WASM VM is not ready.')
            }
        } catch (ex) {
            displayError('Caught exception:', ex);
            if (ex.stack) {
                displayError('Stacktrace:', ex.stack);
            }
            displayError('Restarting the bot next tick.');
            restartNextTick = true;
        }
    }
}

function runLoop(wasm_module) {
    // The deserialized Memory object is replaced with a fresh object that will be forgotten after the loop.
    // The RawMemory object is not touched here.
    delete global.Memory;
    global.Memory = {};
    try {
        // Running the actual game loop.
        wasm_module.loop();
    } finally {
        // Showing the log in one message.
        console.log(wasm_module.take_log());
    }
}

global.set_room_blueprint = wrap(function(roomName, blueprintJSON) {
    wasmModule.set_room_blueprint(roomName, blueprintJSON);
});

module.exports.loop = function () {
    if (restartNextTick) {
        Game.cpu.halt();
    }

    try {
        if (wasmModule && wasmModule.__wasm && initialized) {
            runLoop(wasmModule);
        } else {
            // Attempt to load the wasm only if there's enough bucket to do a bunch of work this tick.
            const minCPU = 750;
            if (Game.cpu.bucket < minCPU) {
                console.log(`There is ${Game.cpu.bucket} CPU left in the bucket. At least ${minCPU} CPU is required ` +
                    `to proceed with the compilation. Waiting.`);
                return;
            }

            // Loading the Rust module compiled to WASM.
            wasmModule = require('xi');
            // Load the WASM instance.
            wasmModule.initialize_instance();
            // Running the exported setup function once.
            wasmModule.setup();
            // Running the exported loop function this tick and then later each new tick.
            runLoop(wasmModule);
            // Marking the bot as initialized.
            initialized = true;
        }
    } catch (ex) {
        displayError('Caught exception:', ex);
        if (ex.stack) {
            displayError('Stacktrace:', ex.stack);
        }
        displayError('Restarting the bot next tick.');
        restartNextTick = true;
    }
}
