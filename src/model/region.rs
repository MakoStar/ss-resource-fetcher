use std::borrow::Borrow;
use std::fmt;

#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Region(
    /// 区域名
    String,
);

impl Region {
    pub fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for Region {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl Borrow<str> for Region {
    fn borrow(&self) -> &str {
        &self.0
    }
}

impl AsRef<str> for Region {
    fn as_ref(&self) -> &str {
        &self.0
    }
}

impl From<&str> for Region {
    fn from(value: &str) -> Self {
        Self::new(value)
    }
}

impl From<String> for Region {
    fn from(value: String) -> Self {
        Self::new(value)
    }
}

impl From<&String> for Region {
    fn from(value: &String) -> Self {
        Self::new(value.clone())
    }
}
