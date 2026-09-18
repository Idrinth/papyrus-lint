//! `additional_script_roots`/`lookup_script_roots`, `--script-root`, and `--config`.

use crate::test_support::*;
use std::fs;

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
    assert!(stdout.contains("[argument-types]"));
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
    assert!(stdout.contains("[argument-types]"));
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
    assert!(stdout.contains("[argument-types]"));
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
    assert!(!stdout.contains("[argument-types]"));
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
