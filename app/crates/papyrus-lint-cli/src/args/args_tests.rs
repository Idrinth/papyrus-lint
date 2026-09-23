use super::*;
use crate::test_support::*;

fn args(values: &[&str]) -> Vec<String> {
    values.iter().map(|value| value.to_string()).collect()
}

fn parse_lint(values: &[String]) -> Result<ParsedCommand, ArgsError> {
    parse_subcommand("lint", values)
}

fn parse_fix(values: &[String]) -> Result<ParsedCommand, ArgsError> {
    parse_subcommand("fix", values)
}

fn parse_subcommand(subcommand: &str, values: &[String]) -> Result<ParsedCommand, ArgsError> {
    let mut argv = vec![subcommand.to_string()];
    argv.extend(values.iter().cloned());
    match parse_cli(&argv)? {
        ParsedCli::Run(command) => Ok(command),
        other => panic!("expected a {subcommand} run, got {other:?}"),
    }
}

#[test]
fn no_arguments_is_a_usage_error() {
    assert!(matches!(parse_cli(&[]), Err(ArgsError::Usage)));
}

#[test]
fn version_subcommand_parses_via_cli() {
    match parse_cli(&args(&["version"])) {
        Ok(ParsedCli::Run(ParsedCommand::Version)) => {}
        other => panic!("expected version, got {other:?}"),
    }
}

#[test]
fn help_subcommand_is_a_usage_error() {
    assert!(matches!(parse_cli(&args(&["help"])), Err(ArgsError::Usage)));
}

#[test]
fn too_many_positional_arguments_is_a_usage_error() {
    assert_eq!(parse_lint(&args(&["a", "b"])), Err(ArgsError::Usage));
}

#[test]
fn a_single_path_parses_to_a_non_fix_lint_run() {
    let parsed = parse_lint(&args(&["Example.psc"])).expect("should parse");
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
    let parsed = parse_fix(&args(&["Example.psc"])).expect("should parse");
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
    assert_eq!(parse_fix(&[]), Err(ArgsError::Usage));
}

#[test]
fn parse_rejects_an_unknown_type_filter_rule_id() {
    assert_eq!(
        parse_fix(&args(&["--type=made-up-rule", "Example.psc"])),
        Err(ArgsError::UnknownRule("made-up-rule".to_string()))
    );
}

#[test]
fn parse_rejects_a_type_filter_rule_with_no_automatic_fix() {
    assert_eq!(
        parse_fix(&args(&["--type=forbidden-functions", "Example.psc"])),
        Err(ArgsError::RuleHasNoFix("forbidden-functions".to_string()))
    );
}

#[test]
fn type_filter_accepts_the_hyphenated_or_underscored_form() {
    let parsed =
        parse_fix(&args(&["--type=trailing_whitespace", "Example.psc"])).expect("should parse");
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
        parse_lint(&args(&["--type=trailing-whitespace", "Example.psc"])),
        Err(ArgsError::Usage)
    );
}

#[test]
fn line_filter_without_fix_is_a_usage_error() {
    assert_eq!(
        parse_lint(&args(&["--line=1", "Example.psc"])),
        Err(ArgsError::Usage)
    );
}

#[test]
fn parse_rejects_a_non_positive_line_filter() {
    assert_eq!(
        parse_fix(&args(&["--line=0", "Example.psc"])),
        Err(ArgsError::InvalidLine("0".to_string()))
    );
}

#[test]
fn parse_rejects_dry_run_without_fix() {
    assert_eq!(
        parse_lint(&args(&["--dry-run", "Example.psc"])),
        Err(ArgsError::Usage)
    );
}

#[test]
fn parse_rejects_an_unknown_tag_filter() {
    assert_eq!(
        parse_lint(&args(&["--tag=made-up-tag", "Example.psc"])),
        Err(ArgsError::UnknownTag("made-up-tag".to_string()))
    );
}

#[test]
fn tag_filter_matches_case_insensitively() {
    let parsed = parse_lint(&args(&["--tag=STYLE", "Example.psc"])).expect("should parse");
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
        parse_fix(&args(&[
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
        parse_lint(&args(&["--progress", "Example.psc"])),
        Err(ArgsError::ProgressRequiresOutput)
    );
}

#[test]
fn progress_with_output_parses() {
    let parsed = parse_lint(&args(&[
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
        parse_lint(&args(&["--threads=0", "Example.psc"])),
        Err(ArgsError::InvalidThreads("0".to_string()))
    );
}

#[test]
fn parse_rejects_a_non_numeric_threads_flag() {
    assert_eq!(
        parse_lint(&args(&["--threads", "many", "Example.psc"])),
        Err(ArgsError::InvalidThreads("many".to_string()))
    );
}

#[test]
fn format_flag_rejects_an_unknown_value() {
    assert_eq!(
        parse_lint(&args(&["--format=yaml", "Example.psc"])),
        Err(ArgsError::InvalidFormat("yaml".to_string()))
    );
}

#[test]
fn hash_source_without_ai_format_is_an_error() {
    assert_eq!(
        parse_lint(&args(&["--hash-source", "Example.psc"])),
        Err(ArgsError::HashSourceRequiresAi)
    );
}

#[test]
fn color_flag_rejects_an_unknown_value() {
    assert_eq!(
        parse_lint(&args(&["--color", "rainbow", "Example.psc"])),
        Err(ArgsError::InvalidColor("rainbow".to_string()))
    );
}

#[test]
fn blob_cannot_be_combined_with_a_path_argument() {
    assert_eq!(
        parse_lint(&args(&["--blob", "ScriptName Example", "some/path.psc"])),
        Err(ArgsError::BlobWithPathArgument)
    );
}

#[test]
fn blob_cannot_be_combined_with_fix_type_line_or_dry_run() {
    for flag in ["--dry-run", "--type=trailing-whitespace", "--line=1"] {
        assert_eq!(
            parse_lint(&args(&[flag, "--blob", "ScriptName Example"])),
            Err(ArgsError::BlobWithFixFlags),
            "flag {flag} should have been rejected"
        );
    }
}

#[test]
fn blob_cannot_be_combined_with_script_root_progress_or_threads() {
    assert_eq!(
        parse_lint(&args(&[
            "--script-root",
            "other",
            "--blob",
            "ScriptName Example"
        ])),
        Err(ArgsError::BlobWithScriptRootProgressThreads)
    );
    assert_eq!(
        parse_lint(&args(&[
            "--progress",
            "--output",
            "out.txt",
            "--blob",
            "ScriptName Example"
        ])),
        Err(ArgsError::BlobWithScriptRootProgressThreads)
    );
    assert_eq!(
        parse_lint(&args(&["--threads=2", "--blob", "ScriptName Example"])),
        Err(ArgsError::BlobWithScriptRootProgressThreads)
    );
}

#[test]
fn blob_rejects_an_unknown_tag() {
    assert_eq!(
        parse_lint(&args(&[
            "--tag=made-up-tag",
            "--blob",
            "ScriptName Example"
        ])),
        Err(ArgsError::UnknownTag("made-up-tag".to_string()))
    );
}

#[test]
fn blob_parses_into_its_own_args() {
    let parsed = parse_lint(&args(&["--blob", "ScriptName Example"])).expect("should parse");
    match parsed {
        ParsedCommand::Blob(blob) => assert_eq!(blob.source, "ScriptName Example"),
        other => panic!("expected a blob run, got {other:?}"),
    }
}

#[test]
fn parse_reports_usage_when_separate_value_flags_are_missing() {
    for flag in ["--line", "--tag", "--format", "--threads"] {
        assert_eq!(
            parse_lint(&args(&[flag])),
            Err(ArgsError::Usage),
            "unexpected result for {flag}"
        );
    }
}

#[test]
fn config_flag_without_a_value_is_a_usage_error() {
    assert_eq!(parse_lint(&args(&["--config"])), Err(ArgsError::Usage));
}

#[test]
fn script_root_flag_without_a_value_is_a_usage_error() {
    assert_eq!(parse_lint(&args(&["--script-root"])), Err(ArgsError::Usage));
}

#[test]
fn threads_flag_without_a_value_is_a_usage_error() {
    assert_eq!(parse_lint(&args(&["--threads"])), Err(ArgsError::Usage));
}

// Below: integration-level checks (via `run`, i.e. `crate::run`) of the
// same usage errors covered by the pure unit tests above, moved
// unchanged from `lib.rs`'s own test module so `run`'s actual wiring of
// `parse_cli`'s outcomes (the exact text written to stderr/stdout,
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
    let (code, stdout, _stderr) = run_captured(&["version".to_string()]);

    assert_eq!(code, 0);
    assert_eq!(stdout, format!("PapyrusLinterCLI {}\n", crate::VERSION));
}

#[test]
fn prints_version_for_short_version_flag() {
    let (code, stdout, _stderr) = run_captured(&["version".to_string()]);

    assert_eq!(code, 0);
    assert_eq!(stdout, format!("PapyrusLinterCLI {}\n", crate::VERSION));
}

#[test]
fn prints_usage_for_help_flag() {
    let (code, _stdout, stderr) = run_captured(&["help".to_string()]);

    assert_eq!(code, 2);
    assert!(stderr.contains("Usage: PapyrusLinterCLI"));
    assert!(stderr.contains("Examples:"));
    assert!(stderr.contains("PapyrusLinterCLI lint --format ai path/to/project.achlist"));
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
