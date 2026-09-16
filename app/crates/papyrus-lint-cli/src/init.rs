use std::io::Write;
use std::path::{Path, PathBuf};

use papyrus_lint_core::config;

pub(crate) fn initialize_config(
    dir: &Path,
    preset: config::Preset,
    stdout: &mut impl Write,
    stderr: &mut impl Write,
) -> u8 {
    match config::initialize_default_config(dir, preset) {
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

/// Why [`parse_init_preset`] rejected `init`'s arguments: a missing
/// `--preset` value, an argument that isn't `--preset`/`--preset=<name>` at
/// all, or a blank `--preset=` value all get the generic [`crate::USAGE`] text,
/// matching every other usage error this CLI reports. A non-blank preset
/// name is never rejected at this stage even if it isn't one of the three
/// built-ins (see [`config::Preset::parse`]): it's accepted as a possible
/// user preset name and only found to be unresolvable once `init` actually
/// looks for a matching file, reported the same way as any other
/// `initialize_config` failure.
#[derive(Debug, PartialEq, Eq)]
pub(crate) enum InitPresetError {
    /// A missing `--preset` value, an argument that isn't `--preset`/
    /// `--preset=<name>` at all, or a blank preset name.
    Usage,
}

/// Parses the arguments following `init` (i.e. `args[1..]` in [`run`]) into
/// the [`config::Preset`] its `--preset <name>`/`--preset=<name>` flag
/// selects, defaulting to [`config::Preset::default`] (`strict`) when
/// `rest` is empty. Split out from [`run`] so the parsing itself is
/// testable without touching the process's actual current directory,
/// unlike `init`'s success path (see [`initialize_config`]), which writes
/// into it.
pub(crate) fn parse_init_preset(rest: &[String]) -> Result<config::Preset, InitPresetError> {
    let mut preset = config::Preset::default();
    let mut args = rest.iter();
    while let Some(arg) = args.next() {
        let value = if arg == "--preset" {
            args.next()
                .map(String::as_str)
                .ok_or(InitPresetError::Usage)?
        } else if let Some(value) = arg.strip_prefix("--preset=") {
            value
        } else {
            return Err(InitPresetError::Usage);
        };

        preset = config::Preset::parse(value).ok_or(InitPresetError::Usage)?;
    }
    Ok(preset)
}

/// Why [`parse_preset_add_args`] rejected `preset add`'s arguments: missing
/// or extra positional arguments, or an unrecognized flag. Mirrors
/// [`InitPresetError`]'s single `Usage` variant, reported the same way (the
/// generic [`crate::USAGE`] text).
#[derive(Debug, PartialEq, Eq)]
pub(crate) enum PresetAddArgsError {
    Usage,
}

/// Parses the arguments following `preset add` (i.e. `args[2..]` in
/// [`run`]) into `(name, source_path, overwrite)`: the two required
/// positional arguments (a preset name and a path to an existing
/// `papyrus-lint.yaml`) plus whether `--yes` was given. Split out from
/// [`run`] so the parsing itself is testable without touching the process's
/// actual executable-adjacent `presets` directory, the same way
/// [`parse_init_preset`] is split from `init`'s own filesystem effects.
pub(crate) fn parse_preset_add_args(
    rest: &[String],
) -> Result<(String, PathBuf, bool), PresetAddArgsError> {
    let mut overwrite = false;
    let mut positionals: Vec<&String> = Vec::new();
    for arg in rest {
        if arg == "--yes" {
            overwrite = true;
        } else if arg.starts_with("--") {
            return Err(PresetAddArgsError::Usage);
        } else {
            positionals.push(arg);
        }
    }
    match positionals.as_slice() {
        [name, path] => Ok(((*name).clone(), PathBuf::from((*path).clone()), overwrite)),
        _ => Err(PresetAddArgsError::Usage),
    }
}

/// Reports the outcome of `preset add` (see [`config::add_user_preset`]) to
/// `stdout`/`stderr` and returns the process exit code. Split out from the
/// actual [`config::add_user_preset`] call in [`run`] so it's testable
/// without touching the executable-adjacent `presets` directory (which
/// [`config::add_user_preset`] always writes into) from a parallel test
/// suite — the same reason [`parse_init_preset`] is split from `init`'s own
/// filesystem effects.
pub(crate) fn report_add_user_preset(
    name: &str,
    result: Result<PathBuf, config::AddPresetError>,
    stdout: &mut impl Write,
    stderr: &mut impl Write,
) -> u8 {
    match result {
        Ok(path) => {
            let _ = writeln!(stdout, "Added preset '{name}' at {}", path.display());
            0
        }
        Err(config::AddPresetError::AlreadyExists(path)) => {
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
