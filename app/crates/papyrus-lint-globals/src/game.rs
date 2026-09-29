//! The game whose Papyrus dialect and runtime APIs a project targets.

use std::fmt;
use std::str::FromStr;

use serde::{Deserialize, Serialize};

/// The game whose Papyrus dialect and runtime APIs a project targets.
///
/// Serialized as the lowercase config/cache key (`skyrim`, `legacy`,
/// `fallout4`, `starfield`). Skyrim remains the default so configurations
/// created before this key existed, and call sites that have not yet
/// selected a game, keep their previous behaviour.
///
/// [`Game::Legacy`] is Skyrim Legendary Edition: the same Papyrus dialect
/// and lint policy as [`Game::Skyrim`] (SE/AE), with the LE Creation Kit
/// and SKSE script archives instead of the SE ones.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Game {
    #[default]
    Skyrim,
    Legacy,
    Fallout4,
    Starfield,
}

impl Game {
    /// Every recognized target, in configuration and JSON-schema order.
    pub const ALL: [Game; 4] = [Game::Skyrim, Game::Legacy, Game::Fallout4, Game::Starfield];

    /// The lowercase key used in YAML, cache paths, and schema enums.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Skyrim => "skyrim",
            Self::Legacy => "legacy",
            Self::Fallout4 => "fallout4",
            Self::Starfield => "starfield",
        }
    }

    /// Short label for diagnostics, cache build logs, and pickers.
    pub const fn display_name(self) -> &'static str {
        match self {
            Self::Skyrim => "Skyrim",
            Self::Legacy => "Skyrim LE",
            Self::Fallout4 => "Fallout 4",
            Self::Starfield => "Starfield",
        }
    }

    /// Identifier prefix used by generated per-game static tables
    /// (`SKYRIM_NATIVE_METHODS`, `LEGACY_NATIVE_GLOBALS`, …).
    pub const fn ident_prefix(self) -> &'static str {
        match self {
            Self::Skyrim => "SKYRIM",
            Self::Legacy => "LEGACY",
            Self::Fallout4 => "FALLOUT4",
            Self::Starfield => "STARFIELD",
        }
    }

    /// Game whose hand-maintained rule tables this target shares.
    ///
    /// Legacy Edition uses Skyrim SE's curated lists (deprecated / forbidden /
    /// slow functions, actor values, update-event pairs) so those YAML files
    /// are not copied. Catalog data extracted from bundled `.psc` archives
    /// still uses `self`, because LE and SE ship different base scripts.
    pub const fn rules_game(self) -> Self {
        match self {
            Self::Legacy => Self::Skyrim,
            other => other,
        }
    }

    /// Vanilla Creation Kit archive under `shared/scripts/`.
    pub const fn base_scripts_archive(self) -> &'static str {
        match self {
            Self::Skyrim => "skyrim-scripts.zip",
            Self::Legacy => "legacy-scripts.zip",
            Self::Fallout4 => "fallout4-scripts.zip",
            Self::Starfield => "starfield-scripts.zip",
        }
    }

    /// Script-extender archive under `shared/scripts/`. Starfield has no
    /// extender zip yet; callers treat a missing file as an empty catalog.
    pub const fn extender_scripts_archive(self) -> &'static str {
        match self {
            Self::Skyrim => "skyrim-extender-scripts.zip",
            Self::Legacy => "legacy-extender-scripts.zip",
            Self::Fallout4 => "fallout4-extender-scripts.zip",
            Self::Starfield => "starfield-extender-scripts.zip",
        }
    }

    /// Archives compiled into this game's bundled AST/token blob, vanilla
    /// first so an extender script of the same `ScriptName` wins.
    pub const fn bundled_script_archives(self) -> &'static [&'static str] {
        match self {
            Self::Skyrim => &["skyrim-scripts.zip", "skyrim-extender-scripts.zip"],
            Self::Legacy => &["legacy-scripts.zip", "legacy-extender-scripts.zip"],
            Self::Fallout4 => &["fallout4-scripts.zip", "fallout4-extender-scripts.zip"],
            Self::Starfield => &["starfield-scripts.zip"],
        }
    }

    /// Relative path under `shared/rules/data/` for deprecated-function
    /// comments applied while compiling the bundled AST blob. `None` when
    /// this family has no such list.
    pub const fn deprecated_functions_data(self) -> Option<&'static str> {
        match self.rules_game() {
            Self::Skyrim | Self::Legacy => Some("skyrim/deprecated-functions.yaml"),
            Self::Fallout4 => Some("fallout4/deprecated-functions.yaml"),
            Self::Starfield => None,
        }
    }

    /// `OUT_DIR` filename for this game's gzip-compressed bundled AST blob.
    pub const fn ast_cache_blob_file(self) -> &'static str {
        match self {
            Self::Skyrim => "skyrim-ast-cache.bin.gz",
            Self::Legacy => "legacy-ast-cache.bin.gz",
            Self::Fallout4 => "fallout4-ast-cache.bin.gz",
            Self::Starfield => "starfield-ast-cache.bin.gz",
        }
    }

    /// Every [`Game`] variant is a supported analysis target.
    #[allow(clippy::unused_self)]
    pub fn assert_supported(self) {}

    /// Whether this game's Papyrus dialect includes Fallout 4's extensions
    /// (`Struct`/`Group`, `DebugOnly`/`BetaOnly`, `New <StructName>`).
    /// Starfield's language is a continuation of that dialect. Legacy and
    /// Skyrim SE share Skyrim's dialect.
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
            "legacy" => Ok(Self::Legacy),
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
        f.write_str("unknown game; expected skyrim, legacy, fallout4, or starfield")
    }
}

impl std::error::Error for ParseGameError {}

#[cfg(test)]
#[path = "game_tests.rs"]
mod tests;
