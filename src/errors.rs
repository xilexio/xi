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
    #[error("object does not exist in the game")]
    ObjectDoesNotExist,
    #[error("failed to scan the room due to lack of visibility")]
    RoomVisibilityError,
    #[error("spawn request tick is in the past")]
    SpawnRequestTickInThePast,
}