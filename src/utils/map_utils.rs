use std::collections::{HashMap};
use std::hash::{BuildHasher, Hash};

pub trait MapUtils<K, V> {
    fn insert_or_remove(&mut self, key: K, value: Option<V>);
}

impl <K, V, S> MapUtils<K, V> for HashMap<K, V, S>
where
    K: Eq + Hash,
    S: BuildHasher,
{
    fn insert_or_remove(&mut self, key: K, value: Option<V>) {
        match value {
            Some(value) => self.insert(key, value),
            None => self.remove(&key),
        };
    }
}