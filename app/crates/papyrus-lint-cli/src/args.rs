//! Parses and validates [`crate::run`]'s own arguments — a plain lint/fix
//! run, or `--blob <source>` — once `init`, `preset add`, and `doctor` (each
//! dispatched, and parsed, by their own module before `run` ever reaches
//! this) are ruled out. Split out from `run` so the parsing and validation
//! themselves are fully testable without touching the filesystem, the same
//! reasoning behind [`crate::init::parse_init_preset`]/
//! [`crate::init::parse_preset_add_args`].

use std::path::PathBuf;

use clap::Parser;

use crate::output::{normalize_tag_filter, ColorChoice, OutputFormat};

/// The main lint/fix/`--blob` invocation's flags and options, extracted by
/// `clap` before [`parse_run_args`]'s own semantic validation (mutually
/// exclusive flags, enum coercion, numeric ranges) runs on the raw values —
/// `clap` only owns recognizing `--flag`/`--flag=value`/`--flag value`,
/// repeatable options, and reporting a missing value, not the business
/// rules layered on top. `-h`/`--help` and `-V`/`--version` are declared
/// here too (rather than left to clap's own built-in handling) so
/// `parse_run_args` can keep reporting them the same way it always has.
/// `fix`'s target path, `fix` itself, and `--blob`'s own exclusivity with a
/// path/`fix` are all resolved from the loose `positionals` left over
/// afterward, since every flag above can appear before, after, or mixed in
/// with them in any order.
#[derive(Parser, Debug)]
#[command(
    no_binary_name = true,
    disable_help_flag = true,
    disable_version_flag = true
)]
struct RawArgs {
    #[arg(long, short = 'h')]
    help: bool,
    #[arg(long, short = 'V')]
    version: bool,
    #[arg(long)]
    json: bool,
    #[arg(long)]
    quiet_warnings: bool,
    #[arg(long)]
    quiet_info: bool,
    #[arg(long)]
    short_paths: bool,
    #[arg(long)]
    progress: bool,
    #[arg(long)]
    dry_run: bool,
    #[arg(long)]
    hash_source: bool,
    #[arg(long)]
    config: Option<String>,
    #[arg(long = "script-root")]
    script_root: Vec<String>,
    #[arg(long)]
    output: Option<String>,
    #[arg(long = "type")]
    type_filter: Option<String>,
    #[arg(long)]
    line: Option<String>,
    #[arg(long)]
    tag: Option<String>,
    #[arg(long)]
    format: Option<String>,
    #[arg(long)]
    color: Option<String>,
    #[arg(long)]
    threads: Option<String>,
    #[arg(long)]
    blob: Option<String>,
    positionals: Vec<String>,
}

/// Why [`parse_run_args`] rejected `args`. `Usage` covers every case
/// [`crate::run`] reports with the generic [`crate::USAGE`] text (a missing
/// flag value, or an unrecognized combination of positional arguments);
/// every other variant carries whatever its own more specific message
/// needs.
#[derive(Debug, PartialEq, Eq)]
pub(crate) enum ArgsError {
    Usage,
    JsonAndFormatConflict,
    InvalidFormat(String),
    HashSourceRequiresAi,
    InvalidColor(String),
    BlobWithPathArgument,
    BlobWithFixFlags,
    BlobWithScriptRootProgressThreads,
    UnknownTag(String),
    ProgressRequiresOutput,
    TypeAndTagConflict,
    UnknownRule(String),
    RuleHasNoFix(String),
    InvalidLine(String),
    InvalidThreads(String),
}

/// `--blob <source>`'s own arguments, already validated and normalized the
/// same way a plain lint/fix run's are, ready to hand to
/// [`crate::run_blob`].
#[derive(Debug, PartialEq)]
pub(crate) struct BlobArgs {
    pub(crate) source: String,
    pub(crate) config_path: Option<PathBuf>,
    pub(crate) output_format: OutputFormat,
    pub(crate) hash_source: bool,
    pub(crate) quiet_warnings: bool,
    pub(crate) quiet_info: bool,
    pub(crate) tag_filter: Option<String>,
    pub(crate) color_choice: ColorChoice,
    pub(crate) output_path: Option<PathBuf>,
}

/// A plain lint/fix run's fully parsed and validated arguments.
#[derive(Debug, PartialEq)]
pub(crate) struct LintArgs {
    pub(crate) fix: bool,
    pub(crate) input_path: PathBuf,
    pub(crate) output_format: OutputFormat,
    pub(crate) quiet_warnings: bool,
    pub(crate) quiet_info: bool,
    pub(crate) short_paths: bool,
    pub(crate) progress: bool,
    pub(crate) dry_run: bool,
    pub(crate) hash_source: bool,
    pub(crate) config_path: Option<PathBuf>,
    pub(crate) output_path: Option<PathBuf>,
    pub(crate) cli_script_roots: Vec<String>,
    pub(crate) tag_filter: Option<String>,
    pub(crate) rule_filter: Option<&'static str>,
    pub(crate) target_line: Option<usize>,
    pub(crate) color_choice: ColorChoice,
    pub(crate) thread_count: usize,
}

/// What [`parse_run_args`] parsed `args` into: `--version`/`-V` (reported
/// and exited before anything else is even looked at), `--blob <source>`,
/// or a plain lint/fix run.
#[derive(Debug, PartialEq)]
pub(crate) enum ParsedCommand {
    Version,
    Blob(BlobArgs),
    Lint(LintArgs),
}

/// Parses and validates `args` into a [`ParsedCommand`], performing every
/// usage check a plain lint/fix run or `--blob` needs before any of the
/// actual work (resolving paths, loading config, linting) begins. A single
/// pass: flags are pulled out of `args` first (in any order, mixed with the
/// positional path/`fix`), then what's left is validated as a whole, mirroring
/// [`crate::run`]'s previous inline parsing exactly so behavior (including
/// every error message) is unchanged.
pub(crate) fn parse_run_args(args: &[String]) -> Result<ParsedCommand, ArgsError> {
    let raw = RawArgs::try_parse_from(args).map_err(|_| ArgsError::Usage)?;

    if raw.help {
        return Err(ArgsError::Usage);
    }
    if raw.version {
        return Ok(ParsedCommand::Version);
    }

    let json_flag = raw.json;
    let quiet_warnings = raw.quiet_warnings;
    let quiet_info = raw.quiet_info;
    let short_paths = raw.short_paths;
    let progress = raw.progress;
    let dry_run = raw.dry_run;
    let hash_source = raw.hash_source;
    let config_path: Option<PathBuf> = raw.config.map(PathBuf::from);
    let output_path: Option<PathBuf> = raw.output.map(PathBuf::from);
    let cli_script_roots: Vec<String> = raw.script_root;
    let type_filter = raw.type_filter;
    let line_filter = raw.line;
    let tag_filter = raw.tag;
    let format_flag = raw.format;
    let color_flag = raw.color;
    let threads_flag = raw.threads;
    let blob_flag = raw.blob;
    let args = raw.positionals;

    if json_flag && format_flag.is_some() {
        return Err(ArgsError::JsonAndFormatConflict);
    }
    let output_format = match format_flag.as_deref() {
        None if json_flag => OutputFormat::Json,
        None | Some("plain") => OutputFormat::Plain,
        Some("json") => OutputFormat::Json,
        Some("ai") => OutputFormat::Ai,
        Some(value) => return Err(ArgsError::InvalidFormat(value.to_string())),
    };

    if hash_source && output_format != OutputFormat::Ai {
        return Err(ArgsError::HashSourceRequiresAi);
    }

    let color_choice = match color_flag.as_deref() {
        None | Some("auto") => ColorChoice::Auto,
        Some("always") => ColorChoice::Always,
        Some("never") => ColorChoice::Never,
        Some(value) => return Err(ArgsError::InvalidColor(value.to_string())),
    };

    if let Some(source) = blob_flag {
        if !args.is_empty() {
            return Err(ArgsError::BlobWithPathArgument);
        }
        if dry_run || type_filter.is_some() || line_filter.is_some() {
            return Err(ArgsError::BlobWithFixFlags);
        }
        if progress || !cli_script_roots.is_empty() || threads_flag.is_some() {
            return Err(ArgsError::BlobWithScriptRootProgressThreads);
        }
        let tag_filter = normalize_tag_filter(tag_filter).map_err(ArgsError::UnknownTag)?;
        return Ok(ParsedCommand::Blob(BlobArgs {
            source,
            config_path,
            output_format,
            hash_source,
            quiet_warnings,
            quiet_info,
            tag_filter,
            color_choice,
            output_path,
        }));
    }

    let (fix, input_path) = match args.as_slice() {
        [sub, path] if sub == "fix" => (true, PathBuf::from(path)),
        [path] if path != "fix" => (false, PathBuf::from(path)),
        _ => return Err(ArgsError::Usage),
    };

    if !fix && (type_filter.is_some() || line_filter.is_some() || dry_run) {
        return Err(ArgsError::Usage);
    }

    if progress && output_path.is_none() {
        return Err(ArgsError::ProgressRequiresOutput);
    }

    if type_filter.is_some() && tag_filter.is_some() {
        return Err(ArgsError::TypeAndTagConflict);
    }

    let tag_filter = normalize_tag_filter(tag_filter).map_err(ArgsError::UnknownTag)?;

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
                        return Err(ArgsError::RuleHasNoFix(value));
                    } else {
                        return Err(ArgsError::UnknownRule(value));
                    }
                }
            }
        }
        None => None,
    };

    let target_line: Option<usize> = match line_filter {
        Some(value) => match value.parse::<usize>() {
            Ok(line) if line >= 1 => Some(line),
            _ => return Err(ArgsError::InvalidLine(value)),
        },
        None => None,
    };

    let thread_count: usize = match threads_flag {
        Some(value) => match value.parse::<usize>() {
            Ok(threads) if threads >= 1 => threads,
            _ => return Err(ArgsError::InvalidThreads(value)),
        },
        None => papyrus_lint_core::parallel::default_thread_count(),
    };

    Ok(ParsedCommand::Lint(LintArgs {
        fix,
        input_path,
        output_format,
        quiet_warnings,
        quiet_info,
        short_paths,
        progress,
        dry_run,
        hash_source,
        config_path,
        output_path,
        cli_script_roots,
        tag_filter,
        rule_filter,
        target_line,
        color_choice,
        thread_count,
    }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::*;

    fn args(values: &[&str]) -> Vec<String> {
        values.iter().map(|value| value.to_string()).collect()
    }

    #[test]
    fn no_arguments_is_a_usage_error() {
        assert_eq!(parse_run_args(&[]), Err(ArgsError::Usage));
    }

    #[test]
    fn version_flag_parses_to_version() {
        assert_eq!(
            parse_run_args(&args(&["--version"])),
            Ok(ParsedCommand::Version)
        );
        assert_eq!(parse_run_args(&args(&["-V"])), Ok(ParsedCommand::Version));
    }

    #[test]
    fn help_flag_is_a_usage_error() {
        assert_eq!(parse_run_args(&args(&["--help"])), Err(ArgsError::Usage));
    }

    #[test]
    fn too_many_positional_arguments_is_a_usage_error() {
        assert_eq!(parse_run_args(&args(&["a", "b"])), Err(ArgsError::Usage));
    }

    #[test]
    fn a_single_path_parses_to_a_non_fix_lint_run() {
        let parsed = parse_run_args(&args(&["Example.psc"])).expect("should parse");
        match parsed {
            ParsedCommand::Lint(lint) => {
                assert!(!lint.fix);
                assert_eq!(lint.input_path, PathBuf::from("Example.psc"));
            }
            other => panic!("expected a lint run, got {other:?}"),
        }
    }

    #[test]
    fn fix_and_a_path_parses_to_a_fix_run() {
        let parsed = parse_run_args(&args(&["fix", "Example.psc"])).expect("should parse");
        match parsed {
            ParsedCommand::Lint(lint) => {
                assert!(lint.fix);
                assert_eq!(lint.input_path, PathBuf::from("Example.psc"));
            }
            other => panic!("expected a lint run, got {other:?}"),
        }
    }

    #[test]
    fn fix_without_a_path_is_a_usage_error() {
        assert_eq!(parse_run_args(&args(&["fix"])), Err(ArgsError::Usage));
    }

    #[test]
    fn parse_rejects_an_unknown_type_filter_rule_id() {
        assert_eq!(
            parse_run_args(&args(&["fix", "--type=made-up-rule", "Example.psc"])),
            Err(ArgsError::UnknownRule("made-up-rule".to_string()))
        );
    }

    #[test]
    fn parse_rejects_a_type_filter_rule_with_no_automatic_fix() {
        assert_eq!(
            parse_run_args(&args(&["fix", "--type=forbidden-functions", "Example.psc"])),
            Err(ArgsError::RuleHasNoFix("forbidden-functions".to_string()))
        );
    }

    #[test]
    fn type_filter_accepts_the_hyphenated_or_underscored_form() {
        let parsed = parse_run_args(&args(&["fix", "--type=trailing_whitespace", "Example.psc"]))
            .expect("should parse");
        match parsed {
            ParsedCommand::Lint(lint) => {
                assert_eq!(lint.rule_filter, Some("trailing-whitespace"));
            }
            other => panic!("expected a lint run, got {other:?}"),
        }
    }

    #[test]
    fn type_filter_without_fix_is_a_usage_error() {
        assert_eq!(
            parse_run_args(&args(&["--type=trailing-whitespace", "Example.psc"])),
            Err(ArgsError::Usage)
        );
    }

    #[test]
    fn line_filter_without_fix_is_a_usage_error() {
        assert_eq!(
            parse_run_args(&args(&["--line=1", "Example.psc"])),
            Err(ArgsError::Usage)
        );
    }

    #[test]
    fn parse_rejects_a_non_positive_line_filter() {
        assert_eq!(
            parse_run_args(&args(&["fix", "--line=0", "Example.psc"])),
            Err(ArgsError::InvalidLine("0".to_string()))
        );
    }

    #[test]
    fn parse_rejects_dry_run_without_fix() {
        assert_eq!(
            parse_run_args(&args(&["--dry-run", "Example.psc"])),
            Err(ArgsError::Usage)
        );
    }

    #[test]
    fn parse_rejects_an_unknown_tag_filter() {
        assert_eq!(
            parse_run_args(&args(&["--tag=made-up-tag", "Example.psc"])),
            Err(ArgsError::UnknownTag("made-up-tag".to_string()))
        );
    }

    #[test]
    fn tag_filter_matches_case_insensitively() {
        let parsed = parse_run_args(&args(&["--tag=STYLE", "Example.psc"])).expect("should parse");
        match parsed {
            ParsedCommand::Lint(lint) => {
                assert_eq!(lint.tag_filter.as_deref(), Some("style"));
            }
            other => panic!("expected a lint run, got {other:?}"),
        }
    }

    #[test]
    fn type_and_tag_filters_cannot_be_combined() {
        assert_eq!(
            parse_run_args(&args(&[
                "fix",
                "--type=trailing-whitespace",
                "--tag=style",
                "Example.psc"
            ])),
            Err(ArgsError::TypeAndTagConflict)
        );
    }

    #[test]
    fn progress_without_output_is_an_error() {
        assert_eq!(
            parse_run_args(&args(&["--progress", "Example.psc"])),
            Err(ArgsError::ProgressRequiresOutput)
        );
    }

    #[test]
    fn progress_with_output_parses() {
        let parsed = parse_run_args(&args(&[
            "--progress",
            "--output",
            "report.txt",
            "Example.psc",
        ]))
        .expect("should parse");
        match parsed {
            ParsedCommand::Lint(lint) => assert!(lint.progress),
            other => panic!("expected a lint run, got {other:?}"),
        }
    }

    #[test]
    fn parse_rejects_a_non_positive_threads_flag() {
        assert_eq!(
            parse_run_args(&args(&["--threads=0", "Example.psc"])),
            Err(ArgsError::InvalidThreads("0".to_string()))
        );
    }

    #[test]
    fn parse_rejects_a_non_numeric_threads_flag() {
        assert_eq!(
            parse_run_args(&args(&["--threads", "many", "Example.psc"])),
            Err(ArgsError::InvalidThreads("many".to_string()))
        );
    }

    #[test]
    fn json_and_format_cannot_be_combined() {
        assert_eq!(
            parse_run_args(&args(&["--json", "--format=json", "Example.psc"])),
            Err(ArgsError::JsonAndFormatConflict)
        );
    }

    #[test]
    fn format_flag_rejects_an_unknown_value() {
        assert_eq!(
            parse_run_args(&args(&["--format=yaml", "Example.psc"])),
            Err(ArgsError::InvalidFormat("yaml".to_string()))
        );
    }

    #[test]
    fn hash_source_without_ai_format_is_an_error() {
        assert_eq!(
            parse_run_args(&args(&["--hash-source", "Example.psc"])),
            Err(ArgsError::HashSourceRequiresAi)
        );
    }

    #[test]
    fn color_flag_rejects_an_unknown_value() {
        assert_eq!(
            parse_run_args(&args(&["--color", "rainbow", "Example.psc"])),
            Err(ArgsError::InvalidColor("rainbow".to_string()))
        );
    }

    #[test]
    fn blob_cannot_be_combined_with_a_path_argument() {
        assert_eq!(
            parse_run_args(&args(&["--blob", "ScriptName Example", "some/path.psc"])),
            Err(ArgsError::BlobWithPathArgument)
        );
    }

    #[test]
    fn blob_cannot_be_combined_with_fix_type_line_or_dry_run() {
        for flag in ["--dry-run", "--type=trailing-whitespace", "--line=1"] {
            assert_eq!(
                parse_run_args(&args(&[flag, "--blob", "ScriptName Example"])),
                Err(ArgsError::BlobWithFixFlags),
                "flag {flag} should have been rejected"
            );
        }
    }

    #[test]
    fn blob_cannot_be_combined_with_script_root_progress_or_threads() {
        assert_eq!(
            parse_run_args(&args(&[
                "--script-root",
                "other",
                "--blob",
                "ScriptName Example"
            ])),
            Err(ArgsError::BlobWithScriptRootProgressThreads)
        );
        assert_eq!(
            parse_run_args(&args(&[
                "--progress",
                "--output",
                "out.txt",
                "--blob",
                "ScriptName Example"
            ])),
            Err(ArgsError::BlobWithScriptRootProgressThreads)
        );
        assert_eq!(
            parse_run_args(&args(&["--threads=2", "--blob", "ScriptName Example"])),
            Err(ArgsError::BlobWithScriptRootProgressThreads)
        );
    }

    #[test]
    fn blob_rejects_an_unknown_tag() {
        assert_eq!(
            parse_run_args(&args(&[
                "--tag=made-up-tag",
                "--blob",
                "ScriptName Example"
            ])),
            Err(ArgsError::UnknownTag("made-up-tag".to_string()))
        );
    }

    #[test]
    fn blob_parses_into_its_own_args() {
        let parsed =
            parse_run_args(&args(&["--blob", "ScriptName Example"])).expect("should parse");
        match parsed {
            ParsedCommand::Blob(blob) => assert_eq!(blob.source, "ScriptName Example"),
            other => panic!("expected a blob run, got {other:?}"),
        }
    }

    #[test]
    fn parse_reports_usage_when_separate_value_flags_are_missing() {
        for flag in ["--line", "--tag", "--format", "--threads"] {
            assert_eq!(
                parse_run_args(&args(&[flag])),
                Err(ArgsError::Usage),
                "unexpected result for {flag}"
            );
        }
    }

    #[test]
    fn config_flag_without_a_value_is_a_usage_error() {
        assert_eq!(parse_run_args(&args(&["--config"])), Err(ArgsError::Usage));
    }

    #[test]
    fn script_root_flag_without_a_value_is_a_usage_error() {
        assert_eq!(
            parse_run_args(&args(&["--script-root"])),
            Err(ArgsError::Usage)
        );
    }

    #[test]
    fn threads_flag_without_a_value_is_a_usage_error() {
        assert_eq!(parse_run_args(&args(&["--threads"])), Err(ArgsError::Usage));
    }

    // Below: integration-level checks (via `run`, i.e. `crate::run`) of the
    // same usage errors covered by the pure unit tests above, moved
    // unchanged from `lib.rs`'s own test module so `run`'s actual wiring of
    // `parse_run_args`'s outcomes (the exact text written to stderr/stdout,
    // and that a rejected `fix`/`--dry-run` never touches the file) stays
    // covered too.
    use crate::USAGE;
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
        assert_eq!(stdout, format!("PapyrusLinterCLI {}\n", crate::VERSION));
    }

    #[test]
    fn prints_version_for_short_version_flag() {
        let (code, stdout, _stderr) = run_captured(&["-V".to_string()]);

        assert_eq!(code, 0);
        assert_eq!(stdout, format!("PapyrusLinterCLI {}\n", crate::VERSION));
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
