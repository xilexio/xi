use crate::utils::game_tick::game_tick;

pub const LARGE_SAMPLE_SIZE: usize = 100;
pub const SMALL_SAMPLE_SIZE: usize = 10;
pub const SAMPLE_INTERVAL: u32 = 10;

const SAMPLE_TICK_MOD: u32 = 4;

pub fn is_sample_tick() -> bool {
    game_tick() % SAMPLE_INTERVAL == SAMPLE_TICK_MOD
}

pub fn ticks_until_sample_tick(min_ticks: u32) -> u32 {
    min_ticks + (SAMPLE_TICK_MOD + SAMPLE_INTERVAL - (game_tick() + min_ticks) % SAMPLE_INTERVAL) % SAMPLE_INTERVAL
}

#[cfg(test)]
mod tests {
    use log::LevelFilter::Trace;
    use crate::logging::init_logging;
    use crate::utils::game_tick::{game_tick, GAME_TICK};
    use crate::utils::sampling::{ticks_until_sample_tick, SAMPLE_INTERVAL, SAMPLE_TICK_MOD};

    #[test]
    fn test_ticks_until_sample_tick() {
        init_logging(Trace);
        
        unsafe {
            GAME_TICK -= GAME_TICK;
        }
        
        assert_eq!(game_tick(), 0);
        assert_eq!(ticks_until_sample_tick(0), SAMPLE_TICK_MOD);
        assert_eq!(ticks_until_sample_tick(SAMPLE_TICK_MOD), SAMPLE_TICK_MOD);
        assert_eq!(ticks_until_sample_tick(SAMPLE_TICK_MOD + 1), SAMPLE_TICK_MOD + SAMPLE_INTERVAL);
        
        unsafe {
            GAME_TICK += SAMPLE_TICK_MOD;
        }
        
        assert_eq!(ticks_until_sample_tick(0), 0);
        assert_eq!(ticks_until_sample_tick(SAMPLE_TICK_MOD), SAMPLE_INTERVAL);
        assert_eq!(ticks_until_sample_tick(SAMPLE_INTERVAL + 1), 2 * SAMPLE_INTERVAL);
        
        unsafe {
            GAME_TICK += 1;
        }
        
        assert_eq!(ticks_until_sample_tick(0), SAMPLE_INTERVAL - 1);
    }
}