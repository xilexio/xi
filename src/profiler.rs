use log::debug;
use screeps::game;

pub fn measure_time<F, R>(name: &str, f: F) -> R
where
    F: FnOnce() -> R,
{
    let start = game::cpu::get_used();
    let result = f();
    let end = game::cpu::get_used();
    // TODO stack
    debug!(
        "<span style=\"color: #6666bb\">{} completed in {}ms.</span>",
        name,
        end - start
    );
    result
}
