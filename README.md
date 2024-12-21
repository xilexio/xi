# ξ (xi) - a (not yet) fully automated screeps bot

A bot written in Rust for the [Screeps: World][screeps] game.

Uses the [screeps-game-api] bindings. [cargo-screeps] is used for deploying the bot
to Screeps servers. Initial implementation based on [screeps-rust-starter].

Setup:
```sh
# Install CLI dependency.
cargo install cargo-screeps
# Copy the example config and fill it with credentials to Screeps servers.
cp screeps.example.toml screeps.toml
```

Build without deployment:
```sh
cargo screeps build
```

Build and deployment to selected target present in `screeps.toml`:
```sh
cargo screeps deploy -m mmo
```

## Flag orders

The bot can be partially controlled manually using flags. The action performed
depends on the prefix of the flag's name.
* `claim` flags placed on a controller make the nearest RCL3+ room spawn a 
  claimer, move it to that room and claim it,

[screeps]: https://screeps.com
[cargo-screeps]: https://github.com/rustyscreeps/cargo-screeps
[screeps-game-api]: https://github.com/rustyscreeps/screeps-game-api
[rustyscreeps]: https://github.com/rustyscreeps
[screeps-rust-starter]: https://github.com/rustyscreeps/screeps-starter-rust