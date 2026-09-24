//! Filesystem discovery for executable-adjacent user presets: locating the
//! running executable's directory and finding or listing preset YAML files.

use std::path::{Path, PathBuf};

use walkdir::WalkDir;

use crate::presets::USER_PRESETS_DIR_NAME;

/// Directory next to the running executable, if it can be determined.
pub(crate) fn executable_dir() -> Option<PathBuf> {
    std::env::current_exe()
        .ok()
        .and_then(|path| path.parent().map(Path::to_path_buf))
}

/// The user presets directory next to the running executable (see
/// [`USER_PRESETS_DIR_NAME`]), if the executable's location can be
/// determined and it actually has such a directory.
pub fn user_presets_dir() -> Option<PathBuf> {
    user_presets_dir_under(executable_dir().as_deref())
}

/// Finds the user presets directory below an explicit executable directory,
/// allowing callers and tests to avoid another `current_exe` lookup.
pub(crate) fn user_presets_dir_under(base_dir: Option<&Path>) -> Option<PathBuf> {
    let dir = base_dir?.join(USER_PRESETS_DIR_NAME);
    dir.is_dir().then_some(dir)
}

/// Whether `path` has a YAML file extension, matched case-insensitively.
pub(crate) fn has_yaml_extension(path: &Path) -> bool {
    path.extension()
        .and_then(|ext| ext.to_str())
        .is_some_and(|ext| ext.eq_ignore_ascii_case("yaml") || ext.eq_ignore_ascii_case("yml"))
}

/// Immediate children of `dir`. An unreadable directory yields no entries.
fn dir_children(dir: &Path) -> impl Iterator<Item = PathBuf> {
    WalkDir::new(dir)
        .min_depth(1)
        .max_depth(1)
        .into_iter()
        .filter_map(Result::ok)
        .map(walkdir::DirEntry::into_path)
}

/// Every user preset name available in `dir`: each `.yaml`/`.yml` file's stem,
/// sorted case-insensitively. Returns an empty `Vec` if `dir` cannot be read.
pub fn list_user_preset_names(dir: &Path) -> Vec<String> {
    let mut names: Vec<String> = dir_children(dir)
        .filter(|path| path.is_file() && has_yaml_extension(path))
        .filter_map(|path| {
            path.file_stem()
                .and_then(|stem| stem.to_str())
                .map(str::to_string)
        })
        .collect();
    names.sort_by_key(|name| name.to_ascii_lowercase());
    names
}

/// Finds the YAML file in `dir` whose stem matches `name` case-insensitively.
pub(crate) fn find_user_preset_file(dir: &Path, name: &str) -> Option<PathBuf> {
    dir_children(dir).find(|path| {
        path.is_file()
            && has_yaml_extension(path)
            && path
                .file_stem()
                .and_then(|stem| stem.to_str())
                .is_some_and(|stem| stem.eq_ignore_ascii_case(name))
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn user_presets_dir_under_requires_an_existing_directory() {
        let base_dir = tempfile::tempdir().expect("failed to create temp dir");

        assert_eq!(user_presets_dir_under(None), None);
        assert_eq!(user_presets_dir_under(Some(base_dir.path())), None);

        let presets_dir = base_dir.path().join(USER_PRESETS_DIR_NAME);
        std::fs::create_dir(&presets_dir).expect("failed to create presets dir");

        assert_eq!(
            user_presets_dir_under(Some(base_dir.path())),
            Some(presets_dir)
        );
    }

    #[test]
    fn yaml_extension_matching_is_case_insensitive_and_rejects_other_paths() {
        assert!(has_yaml_extension(Path::new("preset.yaml")));
        assert!(has_yaml_extension(Path::new("preset.YML")));
        assert!(!has_yaml_extension(Path::new("preset.json")));
        assert!(!has_yaml_extension(Path::new("preset")));
    }
}
