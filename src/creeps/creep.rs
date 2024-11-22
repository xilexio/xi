use rustc_hash::FxHashMap;
use crate::travel::TravelState;
use crate::{log_err, u};
use screeps::{
    game,
    ConstructionSite,
    HasPosition,
    MaybeHasId,
    MoveToOptions,
    ObjectId,
    PolyStyle,
    Position,
    RawObjectId,
    Resource,
    ResourceType,
    SharedCreepProperties,
    Source,
    Store,
    StructureController,
    Transferable,
    Withdrawable,
    BUILD_POWER,
    HARVEST_POWER,
    UPGRADE_CONTROLLER_POWER,
};
use screeps::Part::Work;
use crate::creeps::creep_body::CreepBody;
use crate::creeps::creep_role::CreepRole;
use crate::errors::XiError;
use crate::errors::XiError::*;
use crate::hauling::transfers::register_transfer;
use crate::utils::get_object_by_id::erased_object_by_id;
use crate::utils::cold::cold;
use crate::utils::game_tick::game_tick;
use crate::utils::single_tick_cache::SingleTickCache;
use crate::utils::unchecked_transferable::UncheckedTransferable;
use crate::utils::unchecked_withdrawable::UncheckedWithdrawable;

type CrId = u32;

#[derive(Debug, Default)]
pub struct Creep {
    /// Globally unique creep name.
    pub name: String,
    /// Creep's Screeps ID.
    pub id: Option<ObjectId<screeps::Creep>>,
    /// Creep role. May not change.
    pub role: CreepRole,
    /// Unique creep identifier, separate for each role.
    pub number: CrId,
    /// State of travel of the creep with information about location where it is supposed to be
    /// and temporary state to be managed by the travel module.
    pub travel_state: TravelState,
    pub last_withdraw_tick: u32,
    pub last_pickup_tick: u32,
    pub last_transfer_tick: u32,
    pub dead: bool,
    pub cached_creep: SingleTickCache<screeps::Creep>,
}

impl Creep {
    // Utility

    pub fn screeps_obj(&mut self) -> Result<&mut screeps::Creep, XiError> {
        if !self.dead {
            Ok(self.cached_creep.get_or_insert_with(|| u!(game::creeps().get(self.name.clone()))))
        } else {
            Err(CreepDead)
        }
    }

    /// Creep's Screeps ID. May fail if the creep is dead or not alive yet, e.g., just registered
    /// and starting to spawn.
    pub fn screeps_id(&mut self) -> Result<ObjectId<screeps::Creep>, XiError> {
        // If the creep is alive, it must have an ID.
        if self.dead {
            Err(CreepDead)
        } else if let Some(id) = self.id {
            Ok(id)
        } else {
            let id = u!(self.screeps_obj()?.try_id());
            self.id = Some(id);
            Ok(id)
        }
    }

    // API wrappers
    
    pub fn body(&mut self) -> Result<CreepBody, XiError> {
        Ok(self.screeps_obj()?.body().into())
    }

    pub fn harvest(&mut self, source: &Source) -> Result<(), XiError> {
        self.screeps_obj()?.harvest(source).or(Err(CreepHarvestFailed))
    }

    pub fn move_to(&mut self, pos: Position) -> Result<(), XiError> {
        let options = MoveToOptions::default().visualize_path_style(PolyStyle::default());
        self.screeps_obj()?.move_to_with_options(pos, Some(options)).or(Err(CreepMoveToFailed))
    }

    pub fn pos(&mut self) -> Result<Position, XiError> {
        Ok(self.screeps_obj()?.pos())
    }

    pub fn public_say(&mut self, message: &str) -> Result<(), XiError> {
        self.screeps_obj()?.say(message, true).or(Err(CreepSayFailed))
    }

    pub fn suicide(&mut self) -> Result<(), XiError> {
        self.screeps_obj()?.suicide().or(Err(CreepSuicideFailed))
    }

    /// Zero indicates a dead creep.
    pub fn ticks_to_live(&mut self) -> u32 {
        let obj = self.screeps_obj();
        match obj {
            Ok(creep) => creep.ticks_to_live().unwrap_or(0),
            Err(CreepDead) => 0,
            Err(_) => {
                cold();
                log_err!(obj);
                0
            }
        }
    }
    
    pub fn spawning(&mut self) -> bool {
        let obj = self.screeps_obj();
        match obj {
            Ok(creep) => creep.spawning(),
            Err(CreepDead) => false,
            Err(_) => {
                cold();
                log_err!(obj);
                false
            }
        }
    }

    pub fn withdraw<T>(&mut self, target_id: ObjectId<T>, target: &T, resource_type: ResourceType, amount: u32) -> Result<(), XiError>
    where
        T: Withdrawable,
    {
        self.screeps_obj()?.withdraw(target, resource_type, Some(amount)).or(Err(CreepWithdrawFailed))?;
        register_transfer(target_id.into(), resource_type, -(amount as i32));
        register_transfer(self.screeps_id()?.into(), resource_type, amount as i32);
        self.last_withdraw_tick = game_tick();
        Ok(())
    }

    pub fn unchecked_withdraw(&mut self, target_id: RawObjectId, resource_type: ResourceType, amount: u32) -> Result<(), XiError> {
        let target = erased_object_by_id(target_id)?;
        let unchecked_target = UncheckedWithdrawable(&target);
        self.withdraw(target_id.into(), &unchecked_target, resource_type, amount)
    }

    pub fn pickup(&mut self, target: &Resource) -> Result<(), XiError> {
        // TODO Register the change within this creep and the pile.
        self.screeps_obj()?.pickup(target).or(Err(CreepPickupFailed))?;
        self.last_pickup_tick = game_tick();
        Ok(())
    }

    pub fn transfer<T>(&mut self, target_id: ObjectId<T>, target: &T, resource_type: ResourceType, amount: u32) -> Result<(), XiError>
    where
        T: Transferable
    {
        self.screeps_obj()?.transfer(target, resource_type, Some(amount)).or(Err(CreepTransferFailed))?;
        register_transfer(target_id.into(), resource_type, amount as i32);
        register_transfer(self.screeps_id()?.into(), resource_type, -(amount as i32));
        self.last_transfer_tick = game_tick();
        Ok(())
    }

    pub fn unchecked_transfer(&mut self, target_id: RawObjectId, resource_type: ResourceType, amount: u32) -> Result<(), XiError> {
        let target = erased_object_by_id(target_id)?;
        let unchecked_target = UncheckedTransferable(&target);
        self.transfer(target_id.into(), &unchecked_target, resource_type, amount)
    }

    pub fn drop(&mut self, resource_type: ResourceType, amount: u32) -> Result<(), XiError> {
        self.screeps_obj()?.drop(resource_type, Some(amount)).or(Err(CreepDropFailed))?;
        register_transfer(self.screeps_id()?.into(), resource_type, -(amount as i32));
        Ok(())
    }

    pub fn upgrade_controller(&mut self, controller: &StructureController) -> Result<(), XiError> {
        self.screeps_obj()?.upgrade_controller(controller).or(Err(CreepUpgradeControllerFailed))
    }

    pub fn build(&mut self, construction_site: &ConstructionSite) -> Result<(), XiError> {
        self.screeps_obj()?.build(construction_site).or(Err(CreepBuildFailed))
    }

    pub fn store(&mut self) -> Result<Store, XiError> {
        Ok(self.screeps_obj()?.store())
    }
    
    // Store wrappers
    
    pub fn carry_capacity(&mut self) -> Result<u32, XiError> {
        Ok(self.screeps_obj()?.store().get_capacity(None))
    }
    
    pub fn used_capacity(&mut self, resource_type: Option<ResourceType>) -> Result<u32, XiError> {
        let used = self.screeps_obj()?.store().get_used_capacity(resource_type);
        // TODO
        Ok(used as u32)
    }
    
    pub fn free_capacity(&mut self) -> Result<u32, XiError> {
        // TODO
        Ok(self.screeps_obj()?.store().get_free_capacity(None) as u32)
    }
    
    pub fn capacity_usage(&mut self) -> Result<FxHashMap<ResourceType, u32>, XiError> {
        // TODO Transfers in progress.
        let store = self.screeps_obj()?.store();
        Ok(store
            .store_types()
            .into_iter()
            .map(|resource_type| (resource_type, store.get_used_capacity(Some(resource_type))))
            .collect())
    }

    // Statistics

    pub fn energy_harvest_power(&mut self) -> Result<u32, XiError> {
        Ok(self.screeps_obj()?
            .body()
            .iter()
            .filter_map(|body_part| (body_part.part() == Work).then_some(HARVEST_POWER))
            .sum())
    }

    pub fn upgrade_energy_consumption(&mut self) -> Result<u32, XiError> {
        Ok(self.screeps_obj()?
            .body()
            .iter()
            .filter_map(|body_part| (body_part.part() == Work).then_some(UPGRADE_CONTROLLER_POWER))
            .sum())
    }

    pub fn build_energy_consumption(&mut self) -> Result<u32, XiError> {
        Ok(self.screeps_obj()?
            .body()
            .iter()
            .filter_map(|body_part| (body_part.part() == Work).then_some(BUILD_POWER))
            .sum())
    }
}