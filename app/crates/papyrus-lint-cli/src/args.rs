//! Command-line argument parsing for `PapyrusLinterCLI`.
//!
//! [`parse_cli`] turns the process arguments (excluding the binary name)
//! into a [`ParsedCli`] value [`crate::run`] can dispatch on, so flag
//! handling lives in one place instead of being inlined into the lint
//! path. Usage errors either point at [`USAGE`] or carry a specific
//! `error: ...` message (see [`ParseError`]).

use std::io::Write;
use std::path::PathBuf;

use papyrus_lint_core::config;

use crate::init::{parse_init_preset, parse_preset_add_args, InitPresetError, PresetAddArgsError};
use crate::output::{normalize_tag_filter, ColorChoice, OutputFormat};

pub const USAGE: &str =
    "Usage: PapyrusLinterCLI [--json | --format <plain|json|ai>] [--hash-source] [--quiet-warnings] [--quiet-info] [--short-paths] [--config <path>] [--script-root <path>]... [--output <path>] [--progress] [--threads <n>] [--tag <kind>] <path-to-achlist-or-psc-or-directory>\n       \
PapyrusLinterCLI [--json | --format <plain|json|ai>] [--hash-source] [--quiet-warnings] [--quiet-info] [--config <path>] [--output <path>] [--color <when>] [--tag <kind>] --blob <source>\n       \
PapyrusLinterCLI [--json | --format <plain|json|ai>] [--hash-source] [--quiet-warnings] [--quiet-info] [--short-paths] [--config <path>] [--script-root <path>]... [--output <path>] [--progress] [--threads <n>] fix [--type <rule-id> | --tag <kind>] [--line <n>] [--dry-run] <path-to-achlist-or-psc-or-directory>\n\n\
PapyrusLinterCLI init [--preset <strict|standard|careful|custom-name>]\n\n\
PapyrusLinterCLI preset add <name> <path-to-papyrus-lint.yaml> [--yes]\n\n\
PapyrusLinterCLI doctor [--json] [--config <path>] [--script-root <path>]... <path-to-achlist-or-psc-or-directory>\n\n\
Lints every .psc script listed in the given .achlist file, a single\n\
.psc file given directly, or every .psc file found recursively under a\n\
given directory (any depth of subfolders), using the project's\n\
papyrus-lint.yaml/.yml configuration (looked up next to the .achlist\n\
file or scanned directory, or two directories up from a bare .psc file,\n\
e.g. Data for Data\\Scripts\\Source\\abc.psc; falling back to defaults\n\
if it has none).\n\n\
With the `fix` subcommand, applies every automatic fix (see README.md)\n\
to those scripts first, rewriting each one on disk if it changed, then\n\
reports whatever diagnostics remain the same way. With --dry-run, no\n\
file is written; a standard diff of what would have changed is printed\n\
instead.\n\n\
With the `init` subcommand, creates a papyrus-lint.yaml in the current\n\
working directory without overwriting an existing config, from the\n\
selected --preset (strict, standard, or careful; defaults to strict,\n\
identical to today's built-in default; any other name is looked up as\n\
<name>.yaml/.yml in a presets directory next to the executable).\n\n\
With the `preset add` subcommand, adds a user preset named <name> by\n\
copying <path-to-papyrus-lint.yaml> into a presets directory next to the\n\
executable, so it becomes selectable via --preset <name> just like a\n\
built-in preset. Refuses a blank name or one matching a built-in preset\n\
(strict, standard, careful). Refuses to overwrite an existing preset\n\
of the same name unless --yes is also given.\n\n\
With the `doctor` subcommand, validates a project's configuration and the\n\
paths it assumes or names (conventional script directories, configured\n\
additional_script_roots/lookup_script_roots/compiler_path, and an achlist's own listed entries)\n\
without linting any script, accepting the same --config/--script-root\n\
flags as a normal run and printing one [ok]/[warning]/[error] line per\n\
check (or a single JSON document with --json).\n\n\
With --blob <source>, lints <source> itself as raw Papyrus source text\n\
instead of resolving an achlist/.psc/directory path, e.g. for a script\n\
buffer that isn't (yet) saved to disk. Reported as the literal path\n\
<blob>. Can't be combined with a path argument, `fix`, --type, --line,\n\
--dry-run, --script-root, --progress, or --threads.\n\n\
Options:\n\
  -h, --help              Show this help message\n\
  -V, --version           Print the PapyrusLinterCLI version\n\
  --json                  Print the report as JSON (alias for --format json)\n\
  --format <format>       Print as plain text, JSON, or a self-contained AI export\n\
                          with source, diagnostics, triggered-rule details, and\n\
                          tool/version metadata—everything an AI needs to assist\n\
                          (plain, json, or ai)\n\
  --hash-source           --format ai only: report each file's source as an\n\
                          md5 hash instead of its full content, e.g. to avoid\n\
                          exposing proprietary script text to an external AI.\n\
                          A usage error without --format ai.\n\
  --quiet-warnings        Hide warning-level diagnostics from the report\n\
  --quiet-info            Hide info-level diagnostics from the report\n\
  --short-paths           Strip the project root from each script's path in\n\
                          the report, the same way the desktop app shortens\n\
                          paths in its results list\n\
  --config <path>         Load lint configuration from this file instead of\n\
                          discovering papyrus-lint.yaml/.yml from the project root\n\
                          (also disables the project root's additional_script_roots;\n\
                          use --script-root to add any back explicitly.\n\
                          lookup_script_roots is still read from this file)\n\
  --script-root <path>    An extra directory (relative to the project root,\n\
                          or absolute) to search for .psc files, besides\n\
                          scripts/source, source/scripts, and the project's\n\
                          configured additional_script_roots. Repeatable.\n\
  --output <path>         Write the report (plain text or JSON, per --json) to\n\
                          this file instead of stdout.\n\
  --progress              Print a live files-linted/total-files progress bar\n\
                          to stdout as each script finishes. Requires --output\n\
                          (a usage error otherwise, since the report itself\n\
                          would otherwise also be writing to stdout).\n\
  --color <when>          Colorize the plain-text report: auto (default),\n\
                          always, or never. auto colors only when stdout is a\n\
                          terminal, --output isn't used, and NO_COLOR is unset.\n\
  --threads <n>           Read/fix/lint up to <n> scripts concurrently instead\n\
                          of one at a time. Defaults to the machine's available\n\
                          parallelism; --threads 1 forces sequential processing.\n\
                          Output is always reassembled in the same order\n\
                          regardless of thread count.\n\
  --type <rule-id>        fix only: apply only this rule's automatic fix\n\
                          (e.g. trailing-whitespace or trailing_whitespace)\n\
                          instead of every enabled one.\n\
  --line <n>              fix only: apply the selected fix(es) only to this\n\
                          1-indexed line, leaving every other line untouched.\n\
                          Combinable with --type. Errors if the fix would\n\
                          change the file's line count (e.g. property-sorting\n\
                          or unused-import).\n\
  --dry-run               fix only: don't write any changes to disk; print a\n\
                          standard diff of what would change instead.\n\
  --tag <kind>            Restrict to rules tagged with this kind (e.g. style,\n\
                          performance, correctness, maintainability), matched\n\
                          case-insensitively. Without fix, limits the reported\n\
                          diagnostics; with fix, also limits which automatic\n\
                          fixes run. Valid with or without fix. Can't be\n\
                          combined with --type.\n\
  --blob <source>         Lint <source> directly as raw Papyrus source text\n\
                          instead of a path, reported as <blob>. Can't be\n\
                          combined with a path argument, fix, --type, --line,\n\
                          --dry-run, --script-root, --progress, or --threads.\n\
  --preset <name>         init only: the baseline papyrus-lint.yaml to\n\
                          generate (strict, standard, or careful; see\n\
                          README.md). Defaults to strict, identical to the\n\
                          built-in default. Any other name is looked up as\n\
                          <name>.yaml/.yml in a presets directory next to\n\
                          the executable.\n\
  --yes                   preset add only: confirm overwriting an existing\n\
                          preset of the same name. Without it, an existing\n\
                          preset is left untouched and an error is reported.\n\n\
Exit status: 0 if no problems were found (or none met the configured\n\
fail_on_warning/fail_on_info threshold), 1 if any did, 2 on a usage or\n\
I/O error.\n\n\
Contact:\n\
  Discord    https://discord.gg/idrinth\n\
  NexusMods  https://www.nexusmods.com/skyrimspecialedition/mods/189862\n\
  GitHub     https://github.com/idrinth/papyrus-lint\n";

/// Why [`parse_cli`] rejected the given arguments.
#[derive(Debug, PartialEq, Eq)]
pub(crate) enum ParseError {
    /// Missing required values, an unrecognized combination of
    /// positionals, or `--help`/`-h`. Reported as the generic [`USAGE`]
    /// text, matching every other usage error this CLI prints.
    Usage,
    /// A specific `error: ...` line, already including the `error: `
    /// prefix so [`write_parse_error`] can print it as-is.
    Message(String),
}

/// A fully parsed invocation of the CLI, ready for [`crate::run`] to
/// dispatch without re-inspecting the raw argument list.
#[derive(Debug)]
pub(crate) enum ParsedCli {
    Init {
        preset: config::Preset,
    },
    PresetAdd {
        name: String,
        source_path: PathBuf,
        overwrite: bool,
    },
    /// Remaining arguments after the `doctor` subcommand itself, parsed
    /// by [`crate::doctor::run_doctor`].
    Doctor {
        rest: Vec<String>,
    },
    Version,
    Blob(BlobArgs),
    Lint(LintArgs),
}

/// Flags shared by `--blob` and a normal lint/fix run.
#[derive(Debug)]
pub(crate) struct SharedFlags {
    pub output_format: OutputFormat,
    pub hash_source: bool,
    pub quiet_warnings: bool,
    pub quiet_info: bool,
    pub color_choice: ColorChoice,
    pub config_path: Option<PathBuf>,
    pub output_path: Option<PathBuf>,
    pub tag_filter: Option<String>,
}

/// Parsed `--blob <source>` invocation.
#[derive(Debug)]
pub(crate) struct BlobArgs {
    pub source: String,
    pub flags: SharedFlags,
}

/// Parsed lint or `fix` invocation against an achlist, `.psc`, or directory.
#[derive(Debug)]
pub(crate) struct LintArgs {
    pub fix: bool,
    pub input_path: PathBuf,
    pub flags: SharedFlags,
    pub short_paths: bool,
    pub progress: bool,
    pub dry_run: bool,
    pub cli_script_roots: Vec<String>,
    pub rule_filter: Option<&'static str>,
    pub target_line: Option<usize>,
    pub thread_count: usize,
}

/// Writes a [`ParseError`] to `stderr` the same way [`crate::run`] used
/// to inline the equivalent `write!`/`writeln!` calls.
pub(crate) fn write_parse_error(err: &ParseError, stderr: &mut impl Write) {
    match err {
        ParseError::Usage => {
            let _ = write!(stderr, "{USAGE}");
        }
        ParseError::Message(message) => {
            let _ = writeln!(stderr, "{message}");
        }
    }
}

/// Parses `args` (the program's arguments, excluding the binary name)
/// into a [`ParsedCli`]. Does not touch the filesystem except for
/// looking up the default thread count when `--threads` is omitted.
pub(crate) fn parse_cli(args: &[String]) -> Result<ParsedCli, ParseError> {
    if args.first().map(String::as_str) == Some("init") {
        let preset = match parse_init_preset(&args[1..]) {
            Ok(preset) => preset,
            Err(InitPresetError::Usage) => return Err(ParseError::Usage),
        };
        return Ok(ParsedCli::Init { preset });
    }

    if args.first().map(String::as_str) == Some("preset") {
        if args.get(1).map(String::as_str) != Some("add") {
            return Err(ParseError::Usage);
        }
        let (name, source_path, overwrite) = match parse_preset_add_args(&args[2..]) {
            Ok(parsed) => parsed,
            Err(PresetAddArgsError::Usage) => return Err(ParseError::Usage),
        };
        return Ok(ParsedCli::PresetAdd {
            name,
            source_path,
            overwrite,
        });
    }

    if args.first().map(String::as_str) == Some("doctor") {
        return Ok(ParsedCli::Doctor {
            rest: args[1..].to_vec(),
        });
    }

    let json_flag = args.iter().any(|arg| arg == "--json");
    let quiet_warnings = args.iter().any(|arg| arg == "--quiet-warnings");
    let quiet_info = args.iter().any(|arg| arg == "--quiet-info");
    let short_paths = args.iter().any(|arg| arg == "--short-paths");
    let progress = args.iter().any(|arg| arg == "--progress");
    let dry_run = args.iter().any(|arg| arg == "--dry-run");
    let hash_source = args.iter().any(|arg| arg == "--hash-source");

    let mut config_path: Option<PathBuf> = None;
    let mut output_path: Option<PathBuf> = None;
    let mut cli_script_roots: Vec<String> = Vec::new();
    let mut type_filter: Option<String> = None;
    let mut line_filter: Option<String> = None;
    let mut tag_filter: Option<String> = None;
    let mut color_flag: Option<String> = None;
    let mut format_flag: Option<String> = None;
    let mut threads_flag: Option<String> = None;
    let mut blob_flag: Option<String> = None;
    let mut positional_and_flags: Vec<String> = Vec::with_capacity(args.len());
    let mut input = args
        .iter()
        .filter(|arg| {
            !matches!(
                arg.as_str(),
                "--json"
                    | "--quiet-warnings"
                    | "--quiet-info"
                    | "--short-paths"
                    | "--progress"
                    | "--dry-run"
                    | "--hash-source"
            )
        })
        .cloned();
    while let Some(arg) = input.next() {
        if arg == "--config" {
            let Some(value) = input.next() else {
                return Err(ParseError::Usage);
            };
            config_path = Some(PathBuf::from(value));
        } else if arg == "--script-root" {
            let Some(value) = input.next() else {
                return Err(ParseError::Usage);
            };
            cli_script_roots.push(value);
        } else if arg == "--output" {
            let Some(value) = input.next() else {
                return Err(ParseError::Usage);
            };
            output_path = Some(PathBuf::from(value));
        } else if arg == "--type" {
            let Some(value) = input.next() else {
                return Err(ParseError::Usage);
            };
            type_filter = Some(value);
        } else if let Some(value) = arg.strip_prefix("--type=") {
            type_filter = Some(value.to_string());
        } else if arg == "--line" {
            let Some(value) = input.next() else {
                return Err(ParseError::Usage);
            };
            line_filter = Some(value);
        } else if let Some(value) = arg.strip_prefix("--line=") {
            line_filter = Some(value.to_string());
        } else if arg == "--tag" {
            let Some(value) = input.next() else {
                return Err(ParseError::Usage);
            };
            tag_filter = Some(value);
        } else if let Some(value) = arg.strip_prefix("--tag=") {
            tag_filter = Some(value.to_string());
        } else if arg == "--format" {
            let Some(value) = input.next() else {
                return Err(ParseError::Usage);
            };
            format_flag = Some(value);
        } else if let Some(value) = arg.strip_prefix("--format=") {
            format_flag = Some(value.to_string());
        } else if arg == "--color" {
            let Some(value) = input.next() else {
                return Err(ParseError::Usage);
            };
            color_flag = Some(value);
        } else if let Some(value) = arg.strip_prefix("--color=") {
            color_flag = Some(value.to_string());
        } else if arg == "--threads" {
            let Some(value) = input.next() else {
                return Err(ParseError::Usage);
            };
            threads_flag = Some(value);
        } else if let Some(value) = arg.strip_prefix("--threads=") {
            threads_flag = Some(value.to_string());
        } else if arg == "--blob" {
            let Some(value) = input.next() else {
                return Err(ParseError::Usage);
            };
            blob_flag = Some(value);
        } else if let Some(value) = arg.strip_prefix("--blob=") {
            blob_flag = Some(value.to_string());
        } else {
            positional_and_flags.push(arg);
        }
    }
    let args = positional_and_flags;

    if json_flag && format_flag.is_some() {
        return Err(ParseError::Message(
            "error: --json and --format can't be combined".to_string(),
        ));
    }
    let output_format = match format_flag.as_deref() {
        None if json_flag => OutputFormat::Json,
        None | Some("plain") => OutputFormat::Plain,
        Some("json") => OutputFormat::Json,
        Some("ai") => OutputFormat::Ai,
        Some(value) => {
            return Err(ParseError::Message(format!(
                "error: --format must be 'plain', 'json', or 'ai', got '{value}'"
            )));
        }
    };

    if hash_source && output_format != OutputFormat::Ai {
        return Err(ParseError::Message(
            "error: --hash-source requires --format ai".to_string(),
        ));
    }

    let color_choice = match color_flag.as_deref() {
        None | Some("auto") => ColorChoice::Auto,
        Some("always") => ColorChoice::Always,
        Some("never") => ColorChoice::Never,
        Some(value) => {
            return Err(ParseError::Message(format!(
                "error: --color must be 'auto', 'always', or 'never', got '{value}'"
            )));
        }
    };

    let tag_filter = match normalize_tag_filter(tag_filter) {
        Ok(tag_filter) => tag_filter,
        Err(value) => {
            return Err(ParseError::Message(format!("error: unknown tag '{value}'")));
        }
    };

    let flags = SharedFlags {
        output_format,
        hash_source,
        quiet_warnings,
        quiet_info,
        color_choice,
        config_path,
        output_path,
        tag_filter,
    };

    if let Some(source) = blob_flag {
        if !args.is_empty() {
            return Err(ParseError::Message(
                "error: --blob can't be combined with a path argument (or `fix`)".to_string(),
            ));
        }
        if dry_run || type_filter.is_some() || line_filter.is_some() {
            return Err(ParseError::Message(
                "error: --blob can't be combined with fix/--type/--line/--dry-run".to_string(),
            ));
        }
        if progress || !cli_script_roots.is_empty() || threads_flag.is_some() {
            return Err(ParseError::Message(
                "error: --blob can't be combined with --script-root/--progress/--threads"
                    .to_string(),
            ));
        }
        return Ok(ParsedCli::Blob(BlobArgs { source, flags }));
    }

    let (fix, input_path) = match args.as_slice() {
        [flag] if flag == "--version" || flag == "-V" => {
            return Ok(ParsedCli::Version);
        }
        [sub, path] if sub == "fix" => (true, PathBuf::from(path)),
        [path] if path != "-h" && path != "--help" && path != "fix" => {
            (false, PathBuf::from(path))
        }
        _ => return Err(ParseError::Usage),
    };

    if !fix && (type_filter.is_some() || line_filter.is_some() || dry_run) {
        return Err(ParseError::Usage);
    }

    if progress && flags.output_path.is_none() {
        return Err(ParseError::Message(
            "error: --progress requires --output <path>".to_string(),
        ));
    }

    if type_filter.is_some() && flags.tag_filter.is_some() {
        return Err(ParseError::Message(
            "error: --type and --tag can't be combined".to_string(),
        ));
    }

    let rule_filter: Option<&'static str> = match type_filter {
        Some(value) => {
            let normalized = value.replace('_', "-").to_ascii_lowercase();
            match papyrus_lints::FIXABLE_RULE_IDS
                .iter()
                .find(|rule| **rule == normalized)
            {
                Some(rule) => Some(*rule),
                None => {
                    if papyrus_lints::KNOWN_RULE_IDS
                        .iter()
                        .any(|rule| *rule == normalized)
                    {
                        return Err(ParseError::Message(format!(
                            "error: rule '{value}' has no automatic fix"
                        )));
                    } else {
                        return Err(ParseError::Message(format!(
                            "error: unknown rule '{value}'"
                        )));
                    }
                }
            }
        }
        None => None,
    };

    let target_line: Option<usize> = match line_filter {
        Some(value) => match value.parse::<usize>() {
            Ok(line) if line >= 1 => Some(line),
            _ => {
                return Err(ParseError::Message(format!(
                    "error: --line must be a positive integer, got '{value}'"
                )));
            }
        },
        None => None,
    };

    // Defaults to the machine's available parallelism, matching the same
    // default the desktop app's own already-concurrent per-file Tauri
    // commands get for free from Tauri's blocking thread pool (see
    // `papyrus_lint_core::parallel`'s module docs). `--threads 1` forces
    // fully sequential processing, e.g. for easier-to-reproduce diagnostics
    // or a constrained CI runner.
    let thread_count: usize = match threads_flag {
        Some(value) => match value.parse::<usize>() {
            Ok(threads) if threads >= 1 => threads,
            _ => {
                return Err(ParseError::Message(format!(
                    "error: --threads must be a positive integer, got '{value}'"
                )));
            }
        },
        None => papyrus_lint_core::parallel::default_thread_count(),
    };

    Ok(ParsedCli::Lint(LintArgs {
        fix,
        input_path,
        flags,
        short_paths,
        progress,
        dry_run,
        cli_script_roots,
        rule_filter,
        target_line,
        thread_count,
    }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::*;
    use crate::VERSION;
    use std::fs;

    #[test]
    fn prints_usage_and_exits_2_with_no_arguments() {
        let (code, _stdout, stderr) = run_captured(&[]);

        assert_eq!(code, 2);
        assert!(stderr.contains("Usage: PapyrusLinterCLI"));
    }

    #[test]
    fn prints_version_for_version_flag() {
        let (code, stdout, _stderr) = run_captured(&["--version".to_string()]);

        assert_eq!(code, 0);
        assert_eq!(stdout, format!("PapyrusLinterCLI {VERSION}\n"));
    }

    #[test]
    fn prints_version_for_short_version_flag() {
        let (code, stdout, _stderr) = run_captured(&["-V".to_string()]);

        assert_eq!(code, 0);
        assert_eq!(stdout, format!("PapyrusLinterCLI {VERSION}\n"));
    }

    #[test]
    fn prints_usage_for_help_flag() {
        let (code, _stdout, stderr) = run_captured(&["--help".to_string()]);

        assert_eq!(code, 2);
        assert!(stderr.contains("Usage: PapyrusLinterCLI"));
        assert!(stderr.contains("everything an AI needs to assist"));
    }

    #[test]
    fn prints_usage_with_too_many_arguments() {
        let (code, _stdout, stderr) = run_captured(&["a".to_string(), "b".to_string()]);

        assert_eq!(code, 2);
        assert!(stderr.contains("Usage: PapyrusLinterCLI"));
    }

    #[test]
    fn type_filter_rejects_an_unknown_rule_id() {
        let dir = tempfile::tempdir().expect("failed to create temp dir");
        let script_path = dir.path().join("Example.psc");
        write_file(&script_path, "ScriptName Example\n");

        let (code, _stdout, stderr) = run_captured(&[
            "fix".to_string(),
            "--type=made-up-rule".to_string(),
            script_path.to_string_lossy().into_owned(),
        ]);

        assert_eq!(code, 2);
        assert!(stderr.contains("unknown rule 'made-up-rule'"));
    }

    #[test]
    fn type_filter_rejects_a_rule_with_no_automatic_fix() {
        let dir = tempfile::tempdir().expect("failed to create temp dir");
        let script_path = dir.path().join("Example.psc");
        write_file(&script_path, "ScriptName Example\n");

        let (code, _stdout, stderr) = run_captured(&[
            "fix".to_string(),
            "--type=forbidden-functions".to_string(),
            script_path.to_string_lossy().into_owned(),
        ]);

        assert_eq!(code, 2);
        assert!(stderr.contains("rule 'forbidden-functions' has no automatic fix"));
    }

    #[test]
    fn type_filter_without_fix_prints_usage() {
        let dir = tempfile::tempdir().expect("failed to create temp dir");
        let script_path = dir.path().join("Example.psc");
        write_file(&script_path, "ScriptName Example\n");

        let (code, _stdout, stderr) = run_captured(&[
            "--type=trailing-whitespace".to_string(),
            script_path.to_string_lossy().into_owned(),
        ]);

        assert_eq!(code, 2);
        assert!(stderr.contains("Usage: PapyrusLinterCLI"));
    }

    #[test]
    fn tag_and_type_filters_cannot_be_combined() {
        let dir = tempfile::tempdir().expect("failed to create temp dir");
        let script_path = dir.path().join("Example.psc");
        write_file(&script_path, "ScriptName Example\n");

        let (code, _stdout, stderr) = run_captured(&[
            "fix".to_string(),
            "--type=trailing-whitespace".to_string(),
            "--tag=style".to_string(),
            script_path.to_string_lossy().into_owned(),
        ]);

        assert_eq!(code, 2);
        assert!(stderr.contains("--type and --tag can't be combined"));
    }

    #[test]
    fn tag_filter_rejects_an_unknown_tag() {
        let dir = tempfile::tempdir().expect("failed to create temp dir");
        let script_path = dir.path().join("Example.psc");
        write_file(&script_path, "ScriptName Example\n");

        let (code, _stdout, stderr) = run_captured(&[
            "--tag=made-up-tag".to_string(),
            script_path.to_string_lossy().into_owned(),
        ]);

        assert_eq!(code, 2);
        assert!(stderr.contains("unknown tag 'made-up-tag'"));
    }

    #[test]
    fn line_filter_without_fix_prints_usage() {
        let dir = tempfile::tempdir().expect("failed to create temp dir");
        let script_path = dir.path().join("Example.psc");
        write_file(&script_path, "ScriptName Example\n");

        let (code, _stdout, stderr) = run_captured(&[
            "--line=1".to_string(),
            script_path.to_string_lossy().into_owned(),
        ]);

        assert_eq!(code, 2);
        assert!(stderr.contains("Usage: PapyrusLinterCLI"));
    }

    #[test]
    fn line_filter_rejects_a_non_positive_value() {
        let dir = tempfile::tempdir().expect("failed to create temp dir");
        let script_path = dir.path().join("Example.psc");
        write_file(&script_path, "ScriptName Example\n");

        let (code, _stdout, stderr) = run_captured(&[
            "fix".to_string(),
            "--line=0".to_string(),
            script_path.to_string_lossy().into_owned(),
        ]);

        assert_eq!(code, 2);
        assert!(stderr.contains("--line must be a positive integer"));
    }

    #[test]
    fn threads_flag_rejects_a_non_positive_value() {
        let dir = tempfile::tempdir().expect("failed to create temp dir");
        let script_path = dir.path().join("Example.psc");
        write_file(&script_path, "ScriptName Example\n");

        let (code, _stdout, stderr) = run_captured(&[
            "--threads=0".to_string(),
            script_path.to_string_lossy().into_owned(),
        ]);

        assert_eq!(code, 2);
        assert!(stderr.contains("--threads must be a positive integer"));
    }

    #[test]
    fn threads_flag_rejects_a_non_numeric_value() {
        let dir = tempfile::tempdir().expect("failed to create temp dir");
        let script_path = dir.path().join("Example.psc");
        write_file(&script_path, "ScriptName Example\n");

        let (code, _stdout, stderr) = run_captured(&[
            "--threads".to_string(),
            "many".to_string(),
            script_path.to_string_lossy().into_owned(),
        ]);

        assert_eq!(code, 2);
        assert!(stderr.contains("--threads must be a positive integer"));
    }

    #[test]
    fn threads_flag_without_a_value_prints_usage() {
        let (code, _stdout, stderr) = run_captured(&["--threads".to_string()]);

        assert_eq!(code, 2);
        assert!(stderr.contains("Usage: PapyrusLinterCLI"));
    }

    #[test]
    fn dry_run_without_fix_is_a_usage_error() {
        let dir = tempfile::tempdir().expect("failed to create temp dir");
        let script_path = dir.path().join("Example.psc");
        write_file(&script_path, "ScriptName Example   \n");

        let (code, _stdout, stderr) = run_captured(&[
            "--dry-run".to_string(),
            script_path.to_string_lossy().into_owned(),
        ]);

        assert_eq!(code, 2);
        assert!(stderr.contains("Usage: PapyrusLinterCLI"));
        assert_eq!(
            fs::read_to_string(&script_path).unwrap(),
            "ScriptName Example   \n"
        );
    }

    #[test]
    fn prints_usage_when_fix_is_given_without_a_path() {
        let (code, _stdout, stderr) = run_captured(&["fix".to_string()]);

        assert_eq!(code, 2);
        assert!(stderr.contains("Usage: PapyrusLinterCLI"));
    }

    #[test]
    fn script_root_flag_without_a_value_prints_usage() {
        let (code, _stdout, stderr) = run_captured(&["--script-root".to_string()]);

        assert_eq!(code, 2);
        assert!(stderr.contains("Usage: PapyrusLinterCLI"));
    }

    #[test]
    fn config_flag_without_a_value_prints_usage() {
        let (code, _stdout, stderr) = run_captured(&["--config".to_string()]);

        assert_eq!(code, 2);
        assert!(stderr.contains("Usage: PapyrusLinterCLI"));
    }

    #[test]
    fn value_flags_report_usage_when_their_separate_value_is_missing() {
        for flag in ["--line", "--tag", "--format", "--threads"] {
            let (code, stdout, stderr) = run_captured(&[flag.to_string()]);

            assert_eq!(code, 2, "unexpected exit code for {flag}");
            assert!(stdout.is_empty(), "unexpected stdout for {flag}");
            assert_eq!(stderr, USAGE, "unexpected stderr for {flag}");
        }
    }
}
