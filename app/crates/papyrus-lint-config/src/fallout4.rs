//! Detects a Fallout 4 install via the Windows registry, and its vanilla
//! Papyrus source directories, so a project's `lookup_script_roots` (see
//! [`crate::project_file`]) can be seeded/merged with them when that key
//! hasn't been set explicitly.

use std::path::{Path, PathBuf};

/// Registry keys (under `HKEY_LOCAL_MACHINE`) consulted for Fallout 4's
/// install directory when seeding a project's `lookup_script_roots`.
#[cfg(windows)]
const FALLOUT4_REGISTRY_KEYS: [&str; 2] = [
    r"Software\Bethesda Softworks\Fallout4",
    r"Software\Wow6432Node\Bethesda Softworks\Fallout4",
];

/// Registry value name holding Fallout 4's install path.
#[cfg(windows)]
const FALLOUT4_REGISTRY_VALUE: &str = "installed path";

/// Vanilla Papyrus source directories, relative to a Fallout 4 install
/// root: the Creation Kit's script source archives unpack into `Base`
/// (the vanilla game scripts) and `User` (empty, reserved for mod
/// authors' own scripts) under `Data/Scripts/Source`. Only those that
/// actually exist are ever seeded into a config.
const FALLOUT4_SCRIPT_SOURCE_RELATIVE_DIRS: [&str; 2] =
    ["Data/Scripts/Source/Base", "Data/Scripts/Source/User"];

/// Fallout 4's install directory from the Windows registry, if one of
/// [`FALLOUT4_REGISTRY_KEYS`] contains a usable [`FALLOUT4_REGISTRY_VALUE`].
/// Always `None` on non-Windows platforms, and `None` when the recorded
/// path is not an existing directory.
pub fn detected_fallout4_install_path() -> Option<PathBuf> {
    #[cfg(windows)]
    {
        read_fallout4_install_path_from_registry().filter(|path| path.is_dir())
    }
    #[cfg(not(windows))]
    {
        None
    }
}

#[cfg(windows)]
fn read_fallout4_install_path_from_registry() -> Option<PathBuf> {
    use winreg::enums::HKEY_LOCAL_MACHINE;
    use winreg::RegKey;

    let hklm = RegKey::predef(HKEY_LOCAL_MACHINE);
    for key_path in FALLOUT4_REGISTRY_KEYS {
        let Ok(key) = hklm.open_subkey(key_path) else {
            continue;
        };
        let Ok(value) = key.get_value::<String, _>(FALLOUT4_REGISTRY_VALUE) else {
            continue;
        };
        let path = PathBuf::from(value.trim());
        if !path.as_os_str().is_empty() {
            return Some(path);
        }
    }
    None
}

/// Vanilla Papyrus source directories under Fallout 4's install, used to
/// seed a project's `lookup_script_roots`. Only directories that
/// currently exist are returned.
pub fn detected_fallout4_script_lookup_dirs() -> Vec<String> {
    detected_fallout4_install_path()
        .map(|install| script_lookup_dirs_for_fallout4_install(&install))
        .unwrap_or_default()
}

/// Returns `{install}/Data/Scripts/Source/Base` and
/// `{install}/Data/Scripts/Source/User` when those directories exist.
fn script_lookup_dirs_for_fallout4_install(install: &Path) -> Vec<String> {
    FALLOUT4_SCRIPT_SOURCE_RELATIVE_DIRS
        .iter()
        .map(|relative| install.join(relative))
        .filter(|path| path.is_dir())
        .map(|path| path.to_string_lossy().into_owned())
        .collect()
}

#[cfg(test)]
mod tests {
    use std::fs;

    use super::*;

    #[test]
    fn script_lookup_dirs_for_fallout4_install_returns_existing_source_directories() {
        let install = tempfile::tempdir().expect("failed to create temp dir");
        let base = install.path().join("Data/Scripts/Source/Base");
        let user = install.path().join("Data/Scripts/Source/User");
        fs::create_dir_all(&base).expect("failed to create Source/Base");
        fs::create_dir_all(&user).expect("failed to create Source/User");

        let dirs = script_lookup_dirs_for_fallout4_install(install.path());

        assert_eq!(
            dirs,
            vec![
                base.to_string_lossy().into_owned(),
                user.to_string_lossy().into_owned()
            ]
        );
    }

    #[test]
    fn script_lookup_dirs_for_fallout4_install_omits_missing_directories() {
        let install = tempfile::tempdir().expect("failed to create temp dir");
        let base = install.path().join("Data/Scripts/Source/Base");
        fs::create_dir_all(&base).expect("failed to create Source/Base");

        let dirs = script_lookup_dirs_for_fallout4_install(install.path());

        assert_eq!(dirs, vec![base.to_string_lossy().into_owned()]);
    }
}
