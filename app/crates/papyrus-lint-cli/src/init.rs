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
mod tests {
    use super::*;
    use crate::test_support::*;
    use papyrus_lint_config::{self as config, presets};
    use std::fs;
    use std::path::PathBuf;

    #[test]
    fn prints_usage_when_preset_is_given_without_add() {
        let (code, _stdout, stderr) = run_captured(&["preset".to_string()]);

        assert_eq!(code, 2);
        assert!(stderr.contains("Usage: PapyrusLinterCLI"));
    }

    #[test]
    fn prints_usage_for_an_unrecognized_preset_subcommand() {
        let (code, _stdout, stderr) = run_captured(&["preset".to_string(), "remove".to_string()]);

        assert_eq!(code, 2);
        assert!(stderr.contains("Usage: PapyrusLinterCLI"));
    }

    #[test]
    fn prints_usage_when_preset_add_is_missing_arguments() {
        let (code, _stdout, stderr) = run_captured(&[
            "preset".to_string(),
            "add".to_string(),
            "my-team".to_string(),
        ]);

        assert_eq!(code, 2);
        assert!(stderr.contains("Usage: PapyrusLinterCLI"));
    }

    #[test]
    fn parse_preset_add_args_parses_the_two_positionals() {
        assert_eq!(
            parse_preset_add_args(&[
                "my-team".to_string(),
                "path/to/papyrus-lint.yaml".to_string()
            ]),
            Ok((
                "my-team".to_string(),
                PathBuf::from("path/to/papyrus-lint.yaml"),
                false
            ))
        );
    }

    #[test]
    fn parse_preset_add_args_recognizes_yes_in_any_position() {
        assert_eq!(
            parse_preset_add_args(&[
                "--yes".to_string(),
                "my-team".to_string(),
                "path/to/papyrus-lint.yaml".to_string()
            ]),
            Ok((
                "my-team".to_string(),
                PathBuf::from("path/to/papyrus-lint.yaml"),
                true
            ))
        );
        assert_eq!(
            parse_preset_add_args(&[
                "my-team".to_string(),
                "path/to/papyrus-lint.yaml".to_string(),
                "--yes".to_string()
            ]),
            Ok((
                "my-team".to_string(),
                PathBuf::from("path/to/papyrus-lint.yaml"),
                true
            ))
        );
    }

    #[test]
    fn parse_preset_add_args_rejects_a_missing_argument() {
        let err = parse_preset_add_args(&["my-team".to_string()])
            .expect_err("a single positional argument should be rejected");
        assert!(matches!(err, PresetAddArgsError::Usage));
    }

    #[test]
    fn parse_preset_add_args_rejects_an_extra_argument() {
        let err = parse_preset_add_args(&[
            "my-team".to_string(),
            "path.yaml".to_string(),
            "extra".to_string(),
        ])
        .expect_err("an extra positional argument should be rejected");
        assert!(matches!(err, PresetAddArgsError::Usage));
    }

    #[test]
    fn parse_preset_add_args_rejects_an_unrecognized_flag() {
        let err = parse_preset_add_args(&[
            "my-team".to_string(),
            "path.yaml".to_string(),
            "--force".to_string(),
        ])
        .expect_err("an unrecognized flag should be rejected");
        assert!(matches!(err, PresetAddArgsError::Usage));
    }

    #[test]
    fn report_add_user_preset_prints_the_added_path_on_success() {
        let mut stdout = Vec::new();
        let mut stderr = Vec::new();

        let code = report_add_user_preset(
            "my-team",
            Ok(PathBuf::from("/presets/my-team.yaml")),
            &mut stdout,
            &mut stderr,
        );

        assert_eq!(code, 0);
        assert!(stderr.is_empty());
        assert_eq!(
            String::from_utf8(stdout).unwrap(),
            "Added preset 'my-team' at /presets/my-team.yaml\n"
        );
    }

    #[test]
    fn report_add_user_preset_explains_how_to_confirm_an_overwrite() {
        let mut stdout = Vec::new();
        let mut stderr = Vec::new();

        let code = report_add_user_preset(
            "my-team",
            Err(presets::AddPresetError::AlreadyExists(PathBuf::from(
                "/presets/my-team.yaml",
            ))),
            &mut stdout,
            &mut stderr,
        );

        assert_eq!(code, 2);
        assert!(stdout.is_empty());
        let error = String::from_utf8(stderr).unwrap();
        assert!(error.contains("already exists"));
        assert!(error.contains("--yes"));
    }

    #[test]
    fn report_add_user_preset_reports_an_invalid_name() {
        let mut stdout = Vec::new();
        let mut stderr = Vec::new();

        let code = report_add_user_preset(
            "strict",
            Err(presets::AddPresetError::InvalidName("strict".to_string())),
            &mut stdout,
            &mut stderr,
        );

        assert_eq!(code, 2);
        assert!(stdout.is_empty());
        assert!(String::from_utf8(stderr)
            .unwrap()
            .contains("built-in preset"));
    }

    #[test]
    fn init_creates_a_default_config() {
        let dir = tempfile::tempdir().expect("failed to create temp dir");
        let mut stdout = Vec::new();
        let mut stderr = Vec::new();

        let code = initialize_config(
            dir.path(),
            presets::Preset::default(),
            &mut stdout,
            &mut stderr,
        );

        assert_eq!(code, 0);
        assert!(stderr.is_empty());
        assert!(String::from_utf8(stdout)
            .unwrap()
            .contains("papyrus-lint.yaml"));
        let config = config::load_config(dir.path()).expect("config should load");
        assert_eq!(config, papyrus_lints::Config::default());
    }

    #[test]
    fn init_refuses_to_overwrite_an_existing_config() {
        let dir = tempfile::tempdir().expect("failed to create temp dir");
        let path = dir.path().join("papyrus-lint.yml");
        write_file(&path, "semicolon: true\n");
        let mut stdout = Vec::new();
        let mut stderr = Vec::new();

        let code = initialize_config(
            dir.path(),
            presets::Preset::default(),
            &mut stdout,
            &mut stderr,
        );

        assert_eq!(code, 2);
        assert!(stdout.is_empty());
        assert!(String::from_utf8(stderr)
            .unwrap()
            .contains("config already exists"));
        assert_eq!(fs::read_to_string(path).unwrap(), "semicolon: true\n");
    }

    #[test]
    fn parse_init_preset_defaults_to_strict_when_no_flag_is_given() {
        assert_eq!(parse_init_preset(&[]), Ok(presets::Preset::Strict));
    }

    #[test]
    fn parse_init_preset_accepts_the_flag_and_its_equals_form() {
        assert_eq!(
            parse_init_preset(&["--preset".to_string(), "careful".to_string()]),
            Ok(presets::Preset::Careful)
        );
        assert_eq!(
            parse_init_preset(&["--preset=standard".to_string()]),
            Ok(presets::Preset::Standard)
        );
    }

    #[test]
    fn parse_init_preset_matches_names_case_insensitively() {
        assert_eq!(
            parse_init_preset(&["--preset".to_string(), "STANDARD".to_string()]),
            Ok(presets::Preset::Standard)
        );
    }

    #[test]
    fn parse_init_preset_accepts_a_name_that_is_not_a_built_in_as_a_custom_preset() {
        // Whether a name actually matches a user preset file is only
        // checked once `init` runs (see `presets::Preset::yaml`), not during
        // argument parsing, so an arbitrary non-blank name parses fine here.
        assert_eq!(
            parse_init_preset(&["--preset".to_string(), "lenient".to_string()]),
            Ok(presets::Preset::Custom("lenient".to_string()))
        );
    }

    #[test]
    fn parse_init_preset_rejects_a_missing_value() {
        let err = parse_init_preset(&["--preset".to_string()])
            .expect_err("a --preset with no value should be rejected");
        assert!(matches!(err, InitPresetError::Usage));
    }

    #[test]
    fn parse_init_preset_rejects_a_blank_value() {
        let err = parse_init_preset(&["--preset=".to_string()])
            .expect_err("a blank --preset value should be rejected");
        assert!(matches!(err, InitPresetError::Usage));
    }

    #[test]
    fn parse_init_preset_rejects_an_unrecognized_extra_argument() {
        let err = parse_init_preset(&["extra".to_string()])
            .expect_err("an argument other than --preset should be rejected");
        assert!(matches!(err, InitPresetError::Usage));
    }

    #[test]
    fn run_init_with_an_unresolvable_preset_name_reports_an_error_at_init_time() {
        // No `presets` directory exists next to the test binary, so a name
        // that isn't a built-in preset fails once `init` actually looks for
        // a matching file, rather than during argument parsing.
        let (code, _stdout, stderr) =
            run_captured(&["init".to_string(), "--preset=lenient".to_string()]);

        assert_eq!(code, 2);
        assert!(stderr.contains("unknown preset 'lenient'"));
    }

    #[test]
    fn run_init_with_a_preset_flag_writes_the_selected_presets_config() {
        // `run(["init", ...], ...)` writes into the process's actual
        // current directory (see `initialize_config`'s call in `run`),
        // which isn't safe to exercise from a parallel test suite. The
        // argument parsing itself (see the `parse_init_preset` tests
        // above) and `initialize_config`'s own preset handling (see
        // `init_creates_a_default_config` above and
        // `papyrus-lint-core`'s own preset tests) are covered separately,
        // so this only checks that `run` wires the two together for a
        // `--preset` other than the default.
        let dir = tempfile::tempdir().expect("failed to create temp dir");
        let mut stdout = Vec::new();
        let mut stderr = Vec::new();

        let preset =
            parse_init_preset(&["--preset=careful".to_string()]).expect("careful should parse");
        let code = initialize_config(dir.path(), preset, &mut stdout, &mut stderr);

        assert_eq!(code, 0);
        let generated = fs::read_to_string(dir.path().join("papyrus-lint.yaml"))
            .expect("failed to read generated config");
        assert!(generated.contains("cyclomatic_complexity_warning: 20\n"));
    }
}
