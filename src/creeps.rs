use std::collections::hash_map::Entry;
use rustc_hash::FxHashMap;
use screeps::game;
use crate::creep::Creep;

pub struct CreepManager {
    creeps: FxHashMap<String, Creep>
}

impl CreepManager {
    pub fn pre_tick(&mut self) {
        let game_creeps = game::creeps();

        for creep_name in game_creeps.keys() {
            match self.creeps.entry(creep_name) {
                Entry::Occupied(_) => {}
                Entry::Vacant(_) => {
                    // The creep is not registered in the bot. Most likely it is freshly after a reset.
                    // TODO register the creep
                }
            }
        }

        for creep_name in self.creeps.keys() {
            if game_creeps.get(creep_name.clone()).is_none() {
                // The creep is dead.
                // TODO inform its process
            }
        }
    }
}