use num_traits::Pow;
use rustc_hash::FxHashMap;
use crate::random::random;

pub fn fresh_number<V>(map: &FxHashMap<u32, V>) -> u32 {
    let pid_limit = 10.0f64.pow(((map.len() * 5 / 4 + 2) as f64).log(10.0).ceil()) - 1.0;

    loop {
        let pid = (random() * pid_limit) as u32 + 1;
        if !map.contains_key(&pid) {
            break pid;
        }
    }
}