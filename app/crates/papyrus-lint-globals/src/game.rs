//! The game whose Papyrus dialect and runtime APIs a project targets.

use std::fmt;
use std::str::FromStr;

use serde::{Deserialize, Serialize};

/// Error used when code reaches a recognized target that is not implemented.
pub const UNSUPPORTED_GAME_MESSAGE: &str = "Starfield is not supported yet";

/// The game whose Papyrus dialect and runtime APIs a project targets.
///
/// Serialized as the lowercase config/cache key (`skyrim`, `fallout4`,
/// `starfield`). Skyrim remains the default so configurations created
/// before this key existed, and call sites that have not yet selected a
/// game, keep their previous behaviour.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Game {
    #[default]
    Skyrim,
    Fallout4,
    Starfield,
}

impl Game {
    /// Every recognized target, including targets that are not supported yet,
    /// in configuration and JSON-schema order.
    pub const ALL: [Game; 3] = [Game::Skyrim, Game::Fallout4, Game::Starfield];

    /// The lowercase key used in YAML, cache paths, and schema enums.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Skyrim => "skyrim",
            Self::Fallout4 => "fallout4",
            Self::Starfield => "starfield",
        }
    }

    /// Panics when this target is not supported by the linter yet.
    pub fn assert_supported(self) {
        if self == Self::Starfield {
            panic!("{UNSUPPORTED_GAME_MESSAGE}");
        }
    }

    /// Whether this game's Papyrus dialect includes Fallout 4's extensions
    /// (`Struct`/`Group`, `DebugOnly`/`BetaOnly`, `New <StructName>`).
    /// Starfield's language is a continuation of that dialect.
    pub fn has_fallout4_extensions(self) -> bool {
        matches!(self, Self::Fallout4 | Self::Starfield)
    }
}

impl fmt::Display for Game {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

impl FromStr for Game {
    type Err = ParseGameError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value {
            "skyrim" => Ok(Self::Skyrim),
            "fallout4" => Ok(Self::Fallout4),
            "starfield" => Ok(Self::Starfield),
            _ => Err(ParseGameError),
        }
    }
}

/// Failed to parse a [`Game`] from a config/cache key.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ParseGameError;

impl fmt::Display for ParseGameError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("unknown game; expected skyrim, fallout4, or starfield")
    }
}

impl std::error::Error for ParseGameError {}

#[cfg(test)]
#[path = "game_tests.rs"]
mod tests;
