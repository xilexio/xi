use thiserror::Error;

#[derive(Error, Debug, Copy, Clone)]
pub enum CreepError {
    #[error("creep died before its task was completed")]
    CreepDead,
}