use screeps::{RoomObject, Withdrawable};

pub struct UncheckedWithdrawable<'a>(pub &'a RoomObject);

impl<'a> AsRef<RoomObject> for UncheckedWithdrawable<'a> {
    fn as_ref(&self) -> &RoomObject {
        self.0
    }
}

impl<'a> Withdrawable for UncheckedWithdrawable<'a> {}