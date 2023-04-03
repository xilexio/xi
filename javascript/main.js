"use strict";
let wasm_module;

function displayError(...args) {
    const message = args.join(' ');
    console.log('<span style="color: #ff9999">' + message + '</span>');
    Game.notify(args.join(' '));
}

let restartNextTick = false;

function wrap(f) {
    return function(...args) {
        try {
            if (wasm_module && wasm_module.__wasm) {
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

global.set_room_blueprint = wrap(function(roomName, blueprintJSON) {
    wasm_module.set_room_blueprint(roomName, blueprintJSON);
});

module.exports.loop = function () {
    if (restartNextTick) {
        Game.cpu.halt();
    }

    try {
        if (wasm_module && wasm_module.__wasm) {
            wasm_module.loop();
        } else {
            // Attempt to load the wasm only if there's enough bucket to do a bunch of work this tick.
            if (Game.cpu.bucket < 500) {
                console.log(`There is ${Game.cpu.bucket} CPU left in the bucket. At least 500 CPU is required to ` +
                    `proceed with the compilation. Waiting.`);
                return;
            }

            // Loading the Rust module compiled to WASM.
            wasm_module = require('xi');
            // Load the WASM instance.
            wasm_module.initialize_instance();
            // Running the exported setup function once.
            wasm_module.setup();
            // Running the exported loop function this tick and then later each new tick.
            wasm_module.loop();
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
