//! Lint configuration, read from a project's YAML config file and passed
//! to every check/fix job (see [`crate::lint`] and [`crate::repair`]).
//!
//! A config file only needs to set the keys it wants to override — any
//! key it omits falls back to the default shown below:
//!
//! ```yaml
//! semicolon: false
//! indentation: tab
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
/// config file. Fields absent from the YAML fall back to their default.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Config {
    /// Whether lines are required to end in a semicolon (`true`) or must
    /// not (`false`). See the "Semicolon at end of line" lint in
    /// README.md.
    pub semicolon: bool,
    /// The indentation style enforced by the "Formatting checks"/
    /// "Indentation" lint and automatic fix in README.md.
    pub indentation: Indentation,
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
    }

    #[test]
    fn parses_full_config() {
        let config = parse("semicolon: true\nindentation: space\n").unwrap();
        assert!(config.semicolon);
        assert_eq!(config.indentation, Indentation::Space);
    }

    #[test]
    fn missing_keys_fall_back_to_defaults() {
        let config = parse("semicolon: true\n").unwrap();
        assert!(config.semicolon);
        assert_eq!(config.indentation, Indentation::Tab);

        let config = parse("indentation: space\n").unwrap();
        assert!(!config.semicolon);
        assert_eq!(config.indentation, Indentation::Space);
    }

    #[test]
    fn rejects_invalid_yaml() {
        assert!(parse("semicolon: [this is not a bool\n").is_err());
    }

    #[test]
    fn rejects_unknown_indentation_value() {
        assert!(parse("indentation: eight-spaces\n").is_err());
    }
}
