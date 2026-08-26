//! Lint configuration, read from a project's YAML config file and passed
//! to every check/fix job (see [`crate::lint`] and [`crate::repair`]).
//!
//! A config file only needs to set the keys it wants to override — any
//! key it omits falls back to the default shown below:
//!
//! ```yaml
//! semicolon: false
//! indentation: tab
//! indentation_width: 4
//! ```

use std::fmt;

use serde::{Deserialize, Serialize};

/// The indentation style a project expects, for the "Formatting checks"/
/// "Indentation" lint and automatic fix described in README.md.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Indentation {
    #[default]
    Tab,
    Space,
}

/// Configuration for the lint/fix jobs, deserialized from a project's YAML
/// config file and, in the desktop app, kept in sync with the formatting
/// controls in the UI (loaded on startup, saved back to the file whenever
/// they change). Fields absent from the YAML fall back to their default.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Config {
    /// Whether lines are required to end in a semicolon (`true`) or must
    /// not (`false`). See the "Semicolon at end of line" lint in
    /// README.md.
    pub semicolon: bool,
    /// The indentation style enforced by the "Formatting checks"/
    /// "Indentation" lint and automatic fix in README.md.
    pub indentation: Indentation,
    /// The number of spaces per indentation level, used only when
    /// `indentation` is [`Indentation::Space`].
    pub indentation_width: usize,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            semicolon: false,
            indentation: Indentation::default(),
            indentation_width: 4,
        }
    }
}

impl Config {
    /// The trailing-semicolon policy this configuration selects, for use
    /// with [`crate::semicolon::check`]/[`crate::semicolon::repair`].
    pub fn semicolon_style(&self) -> crate::semicolon::Style {
        if self.semicolon {
            crate::semicolon::Style::Require
        } else {
            crate::semicolon::Style::Forbid
        }
    }

    /// The indentation unit this configuration selects, for use with
    /// [`crate::indentation::repair`]. `indentation_width` is clamped to
    /// `1..=16` to match the range accepted by the UI.
    pub fn indentation_unit(&self) -> crate::indentation::Indentation {
        match self.indentation {
            Indentation::Tab => crate::indentation::Indentation::Tabs,
            Indentation::Space => {
                crate::indentation::Indentation::Spaces(self.indentation_width.clamp(1, 16))
            }
        }
    }
}

/// An error parsing a lint config file.
#[derive(Debug)]
pub enum ConfigError {
    Yaml(serde_yaml::Error),
}

impl fmt::Display for ConfigError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ConfigError::Yaml(err) => write!(f, "failed to parse lint config: {err}"),
        }
    }
}

impl std::error::Error for ConfigError {}

impl From<serde_yaml::Error> for ConfigError {
    fn from(err: serde_yaml::Error) -> Self {
        ConfigError::Yaml(err)
    }
}

/// Parses a YAML config document into a [`Config`]. An empty document
/// (including a missing/empty config file's contents) yields
/// [`Config::default`]; keys the document omits also fall back to their
/// default.
pub fn parse(yaml: &str) -> Result<Config, ConfigError> {
    if yaml.trim().is_empty() {
        return Ok(Config::default());
    }
    Ok(serde_yaml::from_str(yaml)?)
}

/// Serializes a [`Config`] back into the YAML document format read by
/// [`parse`], so the desktop app can persist the formatting selected in
/// its UI.
pub fn to_yaml(config: &Config) -> Result<String, ConfigError> {
    Ok(serde_yaml::to_string(config)?)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_document_yields_defaults() {
        assert_eq!(parse("").unwrap(), Config::default());
        assert_eq!(parse("   \n").unwrap(), Config::default());
    }

    #[test]
    fn defaults_match_documented_default() {
        let config = Config::default();
        assert!(!config.semicolon);
        assert_eq!(config.indentation, Indentation::Tab);
        assert_eq!(config.indentation_width, 4);
    }

    #[test]
    fn parses_full_config() {
        let config = parse("semicolon: true\nindentation: space\nindentation_width: 2\n").unwrap();
        assert!(config.semicolon);
        assert_eq!(config.indentation, Indentation::Space);
        assert_eq!(config.indentation_width, 2);
    }

    #[test]
    fn missing_keys_fall_back_to_defaults() {
        let config = parse("semicolon: true\n").unwrap();
        assert!(config.semicolon);
        assert_eq!(config.indentation, Indentation::Tab);
        assert_eq!(config.indentation_width, 4);

        let config = parse("indentation: space\n").unwrap();
        assert!(!config.semicolon);
        assert_eq!(config.indentation, Indentation::Space);
        assert_eq!(config.indentation_width, 4);
    }

    #[test]
    fn rejects_invalid_yaml() {
        assert!(parse("semicolon: [this is not a bool\n").is_err());
    }

    #[test]
    fn rejects_unknown_indentation_value() {
        assert!(parse("indentation: eight-spaces\n").is_err());
    }

    #[test]
    fn semicolon_style_reflects_semicolon_flag() {
        assert_eq!(
            Config {
                semicolon: true,
                ..Config::default()
            }
            .semicolon_style(),
            crate::semicolon::Style::Require
        );
        assert_eq!(
            Config {
                semicolon: false,
                ..Config::default()
            }
            .semicolon_style(),
            crate::semicolon::Style::Forbid
        );
    }

    #[test]
    fn indentation_unit_reflects_indentation_and_width() {
        let config = Config {
            indentation: Indentation::Tab,
            ..Config::default()
        };
        assert_eq!(
            config.indentation_unit(),
            crate::indentation::Indentation::Tabs
        );

        let config = Config {
            indentation: Indentation::Space,
            indentation_width: 2,
            ..Config::default()
        };
        assert_eq!(
            config.indentation_unit(),
            crate::indentation::Indentation::Spaces(2)
        );
    }

    #[test]
    fn indentation_unit_clamps_width_to_valid_range() {
        let config = Config {
            indentation: Indentation::Space,
            indentation_width: 100,
            ..Config::default()
        };
        assert_eq!(
            config.indentation_unit(),
            crate::indentation::Indentation::Spaces(16)
        );

        let config = Config {
            indentation: Indentation::Space,
            indentation_width: 0,
            ..Config::default()
        };
        assert_eq!(
            config.indentation_unit(),
            crate::indentation::Indentation::Spaces(1)
        );
    }

    #[test]
    fn to_yaml_round_trips_through_parse() {
        let config = Config {
            semicolon: true,
            indentation: Indentation::Space,
            indentation_width: 2,
        };

        let yaml = to_yaml(&config).unwrap();

        assert_eq!(parse(&yaml).unwrap(), config);
    }
}
