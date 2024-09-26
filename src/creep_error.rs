use thiserror::Error;

#[derive(Error, Debug, Copy, Clone)]
pub enum CreepError {
    #[error("creep died before its task was completed")]
    CreepDead,
    #[error("creep failed to pickup a resource")]
    CreepPickupFailed,
    #[error("creep failed to store a resource")]
    CreepTransferFailed,
}