use std::io::Write;
use std::path::{Path, PathBuf};

use clap::Parser;
use papyrus_lint_config::presets;

use crate::USAGE;

/// `init`'s own flags, extracted by `clap` the same way `args.rs`'s
/// `RawArgs` is for the main lint/fix invocation.
#[derive(Parser, Debug)]
#[command(
    no_binary_name = true,
    disable_help_flag = true,
    disable_version_flag = true
)]
struct InitRawArgs {
    #[arg(long)]
    preset: Option<String>,
}

/// `preset add`'s own flags/positionals, extracted by `clap` the same way
/// `args.rs`'s `RawArgs` is for the main lint/fix invocation. Unlike
/// `RawArgs`'s catch-all positionals, an unrecognized `--flag` here is a
/// hard `clap` parse error (mapped to [`PresetAddArgsError::Usage`]) rather
/// than being accepted as a stray positional, matching `preset add`'s
/// narrower, historically stricter grammar (only `--yes` plus exactly two
/// positionals are valid).
#[derive(Parser, Debug)]
#[command(
    no_binary_name = true,
    disable_help_flag = true,
    disable_version_flag = true
)]
struct PresetAddRawArgs {
    #[arg(long)]
    yes: bool,
    positionals: Vec<String>,
}

/// Runs the `init` subcommand against `args` (i.e. `args[1..]` in
/// [`crate::run`]): creates a `papyrus-lint.yaml` in the process's current
/// directory from the selected `--preset` (see [`parse_init_preset`]),
/// without overwriting an existing config.
pub(crate) fn run_init(args: &[String], stdout: &mut impl Write, stderr: &mut impl Write) -> u8 {
    let preset = match parse_init_preset(args) {
        Ok(preset) => preset,
        Err(InitPresetError::Usage) => {
            let _ = write!(stderr, "{USAGE}");
            return 2;
        }
    };

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

/// Runs the `preset add` subcommand against `args` (i.e. `args[1..]` in
/// [`crate::run`]): parses `add <name> <path-to-papyrus-lint.yaml> [--yes]`
/// (see [`parse_preset_add_args`]) and adds the user preset it names (see
/// [`presets::add_user_preset`]).
pub(crate) fn run_preset_add(
    args: &[String],
    stdout: &mut impl Write,
    stderr: &mut impl Write,
) -> u8 {
    if args.first().map(String::as_str) != Some("add") {
        let _ = write!(stderr, "{USAGE}");
        return 2;
    }
    let (name, source_path, overwrite) = match parse_preset_add_args(&args[1..]) {
        Ok(parsed) => parsed,
        Err(PresetAddArgsError::Usage) => {
            let _ = write!(stderr, "{USAGE}");
            return 2;
        }
    };
    let result = presets::add_user_preset(&name, &source_path, overwrite);
    report_add_user_preset(&name, result, stdout, stderr)
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

/// Why [`parse_init_preset`] rejected `init`'s arguments: a missing
/// `--preset` value, an argument that isn't `--preset`/`--preset=<name>` at
/// all, or a blank `--preset=` value all get the generic [`crate::USAGE`] text,
/// matching every other usage error this CLI reports. A non-blank preset
/// name is never rejected at this stage even if it isn't one of the three
/// built-ins (see [`presets::Preset::parse`]): it's accepted as a possible
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
/// the [`presets::Preset`] its `--preset <name>`/`--preset=<name>` flag
/// selects, defaulting to [`presets::Preset::default`] (`strict`) when
/// `rest` is empty. Split out from [`run`] so the parsing itself is
/// testable without touching the process's actual current directory,
/// unlike `init`'s success path (see [`initialize_config`]), which writes
/// into it.
pub(crate) fn parse_init_preset(rest: &[String]) -> Result<presets::Preset, InitPresetError> {
    let raw = InitRawArgs::try_parse_from(rest).map_err(|_| InitPresetError::Usage)?;
    match raw.preset {
        Some(value) => presets::Preset::parse(&value).ok_or(InitPresetError::Usage),
        None => Ok(presets::Preset::default()),
    }
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
    let raw = PresetAddRawArgs::try_parse_from(rest).map_err(|_| PresetAddArgsError::Usage)?;
    match raw.positionals.as_slice() {
        [name, path] => Ok((name.clone(), PathBuf::from(path.clone()), raw.yes)),
        _ => Err(PresetAddArgsError::Usage),
    }
}

/// Reports the outcome of `preset add` (see [`presets::add_user_preset`]) to
/// `stdout`/`stderr` and returns the process exit code. Split out from the
/// actual [`presets::add_user_preset`] call in [`run`] so it's testable
/// without touching the executable-adjacent `presets` directory (which
/// [`presets::add_user_preset`] always writes into) from a parallel test
/// suite — the same reason [`parse_init_preset`] is split from `init`'s own
/// filesystem effects.
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
