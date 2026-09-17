//! Locates `PapyrusCompiler.exe`: an explicit override stored in a
//! project's papyrus-lint config file (see
//! [`crate::project_file::load_compiler_path`]/
//! [`crate::project_file::save_compiler_path`]), or, absent one, the
//! Creation Kit tooling's conventional install layout, auto-detected here.

use std::path::{Path, PathBuf};

/// The name of the compiler executable looked for during auto-detection.
const COMPILER_EXECUTABLE_NAME: &str = "PapyrusCompiler.exe";

/// The directory (relative to a project's `.achlist` directory) that
/// auto-detection looks for the compiler executable under.
const COMPILER_AUTO_DETECT_DIR_NAME: &str = "Papyrus Compiler";

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
    if let Some(path) = crate::project_file::load_compiler_path(dir)? {
        return Ok(Some(path));
    }

    Ok(auto_detect_compiler_path(dir).map(|path| path.to_string_lossy().into_owned()))
}

#[cfg(test)]
mod tests {
    use std::fs;

    use super::*;
    use crate::project_file::save_compiler_path;

    fn write_config(dir: &Path, name: &str, contents: &str) {
        fs::write(dir.join(name), contents).expect("failed to write test config file");
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
    fn auto_detect_compiler_path_ignores_a_directory_named_like_the_executable() {
        let root = tempfile::tempdir().expect("failed to create temp dir");
        let compiler_dir = root.path().join("Papyrus Compiler");
        fs::create_dir(&compiler_dir).expect("failed to create compiler dir");
        fs::create_dir(compiler_dir.join("PapyrusCompiler.exe"))
            .expect("failed to create misleading executable directory");
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

    #[test]
    fn resolve_compiler_path_propagates_config_errors_before_auto_detection() {
        let root = tempfile::tempdir().expect("failed to create temp dir");
        let compiler_dir = root.path().join("Papyrus Compiler");
        fs::create_dir(&compiler_dir).expect("failed to create compiler dir");
        fs::write(compiler_dir.join("PapyrusCompiler.exe"), b"").expect("failed to write stub exe");
        let data_dir = root.path().join("Data");
        fs::create_dir(&data_dir).expect("failed to create data dir");
        write_config(
            &data_dir,
            "papyrus-lint.yaml",
            "compiler_path: [not a path string]\n",
        );

        assert!(resolve_compiler_path(&data_dir).is_err());
    }
}
