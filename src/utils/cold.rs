/// A no-op function marking that the branch of code it is in is cold, i.e., unlikely to be executed.
/// The branch it will end up in will be very costly, so it should be only used on blocks of code that run up to a few
/// times per restart.
#[inline]
#[cold]
pub fn cold() {}