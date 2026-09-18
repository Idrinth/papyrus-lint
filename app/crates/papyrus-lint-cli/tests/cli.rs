//! End-to-end tests for the standalone `PapyrusLinterCLI` binary's shared
//! `run` path (lint / fix / flags) and its thin `main` entry point.
//!
//! Subcommand- and output-specific coverage lives next to those modules:
//! `blob.rs`, `doctor.rs`, `init.rs`, and `output.rs`. The unit tests at the
//! bottom of each `src/` file exercise the shared `run` function; these tests
//! additionally verify that the binary entry point forwards arguments, writes
//! to the expected process streams, and returns the documented status.

mod common;

use common::*;
use std::fs;
use std::path::Path;

#[test]
fn help_is_written_to_stderr_with_the_usage_error_status() {
    let output = run_cli(&["--help"]);

    assert_eq!(output.status.code(), Some(2));
    assert!(output.stdout.is_empty());
    assert!(String::from_utf8(output.stderr)
        .expect("stderr should be UTF-8")
        .starts_with("Usage: PapyrusLinterCLI"));
}

#[test]
fn no_arguments_prints_usage_through_the_binary_entry_point() {
    let output = run_cli(&[]);

    assert_eq!(output.status.code(), Some(2));
    assert!(output.stdout.is_empty());
    assert!(String::from_utf8(output.stderr)
        .expect("stderr should be UTF-8")
        .starts_with("Usage: PapyrusLinterCLI"));
}

#[test]
fn version_is_written_to_stdout() {
    let output = run_cli(&["--version"]);

    assert!(output.status.success());
    assert!(output.stderr.is_empty());
    assert_eq!(
        String::from_utf8(output.stdout).expect("stdout should be UTF-8"),
        format!("PapyrusLinterCLI {}\n", env!("CARGO_PKG_VERSION"))
    );
}

#[test]
fn short_version_flag_is_written_to_stdout() {
    let output = run_cli(&["-V"]);

    assert!(output.status.success());
    assert!(output.stderr.is_empty());
    assert_eq!(
        String::from_utf8(output.stdout).expect("stdout should be UTF-8"),
        format!("PapyrusLinterCLI {}\n", env!("CARGO_PKG_VERSION"))
    );
}

#[test]
fn extra_positional_argument_reports_usage_without_writing_to_stdout() {
    let output = run_cli(&["first.achlist", "second.achlist"]);

    assert_eq!(output.status.code(), Some(2));
    assert!(output.stdout.is_empty());
    let stderr = String::from_utf8(output.stderr).expect("stderr should be UTF-8");
    assert!(stderr.starts_with("Usage: PapyrusLinterCLI"));
}

#[test]
fn fix_mode_rewrites_a_script_through_the_binary_entry_point() {
    let dir = tempfile::tempdir().expect("failed to create temp directory");
    let script = dir.path().join("scripts/source/Example.psc");
    write_file(&script, "ScriptName Example   \n");

    let output = run_cli(&["fix", &script.to_string_lossy()]);

    assert!(output.status.success());
    assert!(output.stderr.is_empty());
    assert_eq!(
        fs::read_to_string(script).expect("fixed script should be readable"),
        "ScriptName Example\n"
    );
    assert!(String::from_utf8(output.stdout)
        .expect("stdout should be UTF-8")
        .contains("(1 script(s) fixed.)"));
}

#[test]
fn fix_dry_run_prints_a_diff_without_writing_through_the_binary_entry_point() {
    let dir = tempfile::tempdir().expect("failed to create temp directory");
    let script = dir.path().join("scripts/source/Example.psc");
    write_file(&script, "ScriptName Example   \n");

    let output = run_cli(&["fix", "--dry-run", &script.to_string_lossy()]);

    assert!(output.status.success());
    assert!(output.stderr.is_empty());
    assert_eq!(
        fs::read_to_string(&script).expect("script should be unchanged"),
        "ScriptName Example   \n"
    );
    let stdout = String::from_utf8(output.stdout).expect("stdout should be UTF-8");
    assert!(stdout.contains(&format!("--- {}\n", script.display())));
    assert!(stdout.contains("-ScriptName Example   \n"));
    assert!(stdout.contains("+ScriptName Example\n"));
    assert!(stdout.contains("(1 script(s) would be fixed.)"));
}

#[test]
fn missing_script_reports_an_io_error_on_stderr() {
    let dir = tempfile::tempdir().expect("failed to create temp directory");
    let missing_script = dir.path().join("scripts/source/Missing.psc");

    let output = run_cli(&[&missing_script.to_string_lossy()]);

    assert_eq!(output.status.code(), Some(2));
    assert!(output.stdout.is_empty());
    let stderr = String::from_utf8(output.stderr).expect("stderr should be UTF-8");
    assert!(stderr.starts_with("error: failed to read "));
    assert!(stderr.contains("Missing.psc"));
}

#[test]
fn lint_errors_produce_a_failure_status_through_the_binary_entry_point() {
    let dir = tempfile::tempdir().expect("failed to create temp directory");
    let script = dir.path().join("scripts/source/Example.psc");
    write_file(
        &script,
        "ScriptName Example\n\nFunction DoThing()\n    Game.GetPlayer()\nEndFunction\n",
    );

    let output = run_cli(&[&script.to_string_lossy()]);

    assert_eq!(output.status.code(), Some(1));
    assert!(output.stderr.is_empty());
    let stdout = String::from_utf8(output.stdout).expect("stdout should be UTF-8");
    assert!(stdout.contains("(forbidden-functions)"));
    assert!(stdout.contains("[error]"));
    assert!(stdout.contains("problem(s) found in 1 of 1 script(s)"));
}

#[test]
fn quiet_warnings_hides_output_without_changing_the_binary_exit_status() {
    let dir = tempfile::tempdir().expect("failed to create temp directory");
    let script = dir.path().join("scripts/source/Example.psc");
    write_file(&script, "ScriptName Example   \n");

    let output = run_cli(&["--quiet-warnings", &script.to_string_lossy()]);

    assert!(output.status.success());
    assert!(output.stderr.is_empty());
    let stdout = String::from_utf8(output.stdout).expect("stdout should be UTF-8");
    assert!(!stdout.contains("(trailing-whitespace)"));
    assert!(stdout.contains("no problems found in 1 script"));
}

#[test]
fn quiet_info_filters_json_without_changing_the_binary_exit_status() {
    let dir = tempfile::tempdir().expect("failed to create temp directory");
    let script = dir.path().join("scripts/source/Example.psc");
    write_file(
        &script,
        "ScriptName Example\n\nGlobalVariable Property Value Auto\n\nFunction Test()\n    Value.GetValueInt()\nEndFunction\n",
    );
    write_file(
        &dir.path().join("papyrus-lint.yaml"),
        "fail_on_info: true\n",
    );

    let output = run_cli(&["--json", "--quiet-info", &script.to_string_lossy()]);

    assert_eq!(output.status.code(), Some(1));
    assert!(output.stderr.is_empty());
    let report: serde_json::Value =
        serde_json::from_slice(&output.stdout).expect("stdout should contain JSON");
    assert_eq!(report["success"], false);
    assert!(report["files"][0]["diagnostics"]
        .as_array()
        .expect("diagnostics should be an array")
        .iter()
        .all(|diagnostic| diagnostic["level"] != "info"));
}

#[test]
fn short_paths_are_used_in_plain_text_through_the_binary_entry_point() {
    let dir = tempfile::tempdir().expect("failed to create temp directory");
    let script = dir.path().join("scripts/source/Example.psc");
    write_file(&script, "ScriptName Example   \n");

    let output = run_cli(&["--short-paths", &script.to_string_lossy()]);

    assert!(output.status.success());
    assert!(output.stderr.is_empty());
    let stdout = String::from_utf8(output.stdout).expect("stdout should be UTF-8");
    let relative_path = Path::new("scripts/source/Example.psc")
        .to_string_lossy()
        .into_owned();
    assert!(stdout.contains(&format!("{relative_path}:1:")));
    assert!(!stdout.contains(dir.path().to_string_lossy().as_ref()));
}

#[test]
fn tag_filter_is_forwarded_through_the_binary_entry_point() {
    let dir = tempfile::tempdir().expect("failed to create temp directory");
    let script = dir.path().join("scripts/source/Example.psc");
    write_file(
        &script,
        "ScriptName Example   \n\nFunction DoThing()\n    Game.GetPlayer()\nEndFunction\n",
    );

    let output = run_cli(&["--tag", "style", &script.to_string_lossy()]);

    assert!(output.status.success());
    assert!(output.stderr.is_empty());
    let stdout = String::from_utf8(output.stdout).expect("stdout should be UTF-8");
    assert!(stdout.contains("(trailing-whitespace)"));
    assert!(!stdout.contains("(forbidden-functions)"));
}

#[test]
fn typed_fix_only_repairs_the_selected_rule_through_the_binary() {
    let dir = tempfile::tempdir().expect("failed to create temp directory");
    let script = dir.path().join("scripts/source/Example.psc");
    write_file(
        &script,
        "ScriptName Example\n\nFunction Add(Int left,Int right)\nEndFunction   \n",
    );

    let output = run_cli(&["fix", "--type", "comma-spacing", &script.to_string_lossy()]);

    assert!(output.status.success());
    assert!(output.stderr.is_empty());
    assert_eq!(
        fs::read_to_string(script).expect("fixed script should be readable"),
        "ScriptName Example\n\nFunction Add(Int left, Int right)\nEndFunction   \n"
    );
    let stdout = String::from_utf8(output.stdout).expect("stdout should be UTF-8");
    assert!(stdout.contains("(trailing-whitespace)"));
    assert!(!stdout.contains("(comma-spacing)"));
}

#[test]
fn line_scoped_fix_only_rewrites_the_selected_line_through_the_binary() {
    let dir = tempfile::tempdir().expect("failed to create temp directory");
    let script = dir.path().join("scripts/source/Example.psc");
    write_file(
        &script,
        "ScriptName Example   \n\nFunction DoThing()   \nEndFunction\n",
    );

    let output = run_cli(&["fix", "--line=3", &script.to_string_lossy()]);

    assert!(output.status.success());
    assert!(output.stderr.is_empty());
    assert_eq!(
        fs::read_to_string(script).expect("fixed script should be readable"),
        "ScriptName Example   \n\nFunction DoThing()\nEndFunction\n"
    );
    let stdout = String::from_utf8(output.stdout).expect("stdout should be UTF-8");
    assert!(stdout.contains("(trailing-whitespace)"));
    assert!(stdout.contains("(1 script(s) fixed.)"));
}

#[test]
fn explicit_config_disables_a_rule_through_the_binary_entry_point() {
    let dir = tempfile::tempdir().expect("failed to create temp directory");
    let script = dir.path().join("scripts/source/Example.psc");
    let config = dir.path().join("config/override.yaml");
    write_file(&script, "ScriptName Example   \n");
    write_file(&config, "rules:\n  trailing_whitespace: false\n");

    let output = run_cli(&[
        "--config",
        &config.to_string_lossy(),
        &script.to_string_lossy(),
    ]);

    assert!(output.status.success());
    assert!(output.stderr.is_empty());
    assert_eq!(
        String::from_utf8(output.stdout).expect("stdout should be UTF-8"),
        "PapyrusLinterCLI: no problems found in 1 script(s).\n"
    );
}

#[test]
fn missing_output_directory_reports_an_io_error_through_the_binary() {
    let dir = tempfile::tempdir().expect("failed to create temp directory");
    let script = dir.path().join("scripts/source/Example.psc");
    let report = dir.path().join("missing/report.txt");
    write_file(&script, "ScriptName Example\n");

    let output = run_cli(&[
        "--output",
        &report.to_string_lossy(),
        &script.to_string_lossy(),
    ]);

    assert_eq!(output.status.code(), Some(2));
    assert!(output.stdout.is_empty());
    let stderr = String::from_utf8(output.stderr).expect("stderr should be UTF-8");
    assert!(stderr.starts_with("error: failed to write "));
    assert!(stderr.contains("report.txt"));
}

#[test]
fn achlist_json_report_includes_clean_and_dirty_scripts() {
    let dir = tempfile::tempdir().expect("failed to create temp directory");
    let clean_script = dir.path().join("scripts/source/Clean.psc");
    let dirty_script = dir.path().join("scripts/source/Dirty.psc");
    let achlist = dir.path().join("sources.achlist");
    write_file(&clean_script, "ScriptName Clean\n");
    write_file(&dirty_script, "ScriptName Dirty   \n");
    write_file(
        &achlist,
        r#"["scripts/source/Clean.psc", "scripts/source/Dirty.psc"]"#,
    );

    let output = run_cli(&["--json", "--short-paths", &achlist.to_string_lossy()]);

    assert!(output.status.success());
    assert!(output.stderr.is_empty());
    let report: serde_json::Value =
        serde_json::from_slice(&output.stdout).expect("stdout should contain JSON");
    assert_eq!(report["scripts_checked"], 2);
    assert_eq!(report["files_with_diagnostics"], 1);
    assert_eq!(report["total_diagnostics"], 1);
    let files = report["files"]
        .as_array()
        .expect("files should be an array");
    assert_eq!(files.len(), 2);
    assert_eq!(
        files[0]["path"],
        Path::new("scripts/source/Clean.psc")
            .to_string_lossy()
            .as_ref()
    );
    assert_eq!(files[0]["diagnostics"].as_array().unwrap().len(), 0);
    assert_eq!(
        files[1]["path"],
        Path::new("scripts/source/Dirty.psc")
            .to_string_lossy()
            .as_ref()
    );
    assert_eq!(files[1]["diagnostics"][0]["rule"], "trailing-whitespace");
}

#[test]
fn invalid_config_is_reported_by_the_binary_without_a_lint_report() {
    let dir = tempfile::tempdir().expect("failed to create temp directory");
    let script = dir.path().join("scripts/source/Example.psc");
    write_file(&script, "ScriptName Example\n");
    write_file(
        &dir.path().join("papyrus-lint.yaml"),
        "rules: not-a-rule-map\n",
    );

    let output = run_cli(&["--json", &script.to_string_lossy()]);

    assert_eq!(output.status.code(), Some(2));
    assert!(output.stdout.is_empty());
    let stderr = String::from_utf8(output.stderr).expect("stderr should be UTF-8");
    assert!(stderr.starts_with("error: failed to load lint config:"));
    assert!(stderr.contains("expected struct Rules"));
}

#[test]
fn malformed_achlist_is_reported_by_the_binary_without_a_lint_report() {
    let dir = tempfile::tempdir().expect("failed to create temp directory");
    let achlist = dir.path().join("sources.achlist");
    write_file(&achlist, r#"{"not": "an array"}"#);

    let output = run_cli(&[&achlist.to_string_lossy()]);

    assert_eq!(output.status.code(), Some(2));
    assert!(output.stdout.is_empty());
    let stderr = String::from_utf8(output.stderr).expect("stderr should be UTF-8");
    assert!(stderr.starts_with("error: failed to parse achlist file:"));
    assert!(stderr.contains("expected a sequence"));
}

#[test]
fn directory_input_recursively_lints_only_papyrus_scripts() {
    let dir = tempfile::tempdir().expect("failed to create temp directory");
    let scripts = dir.path().join("scripts/source");
    write_file(&scripts.join("Clean.psc"), "ScriptName Clean\n");
    write_file(&scripts.join("nested/Dirty.PSC"), "ScriptName Dirty   \n");
    write_file(
        &scripts.join("nested/NotPapyrus.txt"),
        "ScriptName NotPapyrus   \n",
    );

    let output = run_cli(&["--json", &scripts.to_string_lossy()]);

    assert!(output.status.success());
    assert!(output.stderr.is_empty());
    let report: serde_json::Value =
        serde_json::from_slice(&output.stdout).expect("stdout should contain JSON");
    assert_eq!(report["scripts_checked"], 2);
    assert_eq!(report["files_with_diagnostics"], 1);
    assert_eq!(report["total_diagnostics"], 1);
    let files = report["files"]
        .as_array()
        .expect("files should be an array");
    assert!(files.iter().any(|file| file["path"]
        .as_str()
        .is_some_and(|path| path.ends_with("Clean.psc"))));
    assert!(files.iter().any(|file| file["path"]
        .as_str()
        .is_some_and(|path| path.ends_with("Dirty.PSC"))));
    assert!(!files.iter().any(|file| file["path"]
        .as_str()
        .is_some_and(|path| path.ends_with("NotPapyrus.txt"))));
}

#[test]
fn fix_directory_recursively_repairs_each_script() {
    let dir = tempfile::tempdir().expect("failed to create temp directory");
    let scripts = dir.path().join("scripts/source");
    let first = scripts.join("First.psc");
    let second = scripts.join("nested/Second.psc");
    write_file(&first, "ScriptName First   \n");
    write_file(&second, "ScriptName Second\t\n");

    let output = run_cli(&["fix", &scripts.to_string_lossy()]);

    assert!(output.status.success());
    assert!(output.stderr.is_empty());
    assert_eq!(
        fs::read_to_string(first).expect("first script should be readable"),
        "ScriptName First\n"
    );
    assert_eq!(
        fs::read_to_string(second).expect("second script should be readable"),
        "ScriptName Second\n"
    );
    assert!(String::from_utf8(output.stdout)
        .expect("stdout should be UTF-8")
        .contains("(2 script(s) fixed.)"));
}

#[test]
fn fix_rejects_combining_rule_and_tag_filters_without_modifying_the_script() {
    let dir = tempfile::tempdir().expect("failed to create temp directory");
    let script = dir.path().join("Example.psc");
    let original = "ScriptName Example   \n";
    write_file(&script, original);

    let output = run_cli(&[
        "fix",
        "--type=trailing-whitespace",
        "--tag=style",
        &script.to_string_lossy(),
    ]);

    assert_eq!(output.status.code(), Some(2));
    assert!(output.stdout.is_empty());
    assert_eq!(
        String::from_utf8(output.stderr).expect("stderr should be UTF-8"),
        "error: --type and --tag can't be combined\n"
    );
    assert_eq!(
        fs::read_to_string(script).expect("failed to read script"),
        original
    );
}

#[test]
fn fix_rejects_zero_as_a_line_number_without_modifying_the_script() {
    let dir = tempfile::tempdir().expect("failed to create temp directory");
    let script = dir.path().join("Example.psc");
    let original = "ScriptName Example   \n";
    write_file(&script, original);

    let output = run_cli(&["fix", "--line=0", &script.to_string_lossy()]);

    assert_eq!(output.status.code(), Some(2));
    assert!(output.stdout.is_empty());
    assert_eq!(
        String::from_utf8(output.stderr).expect("stderr should be UTF-8"),
        "error: --line must be a positive integer, got '0'\n"
    );
    assert_eq!(
        fs::read_to_string(script).expect("failed to read script"),
        original
    );
}

#[test]
fn threaded_directory_lint_keeps_json_files_in_stable_path_order() {
    let dir = tempfile::tempdir().expect("failed to create temp directory");
    let scripts = dir.path().join("scripts/source");
    // Deliberately create these in a different order from the one expected in
    // the report. Worker completion and filesystem iteration order must not
    // leak into machine-readable output.
    for name in ["Zulu", "Alpha", "Middle"] {
        write_file(
            &scripts.join(format!("{name}.psc")),
            &format!("ScriptName {name}   \n"),
        );
    }

    let output = run_cli(&[
        "--threads",
        "3",
        "--short-paths",
        "--json",
        &scripts.to_string_lossy(),
    ]);

    assert!(output.status.success());
    assert!(output.stderr.is_empty());
    let report: serde_json::Value =
        serde_json::from_slice(&output.stdout).expect("stdout should contain JSON");
    let paths: Vec<_> = report["files"]
        .as_array()
        .expect("files should be an array")
        .iter()
        .map(|file| file["path"].as_str().expect("path should be a string"))
        .collect();
    assert_eq!(
        paths,
        [
            "scripts/source/Alpha.psc",
            "scripts/source/Middle.psc",
            "scripts/source/Zulu.psc"
        ]
    );
}

#[test]
fn threaded_fix_repairs_every_script_through_the_binary_entry_point() {
    let dir = tempfile::tempdir().expect("failed to create temp directory");
    let scripts = dir.path().join("scripts/source");
    let script_paths: Vec<_> = (0..8)
        .map(|index| scripts.join(format!("Example{index}.psc")))
        .collect();
    for (index, script) in script_paths.iter().enumerate() {
        write_file(script, &format!("ScriptName Example{index}   \n"));
    }

    let output = run_cli(&[
        "fix",
        "--threads=4",
        "--type=trailing-whitespace",
        &scripts.to_string_lossy(),
    ]);

    assert!(output.status.success());
    assert!(output.stderr.is_empty());
    assert_eq!(
        String::from_utf8(output.stdout).expect("stdout should be UTF-8"),
        "PapyrusLinterCLI: no problems found in 8 script(s). (8 script(s) fixed.)\n"
    );
    for (index, script) in script_paths.iter().enumerate() {
        assert_eq!(
            fs::read_to_string(script).expect("fixed script should be readable"),
            format!("ScriptName Example{index}\n")
        );
    }
}

#[test]
fn invalid_thread_count_is_rejected_before_fixing_a_script() {
    let dir = tempfile::tempdir().expect("failed to create temp directory");
    let script = dir.path().join("Example.psc");
    let original = "ScriptName Example   \n";
    write_file(&script, original);

    let output = run_cli(&["fix", "--threads", "many", &script.to_string_lossy()]);

    assert_eq!(output.status.code(), Some(2));
    assert!(output.stdout.is_empty());
    assert_eq!(
        String::from_utf8(output.stderr).expect("stderr should be UTF-8"),
        "error: --threads must be a positive integer, got 'many'\n"
    );
    assert_eq!(
        fs::read_to_string(script).expect("script should be readable"),
        original
    );
}
