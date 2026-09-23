//! Detects a Skyrim Special Edition install via the Windows registry, and
//! its vanilla Papyrus source directories, so a project's
//! `lookup_script_roots` (see [`crate::project_file`]) can be seeded/merged
//! with them when that key hasn't been set explicitly.

use std::path::{Path, PathBuf};

/// Registry keys (under `HKEY_LOCAL_MACHINE`) consulted for Skyrim Special
/// Edition's install directory when seeding a project's
/// `lookup_script_roots`.
#[cfg(windows)]
const SKYRIM_SE_REGISTRY_KEYS: [&str; 2] = [
    r"Software\Bethesda Softworks\Skyrim Special Edition",
    r"Software\Wow6432Node\Bethesda Softworks\Skyrim Special Edition",
];

/// Registry value name holding Skyrim Special Edition's install path.
#[cfg(windows)]
const SKYRIM_SE_REGISTRY_VALUE: &str = "installed path";

/// Vanilla Papyrus source directories, relative to a Skyrim Special Edition
/// install root. Both layouts exist across CK/game versions; only those
/// that actually exist are ever seeded into a config.
const SKYRIM_SCRIPT_SOURCE_RELATIVE_DIRS: [&str; 2] =
    ["Data/Scripts/Source", "Data/Source/Scripts"];

/// Skyrim Special Edition's install directory from the Windows registry,
/// if one of [`SKYRIM_SE_REGISTRY_KEYS`] contains a usable
/// [`SKYRIM_SE_REGISTRY_VALUE`]. Always `None` on non-Windows platforms,
/// and `None` when the recorded path is not an existing directory.
pub fn detected_skyrim_install_path() -> Option<PathBuf> {
    #[cfg(windows)]
    {
        read_skyrim_install_path_from_registry().filter(|path| path.is_dir())
    }
    #[cfg(not(windows))]
    {
        None
    }
}

#[cfg(windows)]
fn read_skyrim_install_path_from_registry() -> Option<PathBuf> {
    use winreg::enums::HKEY_LOCAL_MACHINE;
    use winreg::RegKey;

    let hklm = RegKey::predef(HKEY_LOCAL_MACHINE);
    for key_path in SKYRIM_SE_REGISTRY_KEYS {
        let Ok(key) = hklm.open_subkey(key_path) else {
            continue;
        };
        let Ok(value) = key.get_value::<String, _>(SKYRIM_SE_REGISTRY_VALUE) else {
            continue;
        };
        let path = PathBuf::from(value.trim());
        if !path.as_os_str().is_empty() {
            return Some(path);
        }
    }
    None
}

/// Vanilla Papyrus source directories under Skyrim Special Edition's
/// install, used to seed a project's `lookup_script_roots`. Only
/// directories that currently exist are returned.
pub fn detected_skyrim_script_lookup_dirs() -> Vec<String> {
    detected_skyrim_install_path()
        .map(|install| script_lookup_dirs_for_skyrim_install(&install))
        .unwrap_or_default()
}

/// Returns `{install}/Data/Scripts/Source` and `{install}/Data/Source/Scripts`
/// when those directories exist.
fn script_lookup_dirs_for_skyrim_install(install: &Path) -> Vec<String> {
    SKYRIM_SCRIPT_SOURCE_RELATIVE_DIRS
        .iter()
        .map(|relative| install.join(relative))
        .filter(|path| path.is_dir())
        .map(|path| path.to_string_lossy().into_owned())
        .collect()
}

#[cfg(test)]
#[path = "skyrim_tests.rs"]
mod tests;
