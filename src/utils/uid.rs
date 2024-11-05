use std::cell::Cell;

thread_local! {
    static NEXT_UID: Cell<u32> = const { Cell::new(1) };
}

/// Generic unique identifier.
#[derive(Copy, Clone, Debug, Hash, Eq, PartialEq, Ord, PartialOrd)]
#[repr(transparent)]
pub struct UId(u32);

impl UId {
    pub fn new() -> Self {
        NEXT_UID.with(|next_uid| {
            let uid = next_uid.get();
            next_uid.set(uid + 1);

            UId(uid)
        })
    }
}

impl std::fmt::Display for UId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "U{}", self.0)
    }
}