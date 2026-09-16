//! Config file names and executable-adjacent path helpers.

use std::path::{Path, PathBuf};

/// Candidate config file names, checked in order, inside a project's
/// directory (conventionally the directory containing its `.achlist`
/// file).
pub(crate) const CONFIG_FILE_NAMES: [&str; 2] = ["papyrus-lint.yaml", "papyrus-lint.yml"];

/// The name of the compiler executable looked for during auto-detection.
pub(crate) const COMPILER_EXECUTABLE_NAME: &str = "PapyrusCompiler.exe";

/// The directory (relative to a project's `.achlist` directory) that
/// auto-detection looks for the compiler executable under.
pub(crate) const COMPILER_AUTO_DETECT_DIR_NAME: &str = "Papyrus Compiler";

/// Finds a project's existing config file in `dir`, if any.
pub(crate) fn existing_config_path(dir: &Path) -> Option<PathBuf> {
    CONFIG_FILE_NAMES
        .iter()
        .map(|name| dir.join(name))
        .find(|path| path.is_file())
}

/// Returns the configuration file selected for `dir`, if the project has
/// either of the supported configuration file names.
pub fn config_file_path(dir: &Path) -> Option<PathBuf> {
    existing_config_path(dir)
}

/// Directory next to the CLI's own running executable, if it can be
/// determined. [`crate::initialize_default_config`] looks here for an optional
/// shared base config, and [`crate::user_presets_dir`] for an optional user
/// presets directory.
pub(crate) fn executable_dir() -> Option<PathBuf> {
    std::env::current_exe()
        .ok()
        .and_then(|path| path.parent().map(Path::to_path_buf))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::write_config;
    use std::fs;

    #[test]
    fn config_file_path_reports_the_selected_config() {
        let dir = tempfile::tempdir().expect("failed to create temp dir");
        assert_eq!(config_file_path(dir.path()), None);

        let yml = dir.path().join("papyrus-lint.yml");
        write_config(dir.path(), "papyrus-lint.yml", "semicolon: false\n");
        assert_eq!(config_file_path(dir.path()), Some(yml));

        let yaml = dir.path().join("papyrus-lint.yaml");
        write_config(dir.path(), "papyrus-lint.yaml", "semicolon: true\n");
        assert_eq!(config_file_path(dir.path()), Some(yaml));
    }

    #[test]
    fn config_file_path_ignores_directories_with_config_names() {
        let dir = tempfile::tempdir().expect("failed to create temp dir");
        fs::create_dir(dir.path().join("papyrus-lint.yaml"))
            .expect("failed to create misleading config directory");

        assert_eq!(config_file_path(dir.path()), None);
    }
}
