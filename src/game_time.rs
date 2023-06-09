#[cfg(not(test))]
pub fn game_time() -> u32 {
    screeps::game::time()
}

#[cfg(test)]
pub static mut GAME_TIME: u32 = 1u32;

#[cfg(test)]
pub fn game_time() -> u32 {
    unsafe { GAME_TIME }
}
