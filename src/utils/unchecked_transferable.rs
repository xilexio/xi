use screeps::{RoomObject, Transferable};

pub struct UncheckedTransferable<'a>(pub &'a RoomObject);

impl AsRef<RoomObject> for UncheckedTransferable<'_> {
    fn as_ref(&self) -> &RoomObject {
        self.0
    }
}

impl Transferable for UncheckedTransferable<'_> {}