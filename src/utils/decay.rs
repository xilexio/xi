use screeps::{
    StructureType,
    CONTAINER_DECAY,
    CONTAINER_DECAY_TIME,
    CONTAINER_DECAY_TIME_OWNED,
    PORTAL_DECAY,
    POWER_BANK_DECAY,
    RAMPART_DECAY_AMOUNT,
    RAMPART_DECAY_TIME,
    ROAD_DECAY_AMOUNT,
    ROAD_DECAY_TIME,
};

pub trait DecayInfo {
    /// Amount of hits the structure will lose upon decay or `None` if it does not decay or lose
    /// hits upon decay. `Portal` and `PowerBank` do not have hits, but still decay.
    fn decay_amount(&self) -> Option<u32>;

    /// The number of ticks after which the structure decays since its creation or last decay.
    /// `None` if the structure does not decay.
    /// The `owned` argument should be true if the structure is in an owned room.
    /// `StructureContainer` decays at a different rate in an owned room; the argument does not
    /// matter for other structures.
    fn decay_ticks(&self, owned: bool) -> Option<u32>;
    
    /// Average number of hits decaying per tick for structures that decay and have hits.
    fn average_decay_hits_per_tick(&self, owned: bool) -> Option<f32> {
        Some(self.decay_amount()? as f32 / self.decay_ticks(owned)? as f32)
    }
}

impl DecayInfo for StructureType {
    fn decay_amount(&self) -> Option<u32> {
        match self {
            StructureType::Road => Some(ROAD_DECAY_AMOUNT),
            StructureType::Rampart => Some(RAMPART_DECAY_AMOUNT),
            StructureType::Container => Some(CONTAINER_DECAY),
            _ => None,
        }
    }

    fn decay_ticks(&self, owned: bool) -> Option<u32> {
        match self {
            StructureType::Road => Some(ROAD_DECAY_TIME),
            StructureType::Rampart => Some(RAMPART_DECAY_TIME),
            StructureType::Portal => Some(PORTAL_DECAY),
            StructureType::PowerBank => Some(POWER_BANK_DECAY),
            StructureType::Container => if owned {
                Some(CONTAINER_DECAY_TIME_OWNED)
            } else {
                Some(CONTAINER_DECAY_TIME)
            },
            _ => None,
        }
    }
}