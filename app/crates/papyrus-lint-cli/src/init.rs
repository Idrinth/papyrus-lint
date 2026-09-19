use std::io::Write;
use std::path::{Path, PathBuf};

use papyrus_lint_config::presets;

/// Runs the `init` subcommand: creates a `papyrus-lint.yaml` in the process's
/// current directory from `preset`, without overwriting an existing config.
pub(crate) fn run_init(
    preset: presets::Preset,
    stdout: &mut impl Write,
    stderr: &mut impl Write,
) -> u8 {
    let current_dir = match std::env::current_dir() {
        Ok(dir) => dir,
        Err(err) => {
            let _ = writeln!(
                stderr,
                "error: failed to determine current directory: {err}"
            );
            return 2;
        }
    };
    initialize_config(&current_dir, preset, stdout, stderr)
}

/// Runs the `preset add` subcommand: adds the user preset `name` from
/// `source_path` (see [`presets::add_user_preset`]).
pub(crate) fn run_preset_add(
    name: String,
    source_path: PathBuf,
    overwrite: bool,
    stdout: &mut impl Write,
    stderr: &mut impl Write,
) -> u8 {
    let result = presets::add_user_preset(&name, &source_path, overwrite);
    report_add_user_preset(&name, result, stdout, stderr)
}

/// Runs the `preset list` subcommand: prints the name of every preset
/// selectable via `--preset <name>`, one per line — the three built-ins
/// (see [`presets::PRESET_NAMES`]) first, then any user preset found under
/// the executable-adjacent `presets` directory (see
/// [`presets::user_presets_dir`]), in the same alphabetical order
/// [`presets::list_user_preset_names`] returns them.
pub(crate) fn run_preset_list(stdout: &mut impl Write) -> u8 {
    for name in presets::PRESET_NAMES {
        let _ = writeln!(stdout, "{name}");
    }
    if let Some(dir) = presets::user_presets_dir() {
        for name in presets::list_user_preset_names(&dir) {
            let _ = writeln!(stdout, "{name}");
        }
    }
    0
}

pub(crate) fn initialize_config(
    dir: &Path,
    preset: presets::Preset,
    stdout: &mut impl Write,
    stderr: &mut impl Write,
) -> u8 {
    match presets::initialize_default_config(dir, preset) {
        Ok(path) => {
            let _ = writeln!(stdout, "Created {}", path.display());
            0
        }
        Err(err) => {
            let _ = writeln!(stderr, "error: failed to initialize config: {err}");
            2
        }
    }
}

/// Reports the outcome of `preset add` (see [`presets::add_user_preset`]) to
/// `stdout`/`stderr` and returns the process exit code. Split out from the
/// actual [`presets::add_user_preset`] call in [`run_preset_add`] so it's
/// testable without touching the executable-adjacent `presets` directory
/// (which [`presets::add_user_preset`] always writes into) from a parallel
/// test suite — the same reason [`parse_init_preset`] is split from `init`'s
/// own filesystem effects.
pub(crate) fn report_add_user_preset(
    name: &str,
    result: Result<PathBuf, presets::AddPresetError>,
    stdout: &mut impl Write,
    stderr: &mut impl Write,
) -> u8 {
    match result {
        Ok(path) => {
            let _ = writeln!(stdout, "Added preset '{name}' at {}", path.display());
            0
        }
        Err(presets::AddPresetError::AlreadyExists(path)) => {
            let _ = writeln!(
                stderr,
                "error: a preset named '{name}' already exists at {} (pass --yes to overwrite it)",
                path.display()
            );
            2
        }
        Err(err) => {
            let _ = writeln!(stderr, "error: failed to add preset: {err}");
            2
        }
    }
}

#[cfg(test)]
#[path = "init_tests.rs"]
mod tests;
