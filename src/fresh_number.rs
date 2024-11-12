use crate::utils::random::random;
use num_traits::Pow;
use rustc_hash::FxHashMap;

/// Given a `FxHashMap` with u32 keys, returns a positive u32 that does not belong to the map.
pub fn fresh_number<V>(map: &FxHashMap<u32, V>) -> u32 {
    if !map.contains_key(&(map.len() as u32)) {
        return map.len() as u32;
    }

    let number_limit = 10.0f64.pow(((map.len() * 5 / 4 + 2) as f64).log(10.0).ceil()) - 1.0;

    loop {
        let number = (random() * number_limit) as u32 + 1;
        if !map.contains_key(&number) {
            break number;
        }
    }
}

/// Same as `fresh_number`, but returns 1 when the map does not exist.
pub fn fresh_number_if_some<V>(maybe_map: Option<&FxHashMap<u32, V>>) -> u32 {
    if let Some(map) = maybe_map {
        fresh_number(map)
    } else {
        1
    }
}
