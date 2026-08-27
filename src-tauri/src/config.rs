//! Locates and loads a project's papyrus-lint YAML configuration file,
//! producing the [`papyrus_lints::Config`] passed to every check/fix job,
//! and the app-level settings (currently just the PapyrusCompiler.exe path)
//! that live in the same file alongside it.

use std::fs;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

/// Candidate config file names, checked in order, inside a project's
/// directory (conventionally the directory containing its `.achlist`
/// file).
const CONFIG_FILE_NAMES: [&str; 2] = ["papyrus-lint.yaml", "papyrus-lint.yml"];

/// The name of the compiler executable looked for during auto-detection.
const COMPILER_EXECUTABLE_NAME: &str = "PapyrusCompiler.exe";

/// The directory (relative to a project's `.achlist` directory) that
/// auto-detection looks for the compiler executable under.
const COMPILER_AUTO_DETECT_DIR_NAME: &str = "Papyrus Compiler";

/// The full contents of a project's papyrus-lint YAML config file: the
/// lint/fix settings (flattened at the top level, unchanged from before)
/// plus app-level settings that aren't lint-related, currently just an
/// optional explicit PapyrusCompiler.exe path override.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
struct ProjectFile {
    #[serde(skip_serializing_if = "Option::is_none")]
    compiler_path: Option<String>,
    #[serde(flatten)]
    lint: papyrus_lints::Config,
}

/// Finds a project's existing config file in `dir`, if any.
fn existing_config_path(dir: &Path) -> Option<PathBuf> {
    CONFIG_FILE_NAMES
        .iter()
        .map(|name| dir.join(name))
        .find(|path| path.is_file())
}

/// Reads and parses `dir`'s papyrus-lint config file, if it has one.
/// Returns [`ProjectFile::default`] if `dir` has none of the candidate
/// file names.
fn load_project_file(dir: &Path) -> Result<ProjectFile, String> {
    let Some(path) = existing_config_path(dir) else {
        return Ok(ProjectFile::default());
    };

    let contents = fs::read_to_string(&path).map_err(|err| err.to_string())?;
    if contents.trim().is_empty() {
        return Ok(ProjectFile::default());
    }
    serde_yaml::from_str(&contents).map_err(|err| err.to_string())
}

/// Writes `project` to `dir`'s papyrus-lint YAML config file. Overwrites
/// whichever candidate name (`papyrus-lint.yaml`/`.yml`) already exists in
/// `dir`, or creates `papyrus-lint.yaml` if `dir` has neither yet.
fn save_project_file(dir: &Path, project: &ProjectFile) -> Result<(), String> {
    let path = existing_config_path(dir).unwrap_or_else(|| dir.join(CONFIG_FILE_NAMES[0]));

    let yaml = serde_yaml::to_string(project).map_err(|err| err.to_string())?;
    fs::write(&path, yaml).map_err(|err| err.to_string())
}

/// Looks for a papyrus-lint config file in `dir` and parses it into a
/// [`papyrus_lints::Config`]. Returns [`papyrus_lints::Config::default`]
/// if `dir` contains none of the candidate file names.
pub fn load_config(dir: &Path) -> Result<papyrus_lints::Config, String> {
    Ok(load_project_file(dir)?.lint)
}

/// Writes `config` to `dir`'s papyrus-lint YAML config file, preserving
/// any explicit PapyrusCompiler.exe path override already stored there.
pub fn save_config(dir: &Path, config: &papyrus_lints::Config) -> Result<(), String> {
    let mut project = load_project_file(dir)?;
    project.lint = config.clone();
    save_project_file(dir, &project)
}

/// Reads `dir`'s papyrus-lint config file and returns the explicit
/// PapyrusCompiler.exe path override it stores, if any (an empty string is
/// treated the same as no override).
pub fn load_compiler_path(dir: &Path) -> Result<Option<String>, String> {
    let path = load_project_file(dir)?.compiler_path;
    Ok(path.filter(|path| !path.trim().is_empty()))
}

/// Persists an explicit PapyrusCompiler.exe path override to `dir`'s
/// papyrus-lint config file, preserving its lint settings. `path: None`
/// (or an empty string) clears the override, reverting to auto-detection.
pub fn save_compiler_path(dir: &Path, path: Option<&str>) -> Result<(), String> {
    let mut project = load_project_file(dir)?;
    project.compiler_path = path
        .map(str::trim)
        .filter(|path| !path.is_empty())
        .map(str::to_owned);
    save_project_file(dir, &project)
}

/// Looks for `PapyrusCompiler.exe` under a `Papyrus Compiler` directory one
/// level above `dir` (a project's `.achlist` directory) — the layout used
/// by Bethesda's Creation Kit tooling, where a game's `Data` directory
/// (typically where a project's `.achlist` lives) sits alongside a
/// `Papyrus Compiler` directory in the game's install root. Returns `None`
/// if `dir` has no parent or the executable isn't found there.
pub fn auto_detect_compiler_path(dir: &Path) -> Option<PathBuf> {
    let candidate = dir
        .parent()?
        .join(COMPILER_AUTO_DETECT_DIR_NAME)
        .join(COMPILER_EXECUTABLE_NAME);
    candidate.is_file().then_some(candidate)
}

/// Resolves the PapyrusCompiler.exe path to use for `dir`'s project: an
/// explicit override from its papyrus-lint config file, or, absent one,
/// an auto-detected path (see [`auto_detect_compiler_path`]). Returns
/// `None` if neither is available.
pub fn resolve_compiler_path(dir: &Path) -> Result<Option<String>, String> {
    if let Some(path) = load_compiler_path(dir)? {
        return Ok(Some(path));
    }

    Ok(auto_detect_compiler_path(dir).map(|path| path.to_string_lossy().into_owned()))
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

    #[test]
    fn load_compiler_path_returns_none_when_unset() {
        let dir = tempfile::tempdir().expect("failed to create temp dir");

        assert_eq!(
            load_compiler_path(dir.path()).expect("should succeed"),
            None
        );
    }

    #[test]
    fn load_compiler_path_reads_explicit_override() {
        let dir = tempfile::tempdir().expect("failed to create temp dir");
        write_config(
            dir.path(),
            "papyrus-lint.yaml",
            "compiler_path: C:\\Tools\\PapyrusCompiler.exe\n",
        );

        assert_eq!(
            load_compiler_path(dir.path()).expect("should succeed"),
            Some("C:\\Tools\\PapyrusCompiler.exe".to_string())
        );
    }

    #[test]
    fn load_compiler_path_treats_blank_override_as_unset() {
        let dir = tempfile::tempdir().expect("failed to create temp dir");
        write_config(dir.path(), "papyrus-lint.yaml", "compiler_path: \"   \"\n");

        assert_eq!(
            load_compiler_path(dir.path()).expect("should succeed"),
            None
        );
    }

    #[test]
    fn save_compiler_path_persists_override_without_disturbing_lint_settings() {
        let dir = tempfile::tempdir().expect("failed to create temp dir");
        let config = papyrus_lints::Config {
            semicolon: true,
            ..papyrus_lints::Config::default()
        };
        save_config(dir.path(), &config).expect("saving lint config should succeed");

        save_compiler_path(dir.path(), Some("C:\\Tools\\PapyrusCompiler.exe"))
            .expect("saving compiler path should succeed");

        assert_eq!(
            load_compiler_path(dir.path()).expect("should succeed"),
            Some("C:\\Tools\\PapyrusCompiler.exe".to_string())
        );
        assert_eq!(load_config(dir.path()).expect("should succeed"), config);
    }

    #[test]
    fn save_config_preserves_existing_compiler_path_override() {
        let dir = tempfile::tempdir().expect("failed to create temp dir");
        save_compiler_path(dir.path(), Some("C:\\Tools\\PapyrusCompiler.exe"))
            .expect("saving compiler path should succeed");

        let config = papyrus_lints::Config {
            semicolon: true,
            ..papyrus_lints::Config::default()
        };
        save_config(dir.path(), &config).expect("saving lint config should succeed");

        assert_eq!(
            load_compiler_path(dir.path()).expect("should succeed"),
            Some("C:\\Tools\\PapyrusCompiler.exe".to_string())
        );
        assert_eq!(load_config(dir.path()).expect("should succeed"), config);
    }

    #[test]
    fn save_compiler_path_none_clears_override() {
        let dir = tempfile::tempdir().expect("failed to create temp dir");
        save_compiler_path(dir.path(), Some("C:\\Tools\\PapyrusCompiler.exe"))
            .expect("saving compiler path should succeed");

        save_compiler_path(dir.path(), None).expect("clearing compiler path should succeed");

        assert_eq!(
            load_compiler_path(dir.path()).expect("should succeed"),
            None
        );
    }

    #[test]
    fn auto_detect_compiler_path_finds_executable_one_level_up() {
        let root = tempfile::tempdir().expect("failed to create temp dir");
        let compiler_dir = root.path().join("Papyrus Compiler");
        fs::create_dir(&compiler_dir).expect("failed to create compiler dir");
        fs::write(compiler_dir.join("PapyrusCompiler.exe"), b"").expect("failed to write stub exe");
        let data_dir = root.path().join("Data");
        fs::create_dir(&data_dir).expect("failed to create data dir");

        let detected = auto_detect_compiler_path(&data_dir);

        assert_eq!(detected, Some(compiler_dir.join("PapyrusCompiler.exe")));
    }

    #[test]
    fn auto_detect_compiler_path_returns_none_when_absent() {
        let root = tempfile::tempdir().expect("failed to create temp dir");
        let data_dir = root.path().join("Data");
        fs::create_dir(&data_dir).expect("failed to create data dir");

        assert_eq!(auto_detect_compiler_path(&data_dir), None);
    }

    #[test]
    fn resolve_compiler_path_prefers_explicit_override_over_auto_detection() {
        let root = tempfile::tempdir().expect("failed to create temp dir");
        let compiler_dir = root.path().join("Papyrus Compiler");
        fs::create_dir(&compiler_dir).expect("failed to create compiler dir");
        fs::write(compiler_dir.join("PapyrusCompiler.exe"), b"").expect("failed to write stub exe");
        let data_dir = root.path().join("Data");
        fs::create_dir(&data_dir).expect("failed to create data dir");
        save_compiler_path(&data_dir, Some("C:\\Custom\\PapyrusCompiler.exe"))
            .expect("saving compiler path should succeed");

        assert_eq!(
            resolve_compiler_path(&data_dir).expect("should succeed"),
            Some("C:\\Custom\\PapyrusCompiler.exe".to_string())
        );
    }

    #[test]
    fn resolve_compiler_path_falls_back_to_auto_detection() {
        let root = tempfile::tempdir().expect("failed to create temp dir");
        let compiler_dir = root.path().join("Papyrus Compiler");
        fs::create_dir(&compiler_dir).expect("failed to create compiler dir");
        fs::write(compiler_dir.join("PapyrusCompiler.exe"), b"").expect("failed to write stub exe");
        let data_dir = root.path().join("Data");
        fs::create_dir(&data_dir).expect("failed to create data dir");

        assert_eq!(
            resolve_compiler_path(&data_dir).expect("should succeed"),
            Some(
                compiler_dir
                    .join("PapyrusCompiler.exe")
                    .to_string_lossy()
                    .into_owned()
            )
        );
    }

    #[test]
    fn resolve_compiler_path_none_when_neither_available() {
        let dir = tempfile::tempdir().expect("failed to create temp dir");

        assert_eq!(
            resolve_compiler_path(dir.path()).expect("should succeed"),
            None
        );
    }
}
