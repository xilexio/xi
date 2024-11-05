use std::cell::Cell;

thread_local! {
    static NEXT_CID: Cell<CId> = const { Cell::new(CId(1)) };
}

/// Condition Identifier.
#[derive(Copy, Clone, Debug, Eq, PartialEq, Ord, PartialOrd, Hash)]
#[repr(transparent)]
pub struct CId(u32);

impl CId {
    pub fn new() -> Self {
        // Assuming this will never overflow.
        let cid = NEXT_CID.get();
        NEXT_CID.replace(CId(cid.0 + 1));
        cid
    }
}

impl std::fmt::Display for CId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "C{}", self.0)
    }
}
