//! Integration-style tests for the top-level `run()` pipeline (plain lint,
//! `fix`, `--json`/`--format ai`, `--tag`/`--type`, threading, color, and
//! output-file behavior). Kept separate from the command-shaped
//! `run_scan`/`run_lint`/`run_fix` modules since these tests exercise `run()`
//! end to end rather than any one of them in isolation.

use crate::test_support::*;
use std::fs;

#[test]
fn errors_when_achlist_is_missing() {
    let dir = tempfile::tempdir().expect("failed to create temp dir");
    let achlist_path = dir.path().join("missing.achlist");

    let (code, _stdout, stderr) = run_captured(&[achlist_path.to_string_lossy().into_owned()]);

    assert_eq!(code, 2);
    assert!(stderr.starts_with("error:"));
}

#[test]
fn reports_no_problems_for_a_clean_project() {
    let dir = tempfile::tempdir().expect("failed to create temp dir");
    write_file(
        &dir.path().join("scripts/source/Example.psc"),
        "ScriptName Example\n",
    );
    write_file(
        &dir.path().join("sources.achlist"),
        r#"["scripts/source/Example.psc"]"#,
    );
    let achlist_path = dir.path().join("sources.achlist");

    let (code, stdout, _stderr) = run_captured(&[achlist_path.to_string_lossy().into_owned()]);

    assert_eq!(code, 0);
    assert!(stdout.contains("no problems found in 1 script"));
}

#[test]
fn reports_diagnostics_and_exits_1_for_a_dirty_project() {
    let dir = tempfile::tempdir().expect("failed to create temp dir");
    write_file(
        &dir.path().join("scripts/source/Example.psc"),
        "ScriptName Example\n\nFunction DoThing()\n\tGame.GetPlayer()\nEndFunction\n",
    );
    write_file(
        &dir.path().join("sources.achlist"),
        r#"["scripts/source/Example.psc"]"#,
    );
    let achlist_path = dir.path().join("sources.achlist");

    let (code, stdout, _stderr) = run_captured(&[achlist_path.to_string_lossy().into_owned()]);

    assert_eq!(code, 1);
    assert!(stdout.contains("(forbidden-functions)"));
    assert!(stdout.contains("problem(s) found in 1 of 1 script(s)"));
}

#[test]
fn does_not_fail_on_warning_level_diagnostics_by_default() {
    let dir = tempfile::tempdir().expect("failed to create temp dir");
    write_file(
        &dir.path().join("scripts/source/Example.psc"),
        "ScriptName Example\n\nInt Property MyValue = 1 Auto\n\nFunction DoThing()\nEndFunction\n",
    );
    write_file(
        &dir.path().join("sources.achlist"),
        r#"["scripts/source/Example.psc"]"#,
    );
    let achlist_path = dir.path().join("sources.achlist");

    let (code, stdout, _stderr) = run_captured(&[achlist_path.to_string_lossy().into_owned()]);

    assert_eq!(code, 0);
    assert!(stdout.contains("(unused-property)"));
    assert!(stdout.contains("1 problem(s) found in 1 of 1 script(s)"));
}

#[test]
fn fails_on_warning_level_diagnostics_when_configured() {
    let dir = tempfile::tempdir().expect("failed to create temp dir");
    write_file(
        &dir.path().join("scripts/source/Example.psc"),
        "ScriptName Example\n\nInt Property MyValue = 1 Auto\n\nFunction DoThing()\nEndFunction\n",
    );
    write_file(
        &dir.path().join("sources.achlist"),
        r#"["scripts/source/Example.psc"]"#,
    );
    write_file(
        &dir.path().join("papyrus-lint.yaml"),
        "fail_on_warning: true\n",
    );
    let achlist_path = dir.path().join("sources.achlist");

    let (code, stdout, _stderr) = run_captured(&[achlist_path.to_string_lossy().into_owned()]);

    assert_eq!(code, 1);
    assert!(stdout.contains("(unused-property)"));
}

#[test]
fn quiet_warnings_hides_warnings_without_changing_the_exit_code() {
    let dir = tempfile::tempdir().expect("failed to create temp dir");
    let script_path = dir.path().join("scripts/source/Example.psc");
    write_file(&script_path, "ScriptName Example   \n");
    write_file(
        &dir.path().join("papyrus-lint.yaml"),
        "fail_on_warning: true\n",
    );

    let (code, stdout, stderr) = run_captured(&[
        "--quiet-warnings".to_string(),
        script_path.to_string_lossy().into_owned(),
    ]);

    assert_eq!(code, 1);
    assert!(stderr.is_empty());
    assert!(!stdout.contains("[warning]"));
    assert!(stdout.contains("no problems found in 1 script"));
}

#[test]
fn quiet_info_hides_info_diagnostics_from_json_without_changing_the_exit_code() {
    let dir = tempfile::tempdir().expect("failed to create temp dir");
    let script_path = dir.path().join("scripts/source/Example.psc");
    write_file(
        &script_path,
        "ScriptName Example\n\nGlobalVariable Property Value Auto\n\nFunction Test()\n    Value.GetValueInt()\nEndFunction\n",
    );
    write_file(
        &dir.path().join("papyrus-lint.yaml"),
        "fail_on_info: true\n",
    );

    let (unfiltered_code, unfiltered_stdout, _) = run_captured(&[
        "--json".to_string(),
        script_path.to_string_lossy().into_owned(),
    ]);
    let unfiltered: serde_json::Value = serde_json::from_str(&unfiltered_stdout).unwrap();
    assert_eq!(unfiltered_code, 1);
    assert!(unfiltered["files"][0]["diagnostics"]
        .as_array()
        .unwrap()
        .iter()
        .any(|diagnostic| diagnostic["level"] == "info"));

    let (code, stdout, stderr) = run_captured(&[
        "--json".to_string(),
        "--quiet-info".to_string(),
        script_path.to_string_lossy().into_owned(),
    ]);

    assert_eq!(code, 1);
    assert!(stderr.is_empty());
    let report: serde_json::Value = serde_json::from_str(&stdout).unwrap();
    assert_eq!(report["success"], false);
    let diagnostics = report["files"][0]["diagnostics"].as_array().unwrap();
    assert_eq!(report["total_diagnostics"], diagnostics.len());
    assert!(diagnostics
        .iter()
        .all(|diagnostic| diagnostic["level"] != "info"));
}

#[test]
fn honors_the_project_yaml_config() {
    let dir = tempfile::tempdir().expect("failed to create temp dir");
    write_file(
        &dir.path().join("scripts/source/Example.psc"),
        "ScriptName Example   \n",
    );
    write_file(
        &dir.path().join("sources.achlist"),
        r#"["scripts/source/Example.psc"]"#,
    );
    write_file(
        &dir.path().join("papyrus-lint.yaml"),
        "rules:\n  trailing_whitespace: false\n",
    );
    let achlist_path = dir.path().join("sources.achlist");

    let (code, stdout, _stderr) = run_captured(&[achlist_path.to_string_lossy().into_owned()]);

    assert_eq!(code, 0);
    assert!(stdout.contains("no problems found"));
}

#[test]
fn reports_an_invalid_project_yaml_config() {
    let dir = tempfile::tempdir().expect("failed to create temp dir");
    write_file(
        &dir.path().join("scripts/source/Example.psc"),
        "ScriptName Example\n",
    );
    write_file(
        &dir.path().join("sources.achlist"),
        r#"["scripts/source/Example.psc"]"#,
    );
    write_file(
        &dir.path().join("papyrus-lint.yaml"),
        "rules: definitely-not-a-rule-set\n",
    );
    let achlist_path = dir.path().join("sources.achlist");

    let (code, stdout, stderr) = run_captured(&[achlist_path.to_string_lossy().into_owned()]);

    assert_eq!(code, 2);
    assert!(stdout.is_empty());
    assert!(stderr.starts_with("error: failed to load lint config:"));
}

#[test]
fn ignores_non_psc_entries_in_the_achlist() {
    let dir = tempfile::tempdir().expect("failed to create temp dir");
    write_file(&dir.path().join("scripts/source/Example.pex"), "");
    write_file(
        &dir.path().join("sources.achlist"),
        r#"["scripts/source/Example.pex"]"#,
    );
    let achlist_path = dir.path().join("sources.achlist");

    let (code, stdout, _stderr) = run_captured(&[achlist_path.to_string_lossy().into_owned()]);

    assert_eq!(code, 0);
    assert!(stdout.contains("no problems found in 0 script"));
}

#[test]
fn recognizes_uppercase_psc_extensions_in_the_achlist() {
    let dir = tempfile::tempdir().expect("failed to create temp dir");
    write_file(
        &dir.path().join("scripts/source/Example.PSC"),
        "ScriptName Example   \n",
    );
    write_file(
        &dir.path().join("sources.achlist"),
        r#"["scripts/source/Example.PSC"]"#,
    );
    let achlist_path = dir.path().join("sources.achlist");

    let (code, stdout, stderr) = run_captured(&[achlist_path.to_string_lossy().into_owned()]);

    assert_eq!(code, 0);
    assert!(stderr.is_empty());
    assert!(stdout.contains("(trailing-whitespace)"));
    assert!(stdout.contains("1 problem(s) found in 1 of 1 script(s)"));
}

#[test]
fn lints_a_single_psc_file_passed_directly() {
    let dir = tempfile::tempdir().expect("failed to create temp dir");
    let script_path = dir.path().join("Example.psc");
    write_file(&script_path, "ScriptName Example   \n");

    let (code, stdout, _stderr) = run_captured(&[script_path.to_string_lossy().into_owned()]);

    assert_eq!(code, 0);
    assert!(stdout.contains("(trailing-whitespace)"));
    assert!(stdout.contains("1 problem(s) found in 1 of 1 script(s)"));
}

#[test]
fn reports_no_problems_for_a_clean_single_psc_file() {
    let dir = tempfile::tempdir().expect("failed to create temp dir");
    let script_path = dir.path().join("Example.psc");
    write_file(&script_path, "ScriptName Example\n");

    let (code, stdout, _stderr) = run_captured(&[script_path.to_string_lossy().into_owned()]);

    assert_eq!(code, 0);
    assert!(stdout.contains("no problems found in 1 script"));
}

#[test]
fn lints_every_psc_found_recursively_under_a_dropped_directory() {
    // Mirrors a mod like Requiem, whose scripts are spread across
    // arbitrarily nested subfolders rather than a flat scripts/source
    // directory, and ships no .achlist at all.
    let dir = tempfile::tempdir().expect("failed to create temp dir");
    write_file(
        &dir.path().join("scripts/source/Top.psc"),
        "ScriptName Top   \n",
    );
    write_file(
        &dir.path().join("scripts/source/Requiem/Sub/Nested.psc"),
        "ScriptName Nested   \n",
    );
    let target = dir.path().join("scripts/source");

    let (code, stdout, stderr) = run_captured(&[target.to_string_lossy().into_owned()]);

    assert_eq!(code, 0, "stderr: {stderr}");
    assert!(stdout.contains("2 script(s)"));
    assert!(stdout.contains("Top.psc"));
    assert!(stdout.contains("Nested.psc"));
}

#[test]
fn directory_scan_reports_no_problems_for_an_empty_directory() {
    let dir = tempfile::tempdir().expect("failed to create temp dir");

    let (code, stdout, _stderr) = run_captured(&[dir.path().to_string_lossy().into_owned()]);

    assert_eq!(code, 0);
    assert!(stdout.contains("no problems found in 0 script"));
}

#[test]
fn directory_scan_resolves_cross_script_calls_across_nested_subfolders() {
    let dir = tempfile::tempdir().expect("failed to create temp dir");
    write_file(
        &dir.path().join("scripts/source/Greeter.psc"),
        "ScriptName Greeter\n\nFunction Greet(String name)\nEndFunction\n",
    );
    write_file(
        &dir.path().join("scripts/source/Requiem/Example.psc"),
        "ScriptName Example\n\nGreeter Property Target Auto\n\nFunction Test()\n    Target.Greet(1)\nEndFunction\n",
    );
    let target = dir.path().join("scripts/source");

    let (code, stdout, _stderr) = run_captured(&[target.to_string_lossy().into_owned()]);

    assert_eq!(code, 1);
    assert!(stdout.contains("(argument-types)"));
}

#[test]
fn same_script_lints_identically_via_achlist_and_directly_when_namespaced() {
    // The same nested script should produce the same diagnostics
    // whether it's resolved from an .achlist or linted directly.
    let dir = tempfile::tempdir().expect("failed to create temp dir");
    write_file(
        &dir.path().join("scripts/source/Greeter.psc"),
        "ScriptName Greeter\n\nFunction Greet(String name)\nEndFunction\n",
    );
    let script_path = dir.path().join("scripts/source/User/Example.psc");
    write_file(
        &script_path,
        "ScriptName Example\n\nGreeter Property Target Auto\n\nFunction Test()\n    Target.Greet(1)\nEndFunction\n",
    );
    write_file(
        &dir.path().join("sources.achlist"),
        r#"["scripts/source/Greeter.psc", "scripts/source/User/Example.psc"]"#,
    );
    let achlist_path = dir.path().join("sources.achlist");

    let (achlist_code, achlist_stdout, _) =
        run_captured(&[achlist_path.to_string_lossy().into_owned()]);
    let (direct_code, direct_stdout, _) =
        run_captured(&[script_path.to_string_lossy().into_owned()]);

    assert_eq!(achlist_code, 1);
    assert_eq!(direct_code, 1);
    assert!(achlist_stdout.contains("(argument-types)"));
    assert!(direct_stdout.contains("(argument-types)"));
}

#[test]
fn decodes_a_cp1252_encoded_script_instead_of_failing_the_whole_run() {
    let dir = tempfile::tempdir().expect("failed to create temp dir");
    let script_path = dir.path().join("scripts/source/Example.psc");
    fs::create_dir_all(script_path.parent().unwrap()).expect("failed to create parent dir");
    // "; caf\xE9" in Windows-1252 (0xE9 is "é"), which is not valid
    // UTF-8 on its own.
    let mut contents = b"ScriptName Example\n\n; caf".to_vec();
    contents.push(0xE9);
    contents.push(b'\n');
    fs::write(&script_path, &contents).expect("failed to write test file");
    write_file(
        &dir.path().join("sources.achlist"),
        r#"["scripts/source/Example.psc"]"#,
    );
    let achlist_path = dir.path().join("sources.achlist");

    let (code, stdout, stderr) = run_captured(&[achlist_path.to_string_lossy().into_owned()]);

    assert_eq!(code, 0, "stderr: {stderr}");
    assert!(stdout.contains("no problems found in 1 script"));
}

#[test]
fn errors_when_the_given_psc_file_is_missing() {
    let dir = tempfile::tempdir().expect("failed to create temp dir");
    let script_path = dir.path().join("Missing.psc");

    let (code, _stdout, stderr) = run_captured(&[script_path.to_string_lossy().into_owned()]);

    assert_eq!(code, 2);
    assert!(stderr.starts_with("error:"));
}

#[test]
fn fix_rewrites_fixable_issues_and_reports_the_rest() {
    let dir = tempfile::tempdir().expect("failed to create temp dir");
    write_file(
        &dir.path().join("scripts/source/Example.psc"),
        "ScriptName Example   \n\nFunction DoThing()\n\tGame.GetPlayer()\nEndFunction\n",
    );
    write_file(
        &dir.path().join("sources.achlist"),
        r#"["scripts/source/Example.psc"]"#,
    );
    let achlist_path = dir.path().join("sources.achlist");

    let (code, stdout, _stderr) = run_captured(&[
        "fix".to_string(),
        achlist_path.to_string_lossy().into_owned(),
    ]);

    assert_eq!(
        fs::read_to_string(dir.path().join("scripts/source/Example.psc")).unwrap(),
        "ScriptName Example\n\nFunction DoThing()\n\tGame.GetPlayer()\nEndFunction\n"
    );
    assert_eq!(code, 1);
    assert!(!stdout.contains("(trailing-whitespace)"));
    assert!(stdout.contains("Game.GetPlayer"));
    assert!(stdout.contains("(1 script(s) fixed.)"));
}

#[test]
fn fix_preserves_a_cp1252_encoded_files_encoding() {
    let dir = tempfile::tempdir().expect("failed to create temp dir");
    let script_path = dir.path().join("scripts/source/Example.psc");
    fs::create_dir_all(script_path.parent().unwrap()).expect("failed to create parent dir");
    // "ScriptName Example   \n\n; caf\xE9\n" with trailing whitespace on
    // the first line to fix, and 0xE9 ("é" in Windows-1252) making the
    // file as a whole invalid UTF-8.
    let mut contents = b"ScriptName Example   \n\n; caf".to_vec();
    contents.push(0xE9);
    contents.push(b'\n');
    fs::write(&script_path, &contents).expect("failed to write test file");
    write_file(
        &dir.path().join("sources.achlist"),
        r#"["scripts/source/Example.psc"]"#,
    );
    let achlist_path = dir.path().join("sources.achlist");

    let (code, stdout, stderr) = run_captured(&[
        "fix".to_string(),
        achlist_path.to_string_lossy().into_owned(),
    ]);

    assert!(stdout.contains("(1 script(s) fixed.)"), "stderr: {stderr}");
    let mut expected = b"ScriptName Example\n\n; caf".to_vec();
    expected.push(0xE9);
    expected.push(b'\n');
    assert_eq!(
        fs::read(&script_path).expect("failed to read back fixed file"),
        expected,
        "fixing must preserve the file's original Windows-1252 encoding"
    );
    assert_eq!(code, 0);
}

#[test]
fn fix_type_filter_applies_only_the_named_rule() {
    let dir = tempfile::tempdir().expect("failed to create temp dir");
    let script_path = dir.path().join("Example.psc");
    write_file(
        &script_path,
        "ScriptName Example\n\nFunction Add(Int left,Int right)\nEndFunction   \n",
    );

    let (code, _stdout, stderr) = run_captured(&[
        "fix".to_string(),
        "--type=comma_spacing".to_string(),
        script_path.to_string_lossy().into_owned(),
    ]);

    assert!(stderr.is_empty());
    assert_eq!(code, 0);
    assert_eq!(
        fs::read_to_string(&script_path).unwrap(),
        "ScriptName Example\n\nFunction Add(Int left, Int right)\nEndFunction   \n"
    );
}

#[test]
fn fix_type_filter_accepts_the_hyphenated_rule_id() {
    let dir = tempfile::tempdir().expect("failed to create temp dir");
    let script_path = dir.path().join("Example.psc");
    write_file(&script_path, "ScriptName Example   \n");

    let (code, _stdout, stderr) = run_captured(&[
        "fix".to_string(),
        "--type".to_string(),
        "trailing-whitespace".to_string(),
        script_path.to_string_lossy().into_owned(),
    ]);

    assert!(stderr.is_empty());
    assert_eq!(code, 0);
    assert_eq!(
        fs::read_to_string(&script_path).unwrap(),
        "ScriptName Example\n"
    );
}

#[test]
fn fix_line_filter_only_touches_the_named_line() {
    let dir = tempfile::tempdir().expect("failed to create temp dir");
    let script_path = dir.path().join("Example.psc");
    write_file(
        &script_path,
        "ScriptName Example   \n\nFunction DoThing()   \nEndFunction\n",
    );

    let (code, _stdout, stderr) = run_captured(&[
        "fix".to_string(),
        "--line=3".to_string(),
        script_path.to_string_lossy().into_owned(),
    ]);

    assert!(stderr.is_empty());
    assert_eq!(code, 0);
    assert_eq!(
        fs::read_to_string(&script_path).unwrap(),
        "ScriptName Example   \n\nFunction DoThing()\nEndFunction\n"
    );
}

#[test]
fn fix_line_and_type_filters_combine() {
    let dir = tempfile::tempdir().expect("failed to create temp dir");
    let script_path = dir.path().join("Example.psc");
    write_file(
        &script_path,
        "ScriptName Example   \n\nFunction Add(Int left,Int right)   \nEndFunction\n",
    );

    let (code, _stdout, stderr) = run_captured(&[
        "fix".to_string(),
        "--line=3".to_string(),
        "--type=comma_spacing".to_string(),
        script_path.to_string_lossy().into_owned(),
    ]);

    assert!(stderr.is_empty());
    assert_eq!(code, 0);
    assert_eq!(
        fs::read_to_string(&script_path).unwrap(),
        "ScriptName Example   \n\nFunction Add(Int left, Int right)   \nEndFunction\n"
    );
}

#[test]
fn tag_filter_restricts_reported_diagnostics_to_the_matching_kind() {
    let dir = tempfile::tempdir().expect("failed to create temp dir");
    let script_path = dir.path().join("Example.psc");
    write_file(
        &script_path,
        "ScriptName Example   \n\nFunction DoThing()\n\tGame.GetPlayer()\nEndFunction\n",
    );

    let (code, stdout, stderr) = run_captured(&[
        "--tag=style".to_string(),
        script_path.to_string_lossy().into_owned(),
    ]);

    assert!(stderr.is_empty());
    assert_eq!(code, 0);
    assert!(stdout.contains("(trailing-whitespace)"));
    assert!(!stdout.contains("(forbidden-functions)"));
}

#[test]
fn tag_filter_matches_case_insensitively() {
    let dir = tempfile::tempdir().expect("failed to create temp dir");
    let script_path = dir.path().join("Example.psc");
    write_file(&script_path, "ScriptName Example   \n");

    let (code, stdout, stderr) = run_captured(&[
        "--tag=STYLE".to_string(),
        script_path.to_string_lossy().into_owned(),
    ]);

    assert!(stderr.is_empty());
    assert_eq!(code, 0);
    assert!(stdout.contains("(trailing-whitespace)"));
}

#[test]
fn tag_filter_works_without_fix() {
    let dir = tempfile::tempdir().expect("failed to create temp dir");
    let script_path = dir.path().join("Example.psc");
    write_file(
        &script_path,
        "ScriptName Example\n\nFunction DoThing()\n\tGame.GetPlayer()\nEndFunction\n",
    );

    let (code, _stdout, stderr) = run_captured(&[
        "--tag=performance".to_string(),
        script_path.to_string_lossy().into_owned(),
    ]);

    assert!(stderr.is_empty());
    assert_eq!(code, 1);
}

#[test]
fn fix_tag_filter_applies_only_fixes_in_that_kind() {
    let dir = tempfile::tempdir().expect("failed to create temp dir");
    let script_path = dir.path().join("Example.psc");
    write_file(
        &script_path,
        "ScriptName Example\n\nFunction DoThing(GlobalVariable akGlobal)\n    akGlobal.GetValueInt()  \nEndFunction\n",
    );

    let (code, _stdout, stderr) = run_captured(&[
        "fix".to_string(),
        "--tag=performance".to_string(),
        script_path.to_string_lossy().into_owned(),
    ]);

    assert!(stderr.is_empty());
    assert_eq!(code, 0);
    let fixed = fs::read_to_string(&script_path).unwrap();
    assert!(!fixed.contains("GetValueInt"));
    // trailing-whitespace is tagged "style", not "performance", so it's
    // left in place by --tag=performance.
    assert!(fixed.contains("  \n"));
}

// Builds an achlist with enough scripts, several of them cross-referencing
// a shared base script, that a multi-threaded run actually exercises
// more than one worker thread and more than one `SharedFunctionTable`
// lookup collision -- then checks a `--threads 1` (fully sequential) run
// and the default multi-threaded run agree byte-for-byte, since threading
// is only ever meant to change how fast a run finishes, never what it
// reports or in what order.
#[test]
fn threaded_and_sequential_runs_report_identical_results() {
    let dir = tempfile::tempdir().expect("failed to create temp dir");
    write_file(
        &dir.path().join("scripts/source/Base.psc"),
        "ScriptName Base\n\nFunction DoThing(int arg1)\nEndFunction\n",
    );
    let mut entries = vec!["\"scripts/source/Base.psc\"".to_string()];
    for i in 0..12 {
        let name = format!("Child{i}");
        write_file(
            &dir.path().join(format!("scripts/source/{name}.psc")),
            &format!(
                "ScriptName {name} Extends Base\n\nFunction UseIt()\n    DoThing(\"wrong type\")   \nEndFunction\n"
            ),
        );
        entries.push(format!("\"scripts/source/{name}.psc\""));
    }
    write_file(
        &dir.path().join("sources.achlist"),
        &format!("[{}]", entries.join(", ")),
    );
    let achlist_path = dir.path().join("sources.achlist");

    let (sequential_code, sequential_stdout, _) = run_captured(&[
        "--threads=1".to_string(),
        "--short-paths".to_string(),
        "--json".to_string(),
        achlist_path.to_string_lossy().into_owned(),
    ]);
    let (parallel_code, parallel_stdout, _) = run_captured(&[
        "--threads=8".to_string(),
        "--short-paths".to_string(),
        "--json".to_string(),
        achlist_path.to_string_lossy().into_owned(),
    ]);

    assert_eq!(sequential_code, parallel_code);
    assert_eq!(sequential_stdout, parallel_stdout);
    // Sanity check that this fixture actually triggers diagnostics
    // (the argument-type mismatch on every child script), rather than
    // both runs trivially agreeing on an empty report.
    assert!(sequential_stdout.contains("argument-types"));
}

#[test]
fn line_filter_errors_when_a_fix_changes_the_line_count() {
    let dir = tempfile::tempdir().expect("failed to create temp dir");
    let script_path = dir.path().join("scripts/source/Example.psc");
    write_file(
        &script_path,
        "ScriptName Example\n\nInt Property Zulu = 1 Auto\nActor Property Alpha Auto\n",
    );
    write_file(
        &dir.path().join("papyrus-lint.yaml"),
        "rules:\n  property_sorting: true\n",
    );

    let (code, _stdout, stderr) = run_captured(&[
        "fix".to_string(),
        "--line=3".to_string(),
        script_path.to_string_lossy().into_owned(),
    ]);

    assert_eq!(code, 2);
    assert!(stderr.contains("changes the file's line count"));
    assert_eq!(
        fs::read_to_string(&script_path).unwrap(),
        "ScriptName Example\n\nInt Property Zulu = 1 Auto\nActor Property Alpha Auto\n"
    );
}

#[test]
fn fix_does_not_rewrite_an_already_clean_file() {
    let dir = tempfile::tempdir().expect("failed to create temp dir");
    let script_path = dir.path().join("Example.psc");
    write_file(&script_path, "ScriptName Example\n");

    let (code, stdout, _stderr) = run_captured(&[
        "fix".to_string(),
        script_path.to_string_lossy().into_owned(),
    ]);

    assert_eq!(code, 0);
    assert_eq!(
        fs::read_to_string(&script_path).unwrap(),
        "ScriptName Example\n"
    );
    assert!(stdout.contains("(0 script(s) fixed.)"));
}

#[test]
fn fix_dry_run_prints_a_diff_without_writing_the_file() {
    let dir = tempfile::tempdir().expect("failed to create temp dir");
    let script_path = dir.path().join("Example.psc");
    let original = "ScriptName Example   \n\nFunction DoThing()\n\tGame.GetPlayer()\nEndFunction\n";
    write_file(&script_path, original);

    let (code, stdout, stderr) = run_captured(&[
        "fix".to_string(),
        "--dry-run".to_string(),
        script_path.to_string_lossy().into_owned(),
    ]);

    assert_eq!(code, 1, "stderr: {stderr}");
    assert_eq!(
        fs::read_to_string(&script_path).unwrap(),
        original,
        "--dry-run must never write to the file"
    );
    let expected_path = script_path.to_string_lossy();
    assert!(stdout.contains(&format!("--- {expected_path}\n")));
    assert!(stdout.contains(&format!("+++ {expected_path}\n")));
    assert!(stdout.contains("-ScriptName Example   \n"));
    assert!(stdout.contains("+ScriptName Example\n"));
    assert!(stdout.contains("(1 script(s) would be fixed.)"));
    assert!(stdout.contains("Game.GetPlayer"));
}

#[test]
fn fix_dry_run_prints_nothing_extra_for_an_already_clean_file() {
    let dir = tempfile::tempdir().expect("failed to create temp dir");
    let script_path = dir.path().join("Example.psc");
    write_file(&script_path, "ScriptName Example\n");

    let (code, stdout, stderr) = run_captured(&[
        "fix".to_string(),
        "--dry-run".to_string(),
        script_path.to_string_lossy().into_owned(),
    ]);

    assert_eq!(code, 0, "stderr: {stderr}");
    assert_eq!(
        fs::read_to_string(&script_path).unwrap(),
        "ScriptName Example\n"
    );
    assert!(!stdout.contains("---"));
    assert!(stdout.contains("(0 script(s) would be fixed.)"));
}

#[test]
fn fix_errors_when_the_achlist_is_missing() {
    let dir = tempfile::tempdir().expect("failed to create temp dir");
    let achlist_path = dir.path().join("missing.achlist");

    let (code, _stdout, stderr) = run_captured(&[
        "fix".to_string(),
        achlist_path.to_string_lossy().into_owned(),
    ]);

    assert_eq!(code, 2);
    assert!(stderr.starts_with("error:"));
}

#[test]
fn fix_removes_an_unused_import_resolved_through_the_project_root() {
    let dir = tempfile::tempdir().expect("failed to create temp dir");
    write_file(
        &dir.path().join("scripts/source/Helpers.psc"),
        "ScriptName Helpers\n\nGlobal Function Assist()\nEndFunction\n",
    );
    let script_path = dir.path().join("scripts/source/Example.psc");
    write_file(
        &script_path,
        "ScriptName Example\n\nImport Helpers\n\nFunction Test()\nEndFunction\n",
    );
    write_file(
        &dir.path().join("sources.achlist"),
        r#"["scripts/source/Helpers.psc", "scripts/source/Example.psc"]"#,
    );
    let achlist_path = dir.path().join("sources.achlist");

    let (code, stdout, stderr) = run_captured(&[
        "fix".to_string(),
        achlist_path.to_string_lossy().into_owned(),
    ]);

    assert_eq!(code, 0, "stdout: {stdout}, stderr: {stderr}");
    assert_eq!(
        fs::read_to_string(&script_path).unwrap(),
        "ScriptName Example\n\n\nFunction Test()\nEndFunction\n"
    );
    assert!(stdout.contains("(1 script(s) fixed.)"));
}

#[test]
fn fix_type_filter_for_unused_import_only_touches_that_rule() {
    let dir = tempfile::tempdir().expect("failed to create temp dir");
    write_file(
        &dir.path().join("scripts/source/Helpers.psc"),
        "ScriptName Helpers\n\nGlobal Function Assist()\nEndFunction\n",
    );
    let script_path = dir.path().join("scripts/source/Example.psc");
    write_file(
        &script_path,
        "ScriptName Example\n\nImport Helpers\n\nFunction Test()\n    Call(1,2)\nEndFunction\n",
    );
    write_file(
        &dir.path().join("sources.achlist"),
        r#"["scripts/source/Helpers.psc", "scripts/source/Example.psc"]"#,
    );
    let achlist_path = dir.path().join("sources.achlist");

    let (code, _stdout, stderr) = run_captured(&[
        "fix".to_string(),
        "--type=unused-import".to_string(),
        achlist_path.to_string_lossy().into_owned(),
    ]);

    assert_eq!(code, 0, "stderr: {stderr}");
    assert_eq!(
        fs::read_to_string(&script_path).unwrap(),
        "ScriptName Example\n\n\nFunction Test()\n    Call(1,2)\nEndFunction\n"
    );
}

#[test]
fn fix_line_filter_errors_when_removing_an_unused_import_changes_the_line_count() {
    let dir = tempfile::tempdir().expect("failed to create temp dir");
    write_file(
        &dir.path().join("scripts/source/Helpers.psc"),
        "ScriptName Helpers\n\nGlobal Function Assist()\nEndFunction\n",
    );
    let script_path = dir.path().join("scripts/source/Example.psc");
    write_file(
        &script_path,
        "ScriptName Example\n\nImport Helpers\n\nFunction Test()\nEndFunction\n",
    );
    write_file(
        &dir.path().join("sources.achlist"),
        r#"["scripts/source/Helpers.psc", "scripts/source/Example.psc"]"#,
    );
    let achlist_path = dir.path().join("sources.achlist");

    let (code, _stdout, stderr) = run_captured(&[
        "fix".to_string(),
        "--line=3".to_string(),
        achlist_path.to_string_lossy().into_owned(),
    ]);

    assert_eq!(code, 2);
    assert!(stderr.contains("changes the file's line count"));
    assert_eq!(
        fs::read_to_string(&script_path).unwrap(),
        "ScriptName Example\n\nImport Helpers\n\nFunction Test()\nEndFunction\n"
    );
}

#[test]
fn resolves_cross_script_argument_types_from_the_project_root() {
    let dir = tempfile::tempdir().expect("failed to create temp dir");
    write_file(
        &dir.path().join("scripts/source/Greeter.psc"),
        "ScriptName Greeter\n\nFunction Greet(String name)\nEndFunction\n",
    );
    write_file(
        &dir.path().join("scripts/source/Example.psc"),
        "ScriptName Example\n\nGreeter Property Target Auto\n\nFunction Test()\n    Target.Greet(1)\nEndFunction\n",
    );
    write_file(
        &dir.path().join("sources.achlist"),
        r#"["scripts/source/Greeter.psc", "scripts/source/Example.psc"]"#,
    );
    let achlist_path = dir.path().join("sources.achlist");

    let (code, stdout, _stderr) = run_captured(&[achlist_path.to_string_lossy().into_owned()]);

    assert_eq!(code, 1);
    assert!(stdout.contains("(argument-types)"));
}

#[test]
fn flags_a_call_through_a_script_name_to_a_function_not_declared_global() {
    let dir = tempfile::tempdir().expect("failed to create temp dir");
    write_file(
        &dir.path().join("scripts/source/MyScriptOne.psc"),
        "ScriptName MyScriptOne Extends Form\n\nFunction IMNotStatic()\nEndFunction\n",
    );
    write_file(
        &dir.path().join("scripts/source/MyScriptTwo.psc"),
        "ScriptName MyScriptTwo Extends Form\n\nFunction Mine()\n    MyScriptOne.IMNotStatic()\nEndFunction\n",
    );
    write_file(
        &dir.path().join("sources.achlist"),
        r#"["scripts/source/MyScriptOne.psc", "scripts/source/MyScriptTwo.psc"]"#,
    );
    let achlist_path = dir.path().join("sources.achlist");

    let (code, stdout, stderr) = run_captured(&[achlist_path.to_string_lossy().into_owned()]);

    assert_eq!(code, 1, "stderr: {stderr}");
    assert!(
        stdout.contains("(non-global-function-call)"),
        "stdout: {stdout}"
    );
    assert!(stdout.contains("'IMNotStatic' is not declared Global on 'MyScriptOne'"));
}

#[test]
fn resolves_cross_script_types_from_every_directory_listed_in_the_achlist() {
    let dir = tempfile::tempdir().expect("failed to create temp dir");
    write_file(
        &dir.path().join("source/dir/one/Greeter.psc"),
        "ScriptName Greeter\n\nFunction Greet(String name)\nEndFunction\n",
    );
    write_file(
        &dir.path().join("source/dir/two/Example.psc"),
        "ScriptName Example\n\nGreeter Property Target Auto\n\nFunction Test()\n    Target.Greet(1)\nEndFunction\n",
    );
    write_file(
        &dir.path().join("sources.achlist"),
        r#"["source/dir/one/Greeter.psc", "source/dir/two/Example.psc"]"#,
    );

    let (code, stdout, stderr) = run_captured(&[dir
        .path()
        .join("sources.achlist")
        .to_string_lossy()
        .into_owned()]);

    assert_eq!(code, 1, "stderr: {stderr}");
    assert!(stdout.contains("(argument-types)"));
}

#[test]
fn goto_state_resolves_a_state_declared_on_a_parent_script_listed_in_the_achlist() {
    // Regression test for https://github.com/Idrinth/papyrus-lint/issues/259:
    // a state declared only on a script's Extends ancestor must not be
    // flagged as missing, even when (as here) the achlist's entries
    // don't sit under either conventional scripts/source layout.
    let dir = tempfile::tempdir().expect("failed to create temp dir");
    write_file(
        &dir.path().join("VahlokStateBase.psc"),
        "Scriptname VahlokStateBase extends ObjectReference\n\nauto State Idle\nEndState\n\nState Busy\nEndState\n",
    );
    write_file(
        &dir.path().join("VahlokStateChild.psc"),
        "Scriptname VahlokStateChild extends VahlokStateBase\n\nState Extra\nEndState\n\nFunction Demo()\n    GoToState(\"Extra\")\n    GoToState(\"Idle\")\n    GoToState(\"Busy\")\n    GoToState(\"NoSuchState\")\nEndFunction\n",
    );
    write_file(
        &dir.path().join("scripts.achlist"),
        r#"["VahlokStateBase.psc", "VahlokStateChild.psc"]"#,
    );

    let (code, stdout, stderr) = run_captured(&[dir
        .path()
        .join("scripts.achlist")
        .to_string_lossy()
        .into_owned()]);

    assert_eq!(code, 0, "stderr: {stderr}, stdout: {stdout}");
    assert!(!stdout.contains("'Extra'"));
    assert!(!stdout.contains("'Idle'"));
    assert!(!stdout.contains("'Busy'"));
    assert!(stdout.contains("(goto-state)"));
    assert!(stdout.contains("'NoSuchState'"));
}

#[test]
fn achlist_resolves_an_unlisted_sibling_script_by_default_for_backward_compatibility() {
    // `strict_achlist_scope` defaults to false, so an achlist-based
    // project already depending on the pre-#311-fix behavior (every
    // listed entry's directory acting as a generic search root) must
    // see no change: `Unlisted.psc` sits right beside the listed
    // `Example.psc`, is never itself mentioned in the achlist, and
    // still resolves.
    let dir = tempfile::tempdir().expect("failed to create temp dir");
    write_file(
        &dir.path().join("mods/one/Example.psc"),
        "ScriptName Example\n\nFunction Test()\n    Unlisted.DoThing()\nEndFunction\n",
    );
    write_file(
        &dir.path().join("mods/one/Unlisted.psc"),
        "ScriptName Unlisted\n\nFunction DoThing() Global\nEndFunction\n",
    );
    write_file(
        &dir.path().join("scripts.achlist"),
        r#"["mods/one/Example.psc"]"#,
    );

    let (code, stdout, stderr) = run_captured(&[dir
        .path()
        .join("scripts.achlist")
        .to_string_lossy()
        .into_owned()]);

    assert_eq!(code, 0, "stderr: {stderr}");
    assert!(!stdout.contains("(unresolved-script)"), "stdout: {stdout}");
}

#[test]
fn strict_achlist_scope_does_not_leak_into_an_unlisted_sibling_script() {
    // Regression test for https://github.com/Idrinth/papyrus-lint/issues/311:
    // with `strict_achlist_scope: true`, an achlist entry's directory
    // must not become a generic search root, since that would silently
    // make every *other* file in that directory resolvable too, even
    // though it was never listed. Here `Unlisted.psc` sits right beside
    // the listed `Example.psc` but is itself never mentioned in the
    // achlist, so a call against its type must be reported as
    // unresolved once strict scoping is turned on.
    let dir = tempfile::tempdir().expect("failed to create temp dir");
    write_file(
        &dir.path().join("mods/one/Example.psc"),
        "ScriptName Example\n\nFunction Test()\n    Unlisted.DoThing()\nEndFunction\n",
    );
    write_file(
        &dir.path().join("mods/one/Unlisted.psc"),
        "ScriptName Unlisted\n\nFunction DoThing() Global\nEndFunction\n",
    );
    write_file(
        &dir.path().join("scripts.achlist"),
        r#"["mods/one/Example.psc"]"#,
    );
    write_file(
        &dir.path().join("papyrus-lint.yaml"),
        "strict_achlist_scope: true\n",
    );

    let (code, stdout, stderr) = run_captured(&[dir
        .path()
        .join("scripts.achlist")
        .to_string_lossy()
        .into_owned()]);

    assert_eq!(code, 0, "stderr: {stderr}");
    assert!(stdout.contains("(unresolved-script)"), "stdout: {stdout}");
    assert!(stdout.contains("'Unlisted'"), "stdout: {stdout}");
}

#[test]
fn strict_achlist_scope_is_honored_from_an_explicit_config_path() {
    // Regression test for https://github.com/Idrinth/papyrus-lint/issues/362:
    // `--config <path>` must still pick up `strict_achlist_scope` from
    // the file it names, rather than always resolving as if it were
    // off (which made the flag appear entirely inert whenever
    // `--config` was used, and left an achlist entry's directory
    // reachable as a search root when it should not have been).
    let dir = tempfile::tempdir().expect("failed to create temp dir");
    write_file(
        &dir.path().join("mods/one/Example.psc"),
        "ScriptName Example\n\nFunction Test()\n    Unlisted.DoThing()\nEndFunction\n",
    );
    write_file(
        &dir.path().join("mods/one/Unlisted.psc"),
        "ScriptName Unlisted\n\nFunction DoThing() Global\nEndFunction\n",
    );
    write_file(
        &dir.path().join("scripts.achlist"),
        r#"["mods/one/Example.psc"]"#,
    );
    let config_path = dir.path().join("custom-config.yaml");
    write_file(&config_path, "strict_achlist_scope: true\n");

    let (code, stdout, stderr) = run_captured(&[
        "--config".to_string(),
        config_path.to_string_lossy().into_owned(),
        dir.path()
            .join("scripts.achlist")
            .to_string_lossy()
            .into_owned(),
    ]);

    assert_eq!(code, 0, "stderr: {stderr}");
    assert!(stdout.contains("(unresolved-script)"), "stdout: {stdout}");
    assert!(stdout.contains("'Unlisted'"), "stdout: {stdout}");
}

#[test]
fn strict_achlist_scope_still_flags_conflicting_versions_between_two_achlist_entries_sharing_a_file_name(
) {
    // Regression test for https://github.com/Idrinth/papyrus-lint/issues/311:
    // with `strict_achlist_scope: true`, two achlist entries can share a
    // file name while living in directories that are no longer scanned
    // as search roots for one another (see the previous test), so the
    // conflicting-script-versions check has to compare the achlist's
    // own listed entries directly rather than relying on a directory
    // scan to notice the collision.
    let dir = tempfile::tempdir().expect("failed to create temp dir");
    write_file(
        &dir.path().join("mods/one/Example.psc"),
        "ScriptName Example\n",
    );
    write_file(
        &dir.path().join("mods/two/Example.psc"),
        "ScriptName Example\n; a different version\n",
    );
    write_file(
        &dir.path().join("scripts.achlist"),
        r#"["mods/one/Example.psc", "mods/two/Example.psc"]"#,
    );
    write_file(
        &dir.path().join("papyrus-lint.yaml"),
        "strict_achlist_scope: true\n",
    );

    let (code, stdout, stderr) = run_captured(&[dir
        .path()
        .join("scripts.achlist")
        .to_string_lossy()
        .into_owned()]);

    assert_eq!(code, 0, "stderr: {stderr}");
    assert_eq!(
        stdout.matches("(conflicting-script-versions)").count(),
        2,
        "stdout: {stdout}"
    );
}

#[test]
fn strict_achlist_scope_does_not_double_report_a_conflict_also_visible_via_a_conventional_directory(
) {
    // Regression test: when two conflicting achlist entries also happen
    // to sit under the project's conventional scripts/source and
    // source/scripts directories, strict mode must report the
    // collision once per file (via conflicting_script_versions_among),
    // not twice (once more via the directory-based
    // conflicting_script_versions, which strict mode must skip
    // entirely to avoid duplicating what it already reports).
    let dir = tempfile::tempdir().expect("failed to create temp dir");
    write_file(
        &dir.path().join("scripts/source/Example.psc"),
        "ScriptName Example\n",
    );
    write_file(
        &dir.path().join("source/scripts/Example.psc"),
        "ScriptName Example\n; a different version\n",
    );
    write_file(
        &dir.path().join("scripts.achlist"),
        r#"["scripts/source/Example.psc", "source/scripts/Example.psc"]"#,
    );
    write_file(
        &dir.path().join("papyrus-lint.yaml"),
        "strict_achlist_scope: true\n",
    );

    let (code, stdout, stderr) = run_captured(&[dir
        .path()
        .join("scripts.achlist")
        .to_string_lossy()
        .into_owned()]);

    assert_eq!(code, 0, "stderr: {stderr}");
    assert_eq!(
        stdout.matches("(conflicting-script-versions)").count(),
        2,
        "stdout: {stdout}"
    );
}

#[test]
fn flags_a_script_newer_than_its_compiled_pex() {
    let dir = tempfile::tempdir().expect("failed to create temp dir");
    let script_path = dir.path().join("scripts/source/Example.psc");
    let pex_path = dir.path().join("scripts/Example.pex");
    write_file(&script_path, "ScriptName Example\n");
    write_file(&pex_path, "");

    let now = std::time::SystemTime::now();
    let pex_file = fs::File::open(&pex_path).expect("failed to open pex file");
    pex_file
        .set_modified(now - std::time::Duration::from_secs(60))
        .expect("failed to set pex mtime");
    let script_file = fs::File::open(&script_path).expect("failed to open script file");
    script_file
        .set_modified(now)
        .expect("failed to set script mtime");

    let (code, stdout, stderr) = run_captured(&[script_path.to_string_lossy().into_owned()]);

    assert_eq!(code, 0, "stderr: {stderr}");
    assert!(
        stdout.contains("(stale-compiled-output)"),
        "stdout: {stdout}"
    );
    assert!(stdout.contains("[info]"), "stdout: {stdout}");
}

#[test]
fn does_not_flag_a_script_older_than_its_compiled_pex() {
    let dir = tempfile::tempdir().expect("failed to create temp dir");
    let script_path = dir.path().join("scripts/source/Example.psc");
    let pex_path = dir.path().join("scripts/Example.pex");
    write_file(&script_path, "ScriptName Example\n");
    write_file(&pex_path, "");

    let now = std::time::SystemTime::now();
    let script_file = fs::File::open(&script_path).expect("failed to open script file");
    script_file
        .set_modified(now - std::time::Duration::from_secs(60))
        .expect("failed to set script mtime");
    let pex_file = fs::File::open(&pex_path).expect("failed to open pex file");
    pex_file.set_modified(now).expect("failed to set pex mtime");

    let (code, stdout, stderr) = run_captured(&[script_path.to_string_lossy().into_owned()]);

    assert_eq!(code, 0, "stderr: {stderr}");
    assert!(
        !stdout.contains("(stale-compiled-output)"),
        "stdout: {stdout}"
    );
}

#[test]
fn stale_compiled_output_can_be_disabled() {
    let dir = tempfile::tempdir().expect("failed to create temp dir");
    let script_path = dir.path().join("scripts/source/Example.psc");
    let pex_path = dir.path().join("scripts/Example.pex");
    write_file(&script_path, "ScriptName Example\n");
    write_file(&pex_path, "");
    write_file(
        &dir.path().join("papyrus-lint.yaml"),
        "rules:\n  stale_compiled_output: false\n",
    );

    let now = std::time::SystemTime::now();
    let pex_file = fs::File::open(&pex_path).expect("failed to open pex file");
    pex_file
        .set_modified(now - std::time::Duration::from_secs(60))
        .expect("failed to set pex mtime");
    let script_file = fs::File::open(&script_path).expect("failed to open script file");
    script_file
        .set_modified(now)
        .expect("failed to set script mtime");

    let (code, stdout, stderr) = run_captured(&[script_path.to_string_lossy().into_owned()]);

    assert_eq!(code, 0, "stderr: {stderr}");
    assert!(
        !stdout.contains("(stale-compiled-output)"),
        "stdout: {stdout}"
    );
}

#[test]
fn flags_a_script_name_that_does_not_match_its_file_name() {
    let dir = tempfile::tempdir().expect("failed to create temp dir");
    let script_path = dir.path().join("scripts/source/Other.psc");
    write_file(&script_path, "ScriptName Example\n");

    let (code, stdout, stderr) = run_captured(&[script_path.to_string_lossy().into_owned()]);

    assert_eq!(code, 1, "stderr: {stderr}");
    assert!(
        stdout.contains("(script-filename-mismatch)"),
        "stdout: {stdout}"
    );
    assert!(stdout.contains("[error]"), "stdout: {stdout}");
}

#[test]
fn does_not_flag_a_script_name_matching_its_file_name() {
    let dir = tempfile::tempdir().expect("failed to create temp dir");
    let script_path = dir.path().join("scripts/source/Example.psc");
    write_file(&script_path, "ScriptName Example\n");

    let (code, stdout, stderr) = run_captured(&[script_path.to_string_lossy().into_owned()]);

    assert_eq!(code, 0, "stderr: {stderr}");
    assert!(
        !stdout.contains("(script-filename-mismatch)"),
        "stdout: {stdout}"
    );
}

#[test]
fn script_filename_mismatch_can_be_disabled() {
    let dir = tempfile::tempdir().expect("failed to create temp dir");
    let script_path = dir.path().join("scripts/source/Other.psc");
    write_file(&script_path, "ScriptName Example\n");
    write_file(
        &dir.path().join("papyrus-lint.yaml"),
        "rules:\n  script_filename_mismatch: false\n",
    );

    let (code, stdout, stderr) = run_captured(&[script_path.to_string_lossy().into_owned()]);

    assert_eq!(code, 0, "stderr: {stderr}");
    assert!(
        !stdout.contains("(script-filename-mismatch)"),
        "stdout: {stdout}"
    );
}

#[test]
fn script_filename_mismatch_can_be_suppressed_with_a_disable_comment() {
    let dir = tempfile::tempdir().expect("failed to create temp dir");
    let script_path = dir.path().join("scripts/source/Other.psc");
    write_file(
        &script_path,
        "ScriptName Example ; @disable script-filename-mismatch\n",
    );

    let (code, stdout, stderr) = run_captured(&[script_path.to_string_lossy().into_owned()]);

    assert_eq!(code, 0, "stderr: {stderr}");
    assert!(
        !stdout.contains("(script-filename-mismatch)"),
        "stdout: {stdout}"
    );
}

#[test]
fn script_filename_mismatch_can_be_suppressed_with_a_disable_file_comment() {
    let dir = tempfile::tempdir().expect("failed to create temp dir");
    let script_path = dir.path().join("scripts/source/Other.psc");
    write_file(
        &script_path,
        "ScriptName Example\n; @disable-file script-filename-mismatch\n",
    );

    let (code, stdout, stderr) = run_captured(&[script_path.to_string_lossy().into_owned()]);

    assert_eq!(code, 0, "stderr: {stderr}");
    assert!(
        !stdout.contains("(script-filename-mismatch)"),
        "stdout: {stdout}"
    );
}

// Regression tests for
// https://github.com/Idrinth/papyrus-lint/issues/772: `stale-compiled-output`,
// `conflicting-script-versions`, and `script-filename-mismatch` are
// computed after `papyrus_lints::lint_with_external_arguments` used to
// run its own unused-directive validation, so an `@disable`/
// `@disable-file` directive that correctly suppressed one of them was
// still reported as an `unused-disable`, even though the diagnostic it
// named was in fact suppressed.

#[test]
fn stale_compiled_output_disable_comment_is_not_reported_as_unused() {
    let dir = tempfile::tempdir().expect("failed to create temp dir");
    let script_path = dir.path().join("scripts/source/Example.psc");
    let pex_path = dir.path().join("scripts/Example.pex");
    write_file(
        &script_path,
        "ScriptName Example ; @disable stale-compiled-output\n",
    );
    write_file(&pex_path, "");
    write_file(
        &dir.path().join("papyrus-lint.yaml"),
        "rules:\n  unused_disable: true\n",
    );

    let now = std::time::SystemTime::now();
    let pex_file = fs::File::open(&pex_path).expect("failed to open pex file");
    pex_file
        .set_modified(now - std::time::Duration::from_secs(60))
        .expect("failed to set pex mtime");
    let script_file = fs::File::open(&script_path).expect("failed to open script file");
    script_file
        .set_modified(now)
        .expect("failed to set script mtime");

    let (code, stdout, stderr) = run_captured(&[script_path.to_string_lossy().into_owned()]);

    assert_eq!(code, 0, "stderr: {stderr}");
    assert!(
        !stdout.contains("(stale-compiled-output)"),
        "stdout: {stdout}"
    );
    assert!(!stdout.contains("(unused-disable)"), "stdout: {stdout}");
}

#[test]
fn stale_compiled_output_disable_file_comment_is_not_reported_as_unused() {
    let dir = tempfile::tempdir().expect("failed to create temp dir");
    let script_path = dir.path().join("scripts/source/Example.psc");
    let pex_path = dir.path().join("scripts/Example.pex");
    write_file(
        &script_path,
        "ScriptName Example\n; @disable-file stale-compiled-output\n",
    );
    write_file(&pex_path, "");
    write_file(
        &dir.path().join("papyrus-lint.yaml"),
        "rules:\n  unused_disable: true\n",
    );

    let now = std::time::SystemTime::now();
    let pex_file = fs::File::open(&pex_path).expect("failed to open pex file");
    pex_file
        .set_modified(now - std::time::Duration::from_secs(60))
        .expect("failed to set pex mtime");
    let script_file = fs::File::open(&script_path).expect("failed to open script file");
    script_file
        .set_modified(now)
        .expect("failed to set script mtime");

    let (code, stdout, stderr) = run_captured(&[script_path.to_string_lossy().into_owned()]);

    assert_eq!(code, 0, "stderr: {stderr}");
    assert!(
        !stdout.contains("(stale-compiled-output)"),
        "stdout: {stdout}"
    );
    assert!(!stdout.contains("(unused-disable)"), "stdout: {stdout}");
}

#[test]
fn conflicting_script_versions_disable_comment_is_not_reported_as_unused() {
    let dir = tempfile::tempdir().expect("failed to create temp dir");
    let script_path = dir.path().join("scripts/source/Example.psc");
    write_file(
        &script_path,
        "ScriptName Example ; @disable conflicting-script-versions\n",
    );
    write_file(
        &dir.path().join("source/scripts/Example.psc"),
        "ScriptName Example\n; a different version\n",
    );
    write_file(
        &dir.path().join("papyrus-lint.yaml"),
        "rules:\n  unused_disable: true\n",
    );

    let (code, stdout, stderr) = run_captured(&[script_path.to_string_lossy().into_owned()]);

    assert_eq!(code, 0, "stderr: {stderr}");
    assert!(
        !stdout.contains("(conflicting-script-versions)"),
        "stdout: {stdout}"
    );
    assert!(!stdout.contains("(unused-disable)"), "stdout: {stdout}");
}

#[test]
fn conflicting_script_versions_disable_file_comment_is_not_reported_as_unused() {
    let dir = tempfile::tempdir().expect("failed to create temp dir");
    let script_path = dir.path().join("scripts/source/Example.psc");
    write_file(
        &script_path,
        "ScriptName Example\n; @disable-file conflicting-script-versions\n",
    );
    write_file(
        &dir.path().join("source/scripts/Example.psc"),
        "ScriptName Example\n; a different version\n",
    );
    write_file(
        &dir.path().join("papyrus-lint.yaml"),
        "rules:\n  unused_disable: true\n",
    );

    let (code, stdout, stderr) = run_captured(&[script_path.to_string_lossy().into_owned()]);

    assert_eq!(code, 0, "stderr: {stderr}");
    assert!(
        !stdout.contains("(conflicting-script-versions)"),
        "stdout: {stdout}"
    );
    assert!(!stdout.contains("(unused-disable)"), "stdout: {stdout}");
}

#[test]
fn script_filename_mismatch_disable_comment_is_not_reported_as_unused() {
    let dir = tempfile::tempdir().expect("failed to create temp dir");
    let script_path = dir.path().join("scripts/source/Other.psc");
    write_file(
        &script_path,
        "ScriptName Example ; @disable script-filename-mismatch\n",
    );
    write_file(
        &dir.path().join("papyrus-lint.yaml"),
        "rules:\n  unused_disable: true\n",
    );

    let (code, stdout, stderr) = run_captured(&[script_path.to_string_lossy().into_owned()]);

    assert_eq!(code, 0, "stderr: {stderr}");
    assert!(
        !stdout.contains("(script-filename-mismatch)"),
        "stdout: {stdout}"
    );
    assert!(!stdout.contains("(unused-disable)"), "stdout: {stdout}");
}

#[test]
fn script_filename_mismatch_disable_file_comment_is_not_reported_as_unused() {
    let dir = tempfile::tempdir().expect("failed to create temp dir");
    let script_path = dir.path().join("scripts/source/Other.psc");
    write_file(
        &script_path,
        "ScriptName Example\n; @disable-file script-filename-mismatch\n",
    );
    write_file(
        &dir.path().join("papyrus-lint.yaml"),
        "rules:\n  unused_disable: true\n",
    );

    let (code, stdout, stderr) = run_captured(&[script_path.to_string_lossy().into_owned()]);

    assert_eq!(code, 0, "stderr: {stderr}");
    assert!(
        !stdout.contains("(script-filename-mismatch)"),
        "stdout: {stdout}"
    );
    assert!(!stdout.contains("(unused-disable)"), "stdout: {stdout}");
}

#[test]
fn resolves_cross_script_argument_types_from_the_projects_configured_script_root() {
    let dir = tempfile::tempdir().expect("failed to create temp dir");
    let shared_dir = tempfile::tempdir().expect("failed to create temp dir");
    write_file(
        &shared_dir.path().join("Greeter.psc"),
        "ScriptName Greeter\n\nFunction Greet(String name)\nEndFunction\n",
    );
    write_file(
        &dir.path().join("scripts/source/Example.psc"),
        "ScriptName Example\n\nGreeter Property Target Auto\n\nFunction Test()\n    Target.Greet(1)\nEndFunction\n",
    );
    write_file(
        &dir.path().join("sources.achlist"),
        r#"["scripts/source/Example.psc"]"#,
    );
    write_file(
        &dir.path().join("papyrus-lint.yaml"),
        &format!(
            "additional_script_roots:\n  - {}\n",
            shared_dir.path().display()
        ),
    );
    let achlist_path = dir.path().join("sources.achlist");

    let (code, stdout, _stderr) = run_captured(&[achlist_path.to_string_lossy().into_owned()]);

    assert_eq!(code, 1);
    assert!(stdout.contains("(argument-types)"));
}

#[test]
fn resolves_cross_script_argument_types_from_lookup_script_roots_without_linting_them() {
    let dir = tempfile::tempdir().expect("failed to create temp dir");
    let vanilla_dir = tempfile::tempdir().expect("failed to create temp dir");
    write_file(
        &vanilla_dir.path().join("Greeter.psc"),
        "ScriptName Greeter\n\nFunction Greet(String name)\nEndFunction\n",
    );
    write_file(
        &dir.path().join("scripts/source/Example.psc"),
        "ScriptName Example\n\nGreeter Property Target Auto\n\nFunction Test()\n    Target.Greet(1)\nEndFunction\n",
    );
    write_file(
        &dir.path().join("sources.achlist"),
        r#"["scripts/source/Example.psc"]"#,
    );
    write_file(
        &dir.path().join("papyrus-lint.yaml"),
        &format!(
            "lookup_script_roots:\n  - {}\n",
            vanilla_dir.path().display()
        ),
    );
    let achlist_path = dir.path().join("sources.achlist");

    let (code, stdout, _stderr) = run_captured(&[achlist_path.to_string_lossy().into_owned()]);

    assert_eq!(code, 1);
    assert!(stdout.contains("(argument-types)"));
    assert!(
        !stdout.contains("Greeter.psc"),
        "lookup-root scripts must not be linted"
    );
}

#[test]
fn lookup_script_roots_do_not_report_conflicting_script_versions() {
    let dir = tempfile::tempdir().expect("failed to create temp dir");
    write_file(
        &dir.path().join("scripts/source/Actor.psc"),
        "ScriptName Actor\n",
    );
    let vanilla_dir = tempfile::tempdir().expect("failed to create temp dir");
    write_file(
        &vanilla_dir.path().join("Actor.psc"),
        "ScriptName Actor extends Form\n",
    );
    write_file(
        &dir.path().join("sources.achlist"),
        r#"["scripts/source/Actor.psc"]"#,
    );
    write_file(
        &dir.path().join("papyrus-lint.yaml"),
        &format!(
            "lookup_script_roots:\n  - {}\n",
            vanilla_dir.path().display()
        ),
    );
    let achlist_path = dir.path().join("sources.achlist");

    let (code, stdout, _stderr) = run_captured(&[achlist_path.to_string_lossy().into_owned()]);

    assert_eq!(code, 0);
    assert!(
        !stdout.contains("conflicting-script-versions"),
        "lookup roots must not participate in collision checks"
    );
}

#[test]
fn resolves_cross_script_argument_types_from_a_script_root_flag() {
    let dir = tempfile::tempdir().expect("failed to create temp dir");
    let shared_dir = tempfile::tempdir().expect("failed to create temp dir");
    write_file(
        &shared_dir.path().join("Greeter.psc"),
        "ScriptName Greeter\n\nFunction Greet(String name)\nEndFunction\n",
    );
    write_file(
        &dir.path().join("scripts/source/Example.psc"),
        "ScriptName Example\n\nGreeter Property Target Auto\n\nFunction Test()\n    Target.Greet(1)\nEndFunction\n",
    );
    write_file(
        &dir.path().join("sources.achlist"),
        r#"["scripts/source/Example.psc"]"#,
    );
    let achlist_path = dir.path().join("sources.achlist");

    let (code, stdout, _stderr) = run_captured(&[
        "--script-root".to_string(),
        shared_dir.path().to_string_lossy().into_owned(),
        achlist_path.to_string_lossy().into_owned(),
    ]);

    assert_eq!(code, 1);
    assert!(stdout.contains("(argument-types)"));
}

#[test]
fn config_flag_skips_the_project_roots_additional_script_roots() {
    let dir = tempfile::tempdir().expect("failed to create temp dir");
    let shared_dir = tempfile::tempdir().expect("failed to create temp dir");
    write_file(
        &shared_dir.path().join("Greeter.psc"),
        "ScriptName Greeter\n\nFunction Greet(String name)\nEndFunction\n",
    );
    write_file(
        &dir.path().join("scripts/source/Example.psc"),
        "ScriptName Example\n\nGreeter Property Target Auto\n\nFunction Test()\n    Target.Greet(1)\nEndFunction\n",
    );
    write_file(
        &dir.path().join("sources.achlist"),
        r#"["scripts/source/Example.psc"]"#,
    );
    write_file(
        &dir.path().join("papyrus-lint.yaml"),
        &format!(
            "additional_script_roots:\n  - {}\n",
            shared_dir.path().display()
        ),
    );
    let override_path = dir.path().join("overrides/custom.yaml");
    write_file(&override_path, "");
    let achlist_path = dir.path().join("sources.achlist");

    let (code, stdout, _stderr) = run_captured(&[
        "--config".to_string(),
        override_path.to_string_lossy().into_owned(),
        achlist_path.to_string_lossy().into_owned(),
    ]);

    // Greeter can't be resolved (the project root's own config, which
    // declares the shared_dir root, is bypassed by --config), so the
    // "Argument type check" lint has nothing to flag.
    assert_eq!(code, 0);
    assert!(!stdout.contains("(argument-types)"));
}

#[test]
fn config_flag_overrides_project_root_discovery() {
    let dir = tempfile::tempdir().expect("failed to create temp dir");
    write_file(
        &dir.path().join("scripts/source/Example.psc"),
        "ScriptName Example   \n",
    );
    write_file(
        &dir.path().join("sources.achlist"),
        r#"["scripts/source/Example.psc"]"#,
    );
    // A project-root config that would otherwise apply, and an
    // unrelated override file elsewhere that should win instead.
    write_file(
        &dir.path().join("papyrus-lint.yaml"),
        "rules:\n  trailing_whitespace: true\n",
    );
    let override_path = dir.path().join("overrides/custom.yaml");
    write_file(&override_path, "rules:\n  trailing_whitespace: false\n");
    let achlist_path = dir.path().join("sources.achlist");

    let (code, stdout, _stderr) = run_captured(&[
        "--config".to_string(),
        override_path.to_string_lossy().into_owned(),
        achlist_path.to_string_lossy().into_owned(),
    ]);

    assert_eq!(code, 0);
    assert!(stdout.contains("no problems found"));
}

#[test]
fn config_flag_combines_with_fix_and_json_in_any_order() {
    let dir = tempfile::tempdir().expect("failed to create temp dir");
    write_file(
        &dir.path().join("scripts/source/Example.psc"),
        "ScriptName Example   \n",
    );
    write_file(
        &dir.path().join("sources.achlist"),
        r#"["scripts/source/Example.psc"]"#,
    );
    let override_path = dir.path().join("overrides/custom.yaml");
    write_file(&override_path, "rules:\n  trailing_whitespace: false\n");
    let achlist_path = dir.path().join("sources.achlist");

    let (code, stdout, _stderr) = run_captured(&[
        "--json".to_string(),
        "fix".to_string(),
        "--config".to_string(),
        override_path.to_string_lossy().into_owned(),
        achlist_path.to_string_lossy().into_owned(),
    ]);

    assert_eq!(code, 0);
    // trailing_whitespace was disabled by the overriding config, so
    // fix should leave the trailing whitespace in place untouched.
    assert_eq!(
        fs::read_to_string(dir.path().join("scripts/source/Example.psc")).unwrap(),
        "ScriptName Example   \n"
    );
    let report: serde_json::Value = serde_json::from_str(&stdout).unwrap();
    assert_eq!(report["success"], true);
    assert_eq!(report["total_diagnostics"], 0);
}

#[test]
fn config_flag_errors_when_the_override_file_is_missing() {
    let dir = tempfile::tempdir().expect("failed to create temp dir");
    write_file(
        &dir.path().join("scripts/source/Example.psc"),
        "ScriptName Example\n",
    );
    write_file(
        &dir.path().join("sources.achlist"),
        r#"["scripts/source/Example.psc"]"#,
    );
    let achlist_path = dir.path().join("sources.achlist");
    let missing_config = dir.path().join("missing.yaml");

    let (code, _stdout, stderr) = run_captured(&[
        "--config".to_string(),
        missing_config.to_string_lossy().into_owned(),
        achlist_path.to_string_lossy().into_owned(),
    ]);

    assert_eq!(code, 2);
    assert!(stderr.starts_with("error: failed to load lint config:"));
}

#[test]
fn a_psc_path_that_is_a_directory_reports_a_read_error() {
    let dir = tempfile::tempdir().expect("failed to create temp dir");
    let misleading_path = dir.path().join("NotAFile.psc");
    fs::create_dir(&misleading_path).expect("failed to create directory");

    let (code, stdout, stderr) = run_captured(&[misleading_path.to_string_lossy().into_owned()]);

    assert_eq!(code, 2);
    assert!(stdout.is_empty());
    assert!(stderr.contains("error: failed to read"));
    assert!(stderr.contains("NotAFile.psc"));
}

#[test]
#[cfg(unix)]
fn compile_check_merges_compiler_reported_errors_into_the_lint_report() {
    let dir = tempfile::tempdir().expect("failed to create temp dir");
    let source_dir = dir.path().join("scripts/source");
    let script = source_dir.join("Example.psc");
    write_file(&script, "ScriptName Example\n");
    let compiler_path = write_stub_compiler(
        dir.path(),
        "#!/bin/sh\necho \"Example.psc(3,4): custom compiler error\" >&2\nexit 1\n",
    );
    write_file(
        &dir.path().join("papyrus-lint.yaml"),
        &format!(
            "compile_check: true\ncompiler_path: {}\n",
            compiler_path.display()
        ),
    );

    let (code, stdout, _stderr) =
        run_captured(&["--json".to_string(), script.to_string_lossy().into_owned()]);

    assert_eq!(code, 1);
    assert!(stdout.contains("\"rule\": \"compiler-error\""));
    assert!(stdout.contains("custom compiler error"));
}

#[test]
#[cfg(unix)]
fn compile_check_disabled_by_default_ignores_a_failing_compiler() {
    let dir = tempfile::tempdir().expect("failed to create temp dir");
    let source_dir = dir.path().join("scripts/source");
    let script = source_dir.join("Example.psc");
    write_file(&script, "ScriptName Example\n");
    let compiler_path = write_stub_compiler(
        dir.path(),
        "#!/bin/sh\necho \"Example.psc(3,4): custom compiler error\" >&2\nexit 1\n",
    );
    // No `compile_check: true`, only a configured `compiler_path` — the
    // compiler must never be invoked at all when the setting is off.
    write_file(
        &dir.path().join("papyrus-lint.yaml"),
        &format!("compiler_path: {}\n", compiler_path.display()),
    );

    let (code, stdout, _stderr) =
        run_captured(&["--json".to_string(), script.to_string_lossy().into_owned()]);

    assert_eq!(code, 0);
    assert!(!stdout.contains("compiler-error"));
}

#[test]
#[cfg(unix)]
fn compile_check_is_ignored_when_no_compiler_path_can_be_resolved() {
    let dir = tempfile::tempdir().expect("failed to create temp dir");
    let source_dir = dir.path().join("scripts/source");
    let script = source_dir.join("Example.psc");
    write_file(&script, "ScriptName Example\n");
    write_file(
        &dir.path().join("papyrus-lint.yaml"),
        "compile_check: true\n",
    );

    let (code, stdout, _stderr) =
        run_captured(&["--json".to_string(), script.to_string_lossy().into_owned()]);

    assert_eq!(code, 0);
    assert!(!stdout.contains("compiler-error"));
}

#[test]
fn direct_psc_detection_is_case_insensitive() {
    let dir = tempfile::tempdir().expect("failed to create temp dir");
    let script = dir.path().join("Example.PSC");
    write_file(&script, "ScriptName Example   \n");

    let (code, stdout, stderr) = run_captured(&[script.to_string_lossy().into_owned()]);

    assert_eq!(code, 0, "stderr: {stderr}");
    assert!(stderr.is_empty());
    assert!(stdout.contains("(trailing-whitespace)"));
    assert!(stdout.contains("1 problem(s) found in 1 of 1 script(s)"));
}
