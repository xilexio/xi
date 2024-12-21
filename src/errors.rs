use log::warn;
use thiserror::Error;

#[derive(Error, Debug, Copy, Clone)]
pub enum XiError {
    #[error("creep died before its task was completed")]
    CreepDead,
    #[error("creep failed to pickup a resource")]
    CreepPickupFailed,
    #[error("creep failed to store a resource")]
    CreepTransferFailed,
    #[error("creep failed to withdraw a resource")]
    CreepWithdrawFailed,
    #[error("creep failed to drop a resource")]
    CreepDropFailed,
    #[error("creep failed to harvest a source")]
    CreepHarvestFailed,
    #[error("creep movement to target failed")]
    CreepMoveToFailed,
    #[error("creep say failed")]
    CreepSayFailed,
    #[error("creep suicide failed")]
    CreepSuicideFailed,
    #[error("creep failed to upgrade the controller")]
    CreepUpgradeControllerFailed,
    #[error("creep failed to build the construction site")]
    CreepBuildFailed,
    #[error("creep failed to repair a structure")]
    CreepRepairFailed,
    #[error("creep failed to claim a controller")]
    CreepClaimFailed,
    #[error("object does not exist in the game")]
    ObjectDoesNotExist,
    #[error("failed to scan the room due to lack of visibility")]
    RoomVisibilityError,
    #[error("spawn request tick is in the past")]
    SpawnRequestTickInThePast,
    #[error("path not found")]
    PathNotFound,
}

impl XiError {
    pub fn warn(&self, description: &str) {
        warn!("{}: {:?}.", description, self);
    }
}