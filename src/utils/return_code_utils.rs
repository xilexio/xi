use log::warn;
use screeps::ReturnCode;
use std::fmt::Debug;

pub trait ReturnCodeUtils
where
    Self: Debug + PartialEq<ReturnCode>,
{
    fn to_bool_and_warn(&self, description: &str) -> bool {
        if *self == ReturnCode::Ok {
            true
        } else {
            warn!("{}: {:?}.", description, self);
            false
        }
    }
}

impl ReturnCodeUtils for ReturnCode {}
