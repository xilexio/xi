use log::warn;
use rustc_hash::FxHashMap;
use crate::travel::travel_state::TravelState;
use crate::{log_err, u};
use screeps::{game, ConstructionSite, Direction, HasId, MaybeHasId, MoveToOptions, ObjectId, PolyStyle, Position, RawObjectId, Repairable, Resource, ResourceType, SharedCreepProperties, Source, StructureController, Transferable, Withdrawable};
use crate::creeps::creep_body::CreepBody;
use crate::creeps::creep_role::CreepRole;
use crate::creeps::generic_creep::GenericCreep;
use crate::errors::XiError;
use crate::errors::XiError::*;
use crate::hauling::transfers::{
    get_free_capacity_with_object,
    get_used_capacities_with_object,
    get_used_capacity_with_object,
    register_transfer,
    TransferStage
};
use crate::travel::surface::Surface;
use crate::utils::get_object_by_id::erased_object_by_id;
use crate::utils::cold::cold;
use crate::utils::game_tick::game_tick;
use crate::utils::single_tick_cache::SingleTickCache;
use crate::utils::unchecked_transferable::UncheckedTransferable;
use crate::utils::unchecked_withdrawable::UncheckedWithdrawable;

pub type CrId = u32;

// TODO A creep should have its room assigned.
// TODO To enable assigning that upon restart, make a function to find the nearest room.
#[derive(Debug)]
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
    pub body: CreepBody,
    /// The number of ticks it takes for the creep to move one tile.
    /// MAX means the creep is immovable.
    /// Due to the limit of 50 parts and swamp being the worst, the maximum for a movable creep
    /// is 49 * 5 = 245.
    pub ticks_per_tile: [u8; 3],
    pub cached_screeps_obj: SingleTickCache<screeps::Creep>,
}

impl Creep {
    pub fn new(
        name: String,
        id: Option<ObjectId<screeps::Creep>>,
        role: CreepRole,
        number: CrId,
        body: CreepBody
    ) -> Self {
        let ticks_per_tile = [
            body.ticks_per_tile(Surface::Road),
            body.ticks_per_tile(Surface::Plain),
            body.ticks_per_tile(Surface::Swamp),
        ];
        
        // This is an invalid position, but it does not matter as creeps are registered before
        // updating their position.
        let pos = Position::from_packed(0);

        Creep {
            name,
            id,
            role,
            number,
            travel_state: TravelState::new(pos),
            last_withdraw_tick: 0,
            last_pickup_tick: 0,
            last_transfer_tick: 0,
            dead: false,
            body,
            ticks_per_tile: ticks_per_tile.map(|x| x),
            cached_screeps_obj: SingleTickCache::default(),
        }
    }
    
    // Utility

    pub fn screeps_obj(&mut self) -> Result<&mut screeps::Creep, XiError> {
        if !self.dead {
            Ok(self.cached_screeps_obj.get_or_insert_with(|| u!(game::creeps().get(self.name.clone()))))
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

    pub fn role_id(&self) -> (CreepRole, CrId) {
        (self.role, self.number)
    }

    // Actions performed by the creep
    
    pub fn harvest(&mut self, source: &Source) -> Result<(), XiError> {
        self.screeps_obj()?.harvest(source).or(Err(CreepHarvestFailed))
    }

    pub fn move_to(&mut self, pos: Position) -> Result<(), XiError> {
        let options = MoveToOptions::default().visualize_path_style(PolyStyle::default());
        self.screeps_obj()?.move_to_with_options(pos, Some(options)).or(Err(CreepMoveToFailed))
    }
    
    pub fn move_direction(&mut self, direction: Direction) -> Result<(), XiError> {
        self.screeps_obj()?.move_direction(direction).or(Err(CreepMoveToFailed))
    }

    pub fn public_say(&mut self, message: &str) -> Result<(), XiError> {
        self.screeps_obj()?.say(message, true).or(Err(CreepSayFailed))
    }

    pub fn suicide(&mut self) -> Result<(), XiError> {
        self.screeps_obj()?.suicide().or(Err(CreepSuicideFailed))
    }
    
    pub fn withdraw<T>(&mut self, target_id: ObjectId<T>, target: &T, resource_type: ResourceType, amount: u32, limited_transfer: bool) -> Result<(), XiError>
    where
        T: Withdrawable,
    {
        if let Err(e) = self.screeps_obj()?.withdraw(target, resource_type, limited_transfer.then_some(amount)) {
            warn!(
                "Creep {} withdraw of {} {} from {} failed: {:?}.",
                self.name,
                amount,
                resource_type,
                target_id,
                e
            );
            return Err(CreepWithdrawFailed);
        }
        
        register_transfer(target_id.into(), resource_type, -(amount as i32));
        register_transfer(self.screeps_id()?.into(), resource_type, amount as i32);
        self.last_withdraw_tick = game_tick();
        Ok(())
    }

    pub fn unchecked_withdraw(&mut self, target_id: RawObjectId, resource_type: ResourceType, amount: u32, limited_transfer: bool) -> Result<(), XiError> {
        let target = erased_object_by_id(target_id)?;
        let unchecked_target = UncheckedWithdrawable(&target);
        self.withdraw(target_id.into(), &unchecked_target, resource_type, amount, limited_transfer)
    }

    pub fn pickup(&mut self, target: &Resource) -> Result<(), XiError> {
        // TODO Register the change within this creep and the pile.
        if let Err(e) = self.screeps_obj()?.pickup(target) {
            warn!(
                "Creep {} pickup of {} failed: {:?}.",
                self.name,
                target.id(),
                e
            );
            return Err(CreepPickupFailed);
        }
        
        self.last_pickup_tick = game_tick();
        Ok(())
    }

    pub fn transfer<T>(&mut self, target_id: ObjectId<T>, target: &T, resource_type: ResourceType, amount: u32, limited_transfer: bool) -> Result<(), XiError>
    where
        T: Transferable
    {
        if let Err(e) = self.screeps_obj()?.transfer(target, resource_type, limited_transfer.then_some(amount)) {
            warn!(
                "Creep {} transfer of {} {} to {} failed: {:?}.",
                self.name,
                amount,
                resource_type,
                target_id,
                e
            );
            return Err(CreepTransferFailed);
        }
        
        register_transfer(target_id.into(), resource_type, amount as i32);
        register_transfer(self.screeps_id()?.into(), resource_type, -(amount as i32));
        self.last_transfer_tick = game_tick();
        Ok(())
    }

    pub fn unchecked_transfer(&mut self, target_id: RawObjectId, resource_type: ResourceType, amount: u32, limited_transfer: bool) -> Result<(), XiError> {
        let target = erased_object_by_id(target_id)?;
        let unchecked_target = UncheckedTransferable(&target);
        self.transfer(target_id.into(), &unchecked_target, resource_type, amount, limited_transfer)
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
    
    pub fn repair<T>(&mut self, target: &T) -> Result<(), XiError>
    where
        T: ?Sized + Repairable
    {
        self.screeps_obj()?.repair(target).or(Err(CreepRepairFailed))
    }
    
    pub fn claim(&mut self, target: &StructureController) -> Result<(), XiError> {
        self.screeps_obj()?.claim_controller(target).or(Err(CreepClaimFailed))
    }
    
    // Current information about the creep

    pub fn fatigue(&mut self) -> Result<u32, XiError> {
        Ok(self.screeps_obj()?.fatigue())
    }
    
    pub fn carry_capacity(&mut self) -> Result<u32, XiError> {
        Ok(self.screeps_obj()?.store().get_capacity(None))
    }
    
    pub fn used_capacity(&mut self, resource_type: Option<ResourceType>, transfer_stage: TransferStage) -> Result<u32, XiError> {
        let id = self.screeps_id()?;
        let obj = self.screeps_obj()?;
        Ok(get_used_capacity_with_object(obj, id.into(), resource_type, transfer_stage))
    }
    
    pub fn free_capacity(&mut self, transfer_stage: TransferStage) -> Result<u32, XiError> {
        let id = self.screeps_id()?;
        let obj = self.screeps_obj()?;
        Ok(get_free_capacity_with_object(obj, id.into(), None, transfer_stage))
    }
    
    pub fn used_capacities(&mut self, transfer_stage: TransferStage) -> Result<FxHashMap<ResourceType, u32>, XiError> {
        let id = self.screeps_id()?;
        let obj = self.screeps_obj()?;
        Ok(get_used_capacities_with_object(obj, id.into(), transfer_stage))
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

    // Statistics based on the body alone

    pub fn energy_harvest_power(&mut self) -> u32 {
        self.body.energy_harvest_power()
    }

    pub fn upgrade_energy_consumption(&mut self) -> u32 {
        self.body.upgrade_energy_usage()
    }

    pub fn build_energy_consumption(&mut self) -> u32 {
        self.body.build_energy_usage()
    }
}

impl GenericCreep for Creep {
    fn get_name(&self) -> &String {
        &self.name
    }
    
    fn get_screeps_id(&mut self) -> Result<ObjectId<screeps::Creep>, XiError> {
        self.screeps_id()
    }

    fn get_travel_state(&self) -> &TravelState {
        &self.travel_state
    }

    fn get_travel_state_mut(&mut self) -> &mut TravelState {
        &mut self.travel_state
    }

    fn get_ticks_per_tile(&self, surface: Surface) -> u8 {
        self.ticks_per_tile[surface as usize]
    }

    fn get_fatigue(&mut self) -> Result<u32, XiError> {
        self.fatigue()
    }
}