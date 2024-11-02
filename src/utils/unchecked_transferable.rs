use screeps::{RoomObject, Transferable};

pub struct UncheckedTransferable<'a>(pub &'a RoomObject);

impl<'a> AsRef<RoomObject> for UncheckedTransferable<'a> {
    fn as_ref(&self) -> &RoomObject {
        self.0
    }
}

impl<'a> Transferable for UncheckedTransferable<'a> {}