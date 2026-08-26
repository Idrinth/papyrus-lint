//! Locates and loads a project's papyrus-lint YAML configuration file,
//! producing the [`papyrus_lints::Config`] passed to every check/fix job.

use std::fs;
use std::path::Path;

/// Candidate config file names, checked in order, inside a project's
/// directory (conventionally the directory containing its `.archlist`
/// file).
const CONFIG_FILE_NAMES: [&str; 2] = ["papyrus-lint.yaml", "papyrus-lint.yml"];

/// Looks for a papyrus-lint config file in `dir` and parses it into a
/// [`papyrus_lints::Config`]. Returns [`papyrus_lints::Config::default`]
/// if `dir` contains none of the candidate file names.
pub fn load_config(dir: &Path) -> Result<papyrus_lints::Config, String> {
    for name in CONFIG_FILE_NAMES {
        let path = dir.join(name);
        if !path.is_file() {
            continue;
        }

        let contents = fs::read_to_string(&path).map_err(|err| err.to_string())?;
        return papyrus_lints::config::parse(&contents).map_err(|err| err.to_string());
    }

    Ok(papyrus_lints::Config::default())
}

/// Writes `config` to `dir`'s papyrus-lint YAML config file. Overwrites
/// whichever candidate name (`papyrus-lint.yaml`/`.yml`) already exists in
/// `dir`, or creates `papyrus-lint.yaml` if `dir` has neither yet.
pub fn save_config(dir: &Path, config: &papyrus_lints::Config) -> Result<(), String> {
    let path = CONFIG_FILE_NAMES
        .iter()
        .map(|name| dir.join(name))
        .find(|path| path.is_file())
        .unwrap_or_else(|| dir.join(CONFIG_FILE_NAMES[0]));

    let yaml = papyrus_lints::config::to_yaml(config).map_err(|err| err.to_string())?;
    fs::write(&path, yaml).map_err(|err| err.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use papyrus_lints::config::Indentation;

    fn write_config(dir: &Path, name: &str, contents: &str) {
        fs::write(dir.join(name), contents).expect("failed to write test config file");
    }

    #[test]
    fn returns_defaults_when_no_config_file_present() {
        let dir = tempfile::tempdir().expect("failed to create temp dir");

        let config = load_config(dir.path()).expect("loading should succeed");

        assert_eq!(config, papyrus_lints::Config::default());
    }

    #[test]
    fn loads_yaml_config_file() {
        let dir = tempfile::tempdir().expect("failed to create temp dir");
        write_config(
            dir.path(),
            "papyrus-lint.yaml",
            "semicolon: true\nindentation: space\n",
        );

        let config = load_config(dir.path()).expect("loading should succeed");

        assert!(config.semicolon);
        assert_eq!(config.indentation, Indentation::Space);
    }

    #[test]
    fn loads_yml_config_file() {
        let dir = tempfile::tempdir().expect("failed to create temp dir");
        write_config(dir.path(), "papyrus-lint.yml", "semicolon: true\n");

        let config = load_config(dir.path()).expect("loading should succeed");

        assert!(config.semicolon);
    }

    #[test]
    fn prefers_yaml_over_yml() {
        let dir = tempfile::tempdir().expect("failed to create temp dir");
        write_config(dir.path(), "papyrus-lint.yaml", "semicolon: true\n");
        write_config(dir.path(), "papyrus-lint.yml", "semicolon: false\n");

        let config = load_config(dir.path()).expect("loading should succeed");

        assert!(config.semicolon);
    }

    #[test]
    fn errors_on_invalid_yaml() {
        let dir = tempfile::tempdir().expect("failed to create temp dir");
        write_config(dir.path(), "papyrus-lint.yaml", "semicolon: [not a bool\n");

        let result = load_config(dir.path());

        assert!(result.is_err());
    }

    #[test]
    fn save_creates_yaml_file_when_none_exists() {
        let dir = tempfile::tempdir().expect("failed to create temp dir");
        let config = papyrus_lints::Config {
            semicolon: true,
            indentation: Indentation::Space,
            indentation_width: 2,
            ..papyrus_lints::Config::default()
        };

        save_config(dir.path(), &config).expect("saving should succeed");

        assert!(dir.path().join("papyrus-lint.yaml").is_file());
        assert!(!dir.path().join("papyrus-lint.yml").exists());
        let loaded = load_config(dir.path()).expect("loading should succeed");
        assert_eq!(loaded, config);
    }

    #[test]
    fn save_overwrites_existing_yaml_file() {
        let dir = tempfile::tempdir().expect("failed to create temp dir");
        write_config(dir.path(), "papyrus-lint.yaml", "semicolon: false\n");
        let config = papyrus_lints::Config {
            semicolon: true,
            ..papyrus_lints::Config::default()
        };

        save_config(dir.path(), &config).expect("saving should succeed");

        let loaded = load_config(dir.path()).expect("loading should succeed");
        assert_eq!(loaded, config);
    }

    #[test]
    fn save_prefers_existing_yml_file_over_creating_yaml() {
        let dir = tempfile::tempdir().expect("failed to create temp dir");
        write_config(dir.path(), "papyrus-lint.yml", "semicolon: false\n");
        let config = papyrus_lints::Config {
            semicolon: true,
            ..papyrus_lints::Config::default()
        };

        save_config(dir.path(), &config).expect("saving should succeed");

        assert!(!dir.path().join("papyrus-lint.yaml").exists());
        let loaded = load_config(dir.path()).expect("loading should succeed");
        assert_eq!(loaded, config);
    }
}
