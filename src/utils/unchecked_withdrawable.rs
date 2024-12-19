use screeps::{RoomObject, Withdrawable};

pub struct UncheckedWithdrawable<'a>(pub &'a RoomObject);

impl AsRef<RoomObject> for UncheckedWithdrawable<'_> {
    fn as_ref(&self) -> &RoomObject {
        self.0
    }
}

impl Withdrawable for UncheckedWithdrawable<'_> {}