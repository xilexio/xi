use crate::game_tick::game_tick;
use crate::u;

#[derive(Debug)]
pub struct SingleTickCache<T> {
    pub data: Option<T>,
    pub cache_tick: u32,
}

impl<T> Default for SingleTickCache<T> {
    fn default() -> SingleTickCache<T> {
        Self {
            data: None,
            cache_tick: 0,
        }
    }
}

impl<T> SingleTickCache<T> {
    pub fn get_or_insert_with(&mut self, f: impl FnOnce() -> T) -> &mut T {
        let current_tick = game_tick();
        if current_tick != self.cache_tick {
            self.data = Some(f());
            self.cache_tick = current_tick;
        }
        u!(self.data.as_mut())
    }
}