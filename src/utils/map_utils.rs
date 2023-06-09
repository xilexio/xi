use crate::u;
use std::collections::btree_map::Entry as BEntry;
use std::collections::hash_map::Entry as HEntry;
use std::collections::{BTreeMap, HashMap, HashSet};
use std::hash::{BuildHasher, Hash};
use std::iter::once;

pub trait MultiMapUtils<K, V> {
    fn push_or_insert(&mut self, key: K, value: V);
    fn pop_from_key(&mut self, key: K) -> Option<V>;
}

pub trait OrderedMultiMapUtils<K, V> {
    fn pop_from_first(&mut self) -> Option<(K, V)>;
    fn pop_from_last(&mut self) -> Option<(K, V)>;
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
                let result = u!(e.get_mut().pop());
                if e.get().is_empty() {
                    e.remove();
                }
                Some(result)
            }
            HEntry::Vacant(_) => None,
        }
    }
}

impl<K, V, S1, S2> MultiMapUtils<K, V> for HashMap<K, HashSet<V, S2>, S1>
where
    K: Eq + Hash,
    V: Eq + Hash + Clone,
    S1: BuildHasher,
    S2: Default + BuildHasher,
{
    fn push_or_insert(&mut self, key: K, value: V) {
        match self.entry(key) {
            HEntry::Occupied(mut e) => {
                e.get_mut().insert(value);
            }
            HEntry::Vacant(e) => {
                e.insert(once(value).collect());
            }
        }
    }

    fn pop_from_key(&mut self, key: K) -> Option<V> {
        match self.entry(key) {
            HEntry::Occupied(mut e) => {
                let e_mut = e.get_mut();

                let value = u!(e_mut.iter().next()).clone();
                e_mut.remove(&value);
                if e.get().is_empty() {
                    e.remove();
                }
                Some(value)
            }
            HEntry::Vacant(_) => None,
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
                let result = u!(e.get_mut().pop());
                if e.get().is_empty() {
                    e.remove();
                }
                Some(result)
            }
            BEntry::Vacant(_) => None,
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
                let value = u!(e.get_mut().pop());
                if e.get().is_empty() {
                    e.remove();
                }
                Some((key, value))
            }
            None => None,
        }
    }

    fn pop_from_last(&mut self) -> Option<(K, V)> {
        match self.last_entry() {
            Some(mut e) => {
                let key = e.key().clone();
                let value = u!(e.get_mut().pop());
                if e.get().is_empty() {
                    e.remove();
                }
                Some((key, value))
            }
            None => None,
        }
    }
}
