use std::cell::Cell;

thread_local! {
    static NEXT_UID: Cell<u32> = const { Cell::new(1) };
}

/// Generic unique identifier with a single character display prefix.
#[derive(Copy, Clone, Debug, Hash, Eq, PartialEq, Ord, PartialOrd)]
#[repr(transparent)]
pub struct UId<const N: char='U'>(u32);

impl<const N: char> UId<N> {
    pub fn new() -> Self {
        NEXT_UID.with(|next_uid| {
            let uid = next_uid.get();
            next_uid.set(uid + 1);

            UId(uid)
        })
    }
}

impl<const N: char> std::fmt::Display for UId<N> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}{}", N, self.0)
    }
}