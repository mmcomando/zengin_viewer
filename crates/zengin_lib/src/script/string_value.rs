/// String literal of a daedalus script with its original case, used for
/// player-visible text (dialog lines, choice descriptions).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ShowString {
    string: String,
}

impl ShowString {
    pub fn new(string: impl Into<String>) -> Self {
        Self {
            string: string.into(),
        }
    }

    pub fn string(&self) -> &str {
        &self.string
    }

    pub fn into_string(self) -> String {
        self.string
    }
}

impl std::fmt::Display for ShowString {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.string.fmt(f)
    }
}

/// Lowercased string used as a canonical, case-insensitive lookup key (block
/// names, waypoint names).
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord, Default)]
pub struct StringCanon(String);

impl StringCanon {
    /// Lowercases the value, making all keys canonical.
    pub fn new(value: impl AsRef<str>) -> Self {
        Self(value.as_ref().to_lowercase())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl std::fmt::Display for StringCanon {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.0.fmt(f)
    }
}
