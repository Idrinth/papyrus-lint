use std::io::Write;
use std::path::{Path, PathBuf};

use papyrus_lint_config::{self as config, presets};
use papyrus_lint_core::ppj;

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

/// The `.ppj` (Papyrus Project XML) file directly inside `dir`, if there's
/// exactly one — matched the same way [`crate::project::is_ppj_path`] does.
/// A project with several `.ppj` files (e.g. one per DLC) picks the first in
/// alphabetical order rather than refusing to seed anything, since guessing
/// wrong here is no worse than `init`'s previous behavior of not looking at
/// all.
fn find_ppj_in_dir(dir: &Path) -> Option<PathBuf> {
    let mut candidates: Vec<PathBuf> = std::fs::read_dir(dir)
        .ok()?
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .filter(|path| path.is_file() && crate::project::is_ppj_path(path))
        .collect();
    candidates.sort();
    candidates.into_iter().next()
}

/// Seeds a freshly initialized config's `additional_script_roots` from a
/// `.ppj` file's own `<Import>` entries (see [`papyrus_lint_core::ppj`]),
/// when `dir` has exactly one and the config `init` just wrote doesn't
/// already have roots of its own (e.g. from an executable-adjacent base
/// config) — otherwise this project's own compile-time import search paths
/// would otherwise have to be guessed or hand-copied from the `.ppj` file.
/// Reports what it did (or why it couldn't) to `stdout`/`stderr`, but never
/// fails `init` itself: the base config was already written successfully by
/// the time this runs.
fn seed_additional_script_roots_from_ppj(
    dir: &Path,
    stdout: &mut impl Write,
    stderr: &mut impl Write,
) {
    let Some(ppj_path) = find_ppj_in_dir(dir) else {
        return;
    };
    match config::load_script_roots(dir) {
        Ok(roots) if !roots.is_empty() => return,
        Err(err) => {
            let _ = writeln!(
                stderr,
                "warning: failed to read additional_script_roots: {err}"
            );
            return;
        }
        Ok(_) => {}
    }

    let project = match ppj::parse_ppj(&ppj_path) {
        Ok(project) => project,
        Err(err) => {
            let _ = writeln!(
                stderr,
                "warning: found {} but failed to parse it: {err}",
                ppj_path.display()
            );
            return;
        }
    };
    if project.imports.is_empty() {
        return;
    }

    let roots: Vec<String> = project
        .imports
        .iter()
        .map(|import| {
            import
                .strip_prefix(dir)
                .map(|relative| relative.to_string_lossy().into_owned())
                .unwrap_or_else(|_| import.to_string_lossy().into_owned())
        })
        .collect();
    match config::save_script_roots(dir, &roots) {
        Ok(()) => {
            let _ = writeln!(
                stdout,
                "Seeded additional_script_roots from {} ({} entr{})",
                ppj_path.display(),
                roots.len(),
                if roots.len() == 1 { "y" } else { "ies" }
            );
        }
        Err(err) => {
            let _ = writeln!(
                stderr,
                "warning: found {} but failed to save its additional_script_roots: {err}",
                ppj_path.display()
            );
        }
    }
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
            seed_additional_script_roots_from_ppj(dir, stdout, stderr);
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
