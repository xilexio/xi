use parking_lot::Mutex;

/// Current game tick.
/// A wrapper on the API to enable testing functions that depend on game tick.
#[cfg(not(test))]
#[inline]
pub fn game_tick() -> u32 {
    screeps::game::time()
}

#[cfg(test)]
pub static mut GAME_TIME: u32 = 1u32;

#[cfg(test)]
pub fn game_tick() -> u32 {
    unsafe { GAME_TIME }
}

static FIRST_TICK: Mutex<u32> = Mutex::new(0);

/// Returns the first tick when this function was called, i.e., the first tick after restart.
pub fn first_tick() -> u32 {
    let mut tick = FIRST_TICK.lock();
    if *tick == 0 {
        *tick = game_tick();
    }
    *tick
}