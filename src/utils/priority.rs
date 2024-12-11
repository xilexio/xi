use serde::{Deserialize, Serialize};

/// Generic priority. Higher is more important.
#[derive(Copy, Clone, Debug, Default, Eq, PartialEq, Ord, PartialOrd, Deserialize, Serialize)]
#[repr(transparent)]
pub struct Priority(pub u8);

impl Priority {
    pub fn saturating_sub(self, rhs: u8) -> Self {
        Self(self.0.saturating_sub(rhs))
    }

    pub fn saturating_add(self, rhs: u8) -> Self {
        Self(self.0.saturating_add(rhs))
    }
}

impl std::ops::Sub<u8> for Priority {
    type Output = Self;

    fn sub(self, rhs: u8) -> Self::Output {
        self.saturating_sub(rhs)
    }
}

impl std::ops::Add<u8> for Priority {
    type Output = Self;

    fn add(self, rhs: u8) -> Self::Output {
        self.saturating_add(rhs)
    }
}

impl std::fmt::Display for Priority {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "!{}", self.0)
    }
}