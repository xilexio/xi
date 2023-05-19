use crate::unwrap;
use std::collections::btree_map::Entry as BEntry;
use std::collections::hash_map::Entry as HEntry;
use std::collections::{BTreeMap, HashMap};
use std::hash::{BuildHasher, Hash};

pub trait MultiMapUtils<K, V> {
    fn push_or_insert(&mut self, key: K, value: V);
    fn pop_from_key(&mut self, key: K) -> Option<V>;
}

pub trait MapUtils<K, V> {
    fn get_or_insert<F>(&mut self, key: K, value_generator: F) -> &V
    where
        F: FnOnce() -> V;

    fn get_mut_or_insert<F>(&mut self, key: K, value_generator: F) -> &mut V
    where
        F: FnOnce() -> V;
}

pub trait OrderedMultiMapUtils<K, V> {
    fn pop_from_first(&mut self) -> Option<(K, V)>;
}

impl<K, V, S> MultiMapUtils<K, V> for HashMap<K, Vec<V>, S>
where
    K: Eq + Hash,
    S: BuildHasher,
{
    fn push_or_insert(&mut self, key: K, value: V) {
        match self.entry(key) {
            HEntry::Occupied(mut e) => {
                e.get_mut().push(value);
            }
            HEntry::Vacant(e) => {
                e.insert(vec![value]);
            }
        }
    }

    fn pop_from_key(&mut self, key: K) -> Option<V> {
        match self.entry(key) {
            HEntry::Occupied(mut e) => {
                let result = unwrap!(e.get_mut().pop());
                if e.get().is_empty() {
                    e.remove();
                }
                Some(result)
            }
            HEntry::Vacant(_) => None,
        }
    }
}

impl<K, V, S> MapUtils<K, V> for HashMap<K, V, S>
where
    K: Eq + Hash,
    S: BuildHasher,
{
    fn get_or_insert<F>(&mut self, key: K, value_generator: F) -> &V
    where
        F: FnOnce() -> V,
    {
        match self.entry(key) {
            HEntry::Occupied(e) => e.into_mut(),
            HEntry::Vacant(e) => e.insert(value_generator()),
        }
    }

    fn get_mut_or_insert<F>(&mut self, key: K, value_generator: F) -> &mut V
    where
        F: FnOnce() -> V,
    {
        match self.entry(key) {
            HEntry::Occupied(e) => e.into_mut(),
            HEntry::Vacant(e) => e.insert(value_generator()),
        }
    }
}

impl<K, V> MultiMapUtils<K, V> for BTreeMap<K, Vec<V>>
where
    K: Ord,
{
    fn push_or_insert(&mut self, key: K, value: V) {
        match self.entry(key) {
            BEntry::Occupied(mut e) => {
                e.get_mut().push(value);
            }
            BEntry::Vacant(e) => {
                e.insert(vec![value]);
            }
        }
    }

    fn pop_from_key(&mut self, key: K) -> Option<V> {
        match self.entry(key) {
            BEntry::Occupied(mut e) => {
                let result = unwrap!(e.get_mut().pop());
                if e.get().is_empty() {
                    e.remove();
                }
                Some(result)
            }
            BEntry::Vacant(_) => None,
        }
    }
}

impl<K, V> MapUtils<K, V> for BTreeMap<K, V>
where
    K: Ord,
{
    fn get_or_insert<F>(&mut self, key: K, value_generator: F) -> &V
    where
        F: FnOnce() -> V,
    {
        match self.entry(key) {
            BEntry::Occupied(e) => e.into_mut(),
            BEntry::Vacant(e) => e.insert(value_generator()),
        }
    }

    fn get_mut_or_insert<F>(&mut self, key: K, value_generator: F) -> &mut V
    where
        F: FnOnce() -> V,
    {
        match self.entry(key) {
            BEntry::Occupied(e) => e.into_mut(),
            BEntry::Vacant(e) => e.insert(value_generator()),
        }
    }
}

impl<K, V> OrderedMultiMapUtils<K, V> for BTreeMap<K, Vec<V>>
where
    K: Ord + Clone,
{
    fn pop_from_first(&mut self) -> Option<(K, V)> {
        match self.first_entry() {
            Some(mut e) => {
                let key = e.key().clone();
                let value = unwrap!(e.get_mut().pop());
                if e.get().is_empty() {
                    e.remove();
                }
                Some((key, value))
            }
            None => None,
        }
    }
}
