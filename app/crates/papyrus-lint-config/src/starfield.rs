//! Detects a Starfield install via the Windows registry, and its vanilla
//! Papyrus source directories, so a project's `lookup_script_roots` (see
//! [`crate::project_file`]) can be seeded/merged with them when that key
//! hasn't been set explicitly.

use std::path::{Path, PathBuf};

/// Registry keys (under `HKEY_LOCAL_MACHINE`) consulted for Starfield's
/// install directory when seeding a project's `lookup_script_roots`.
#[cfg(windows)]
const STARFIELD_REGISTRY_KEYS: [&str; 2] = [
    r"Software\Bethesda Softworks\Starfield",
    r"Software\Wow6432Node\Bethesda Softworks\Starfield",
];

/// Registry value name holding Starfield's install path.
#[cfg(windows)]
const STARFIELD_REGISTRY_VALUE: &str = "installed path";

/// Vanilla Papyrus source directories, relative to a Starfield install
/// root. The Creation Kit unpacks sources under `Data/Scripts/Source`;
/// some layouts also use FO4-style `Base`/`User` subfolders. Only those
/// that actually exist are ever seeded into a config.
const STARFIELD_SCRIPT_SOURCE_RELATIVE_DIRS: [&str; 3] = [
    "Data/Scripts/Source",
    "Data/Scripts/Source/Base",
    "Data/Scripts/Source/User",
];

/// Starfield's install directory from the Windows registry, if one of
/// [`STARFIELD_REGISTRY_KEYS`] contains a usable [`STARFIELD_REGISTRY_VALUE`].
/// Always `None` on non-Windows platforms, and `None` when the recorded
/// path is not an existing directory.
pub fn detected_starfield_install_path() -> Option<PathBuf> {
    #[cfg(windows)]
    {
        read_starfield_install_path_from_registry().filter(|path| path.is_dir())
    }
    #[cfg(not(windows))]
    {
        None
    }
}

#[cfg(windows)]
fn read_starfield_install_path_from_registry() -> Option<PathBuf> {
    use winreg::enums::HKEY_LOCAL_MACHINE;
    use winreg::RegKey;

    let hklm = RegKey::predef(HKEY_LOCAL_MACHINE);
    for key_path in STARFIELD_REGISTRY_KEYS {
        let Ok(key) = hklm.open_subkey(key_path) else {
            continue;
        };
        let Ok(value) = key.get_value::<String, _>(STARFIELD_REGISTRY_VALUE) else {
            continue;
        };
        let path = PathBuf::from(value.trim());
        if !path.as_os_str().is_empty() {
            return Some(path);
        }
    }
    None
}

/// Vanilla Papyrus source directories under Starfield's install, used to
/// seed a project's `lookup_script_roots`. Only directories that
/// currently exist are returned.
pub fn detected_starfield_script_lookup_dirs() -> Vec<String> {
    detected_starfield_install_path()
        .map(|install| script_lookup_dirs_for_starfield_install(&install))
        .unwrap_or_default()
}

/// Returns the Starfield source directories under `install` that exist.
fn script_lookup_dirs_for_starfield_install(install: &Path) -> Vec<String> {
    STARFIELD_SCRIPT_SOURCE_RELATIVE_DIRS
        .iter()
        .map(|relative| install.join(relative))
        .filter(|path| path.is_dir())
        .map(|path| path.to_string_lossy().into_owned())
        .collect()
}

#[cfg(test)]
#[path = "starfield_tests.rs"]
mod tests;
