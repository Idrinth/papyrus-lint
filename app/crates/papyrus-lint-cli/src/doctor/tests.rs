use crate::test_support::*;
use std::fs;

#[test]
fn doctor_reports_usage_error_without_a_path() {
    let (code, stdout, stderr) = run_captured(&["doctor".to_string()]);

    assert_eq!(code, 2);
    assert!(stdout.is_empty());
    assert!(stderr.contains("Usage: PapyrusLinterCLI"));
}

#[test]
fn doctor_rejects_unknown_and_unsupported_formats() {
    for format in ["xml", "ai"] {
        let (code, stdout, stderr) = run_captured(&[
            "doctor".to_string(),
            format!("--format={format}"),
            "Example.psc".to_string(),
        ]);

        assert_eq!(code, 2);
        assert!(stdout.is_empty());
        assert_eq!(
            stderr,
            format!("error: doctor --format must be 'plain' or 'json', got '{format}'\n")
        );
    }
}

#[test]
fn doctor_reports_ok_for_an_existing_psc_with_no_config() {
    let dir = tempfile::tempdir().expect("failed to create temp dir");
    let source_dir = dir.path().join("scripts/source");
    let script = source_dir.join("Example.psc");
    write_file(&script, "ScriptName Example\n");

    let (code, stdout, stderr) =
        run_captured(&["doctor".to_string(), script.to_string_lossy().into_owned()]);

    assert_eq!(code, 0);
    assert!(stderr.is_empty());
    assert!(stdout.contains(&format!("[ok] script {} exists", script.display())));
    assert!(stdout.contains("scripts/source"));
    assert!(stdout.contains("no problems found"));
}

#[test]
fn doctor_reports_error_for_a_missing_psc_file() {
    let dir = tempfile::tempdir().expect("failed to create temp dir");
    let missing = dir.path().join("scripts/source/Missing.psc");

    let (code, stdout, _stderr) =
        run_captured(&["doctor".to_string(), missing.to_string_lossy().into_owned()]);

    assert_eq!(code, 1);
    assert!(stdout.contains(&format!(
        "[error] script {} does not exist",
        missing.display()
    )));
    assert!(stdout.contains("problem(s) found"));
}

#[test]
fn doctor_reports_each_missing_achlist_entry() {
    let dir = tempfile::tempdir().expect("failed to create temp dir");
    let existing = dir.path().join("scripts/source/Present.psc");
    write_file(&existing, "ScriptName Present\n");
    let achlist_path = dir.path().join("project.achlist");
    write_file(
        &achlist_path,
        r#"["scripts/source/Present.psc", "scripts/source/Missing.psc"]"#,
    );

    let (code, stdout, _stderr) = run_captured(&[
        "doctor".to_string(),
        achlist_path.to_string_lossy().into_owned(),
    ]);

    assert_eq!(code, 1);
    assert!(stdout.contains("achlist entry"));
    assert!(stdout.contains("Missing.psc"));
    assert!(stdout.contains("does not exist"));
    assert!(!stdout.contains("every entry"));
}

#[test]
fn doctor_reports_ok_when_every_achlist_entry_exists() {
    let dir = tempfile::tempdir().expect("failed to create temp dir");
    let existing = dir.path().join("scripts/source/Present.psc");
    write_file(&existing, "ScriptName Present\n");
    let achlist_path = dir.path().join("project.achlist");
    write_file(&achlist_path, r#"["scripts/source/Present.psc"]"#);

    let (code, stdout, _stderr) = run_captured(&[
        "doctor".to_string(),
        achlist_path.to_string_lossy().into_owned(),
    ]);

    assert_eq!(code, 0);
    assert!(stdout.contains("every entry"));
    assert!(stdout.contains("exists on disk (1 total)"));
}

#[test]
fn doctor_reports_error_when_the_achlist_itself_is_missing() {
    let dir = tempfile::tempdir().expect("failed to create temp dir");
    let missing_achlist = dir.path().join("missing.achlist");

    let (code, stdout, _stderr) = run_captured(&[
        "doctor".to_string(),
        missing_achlist.to_string_lossy().into_owned(),
    ]);

    assert_eq!(code, 1);
    assert!(stdout.contains("does not exist"));
}

#[test]
fn doctor_warns_when_no_conventional_script_directory_exists() {
    let dir = tempfile::tempdir().expect("failed to create temp dir");
    let achlist_path = dir.path().join("project.achlist");
    write_file(&achlist_path, "[]");

    let (code, stdout, _stderr) = run_captured(&[
        "doctor".to_string(),
        achlist_path.to_string_lossy().into_owned(),
    ]);

    assert_eq!(code, 1);
    assert!(stdout.contains("[warning] neither scripts/source nor source/scripts exists"));
}

#[test]
fn doctor_warns_about_a_missing_configured_additional_script_root() {
    let dir = tempfile::tempdir().expect("failed to create temp dir");
    let source_dir = dir.path().join("scripts/source");
    let script = source_dir.join("Example.psc");
    write_file(&script, "ScriptName Example\n");
    write_file(
        &dir.path().join("papyrus-lint.yaml"),
        "additional_script_roots:\n  - Shared\n",
    );

    let (code, stdout, _stderr) =
        run_captured(&["doctor".to_string(), script.to_string_lossy().into_owned()]);

    assert_eq!(code, 1);
    assert!(stdout.contains("configured additional script root"));
    assert!(stdout.contains("Shared"));
    assert!(stdout.contains("does not exist"));
}

#[test]
fn doctor_reports_ok_for_an_existing_configured_additional_script_root() {
    let dir = tempfile::tempdir().expect("failed to create temp dir");
    let source_dir = dir.path().join("scripts/source");
    let script = source_dir.join("Example.psc");
    write_file(&script, "ScriptName Example\n");
    fs::create_dir_all(dir.path().join("Shared")).expect("failed to create shared dir");
    write_file(
        &dir.path().join("papyrus-lint.yaml"),
        "additional_script_roots:\n  - Shared\n",
    );

    let (code, stdout, _stderr) =
        run_captured(&["doctor".to_string(), script.to_string_lossy().into_owned()]);

    assert_eq!(code, 0);
    assert!(stdout.contains("additional script root"));
    assert!(stdout.contains("Shared"));
    assert!(stdout.contains("exists"));
}

#[test]
fn doctor_checks_each_configured_lookup_script_root() {
    let dir = tempfile::tempdir().expect("failed to create temp dir");
    let source_dir = dir.path().join("scripts/source");
    let script = source_dir.join("Example.psc");
    let existing_lookup_root = dir.path().join("ExistingLookup");
    let missing_lookup_root = dir.path().join("MissingLookup");
    write_file(&script, "ScriptName Example\n");
    fs::create_dir_all(&existing_lookup_root).expect("failed to create lookup root");
    write_file(
        &dir.path().join("papyrus-lint.yaml"),
        "lookup_script_roots:\n  - ExistingLookup\n  - MissingLookup\n",
    );

    let (code, stdout, stderr) =
        run_captured(&["doctor".to_string(), script.to_string_lossy().into_owned()]);

    assert_eq!(code, 1);
    assert!(stderr.is_empty());
    assert!(stdout.contains(&format!(
        "[ok] lookup script root (analysis only) {} exists",
        existing_lookup_root.display()
    )));
    assert!(stdout.contains(&format!(
        "[warning] configured lookup script root {} does not exist",
        missing_lookup_root.display()
    )));
    assert!(stdout.contains("PapyrusLinterCLI doctor: 1 problem(s) found."));
}

#[test]
fn doctor_loads_lookup_script_roots_from_an_explicit_config() {
    let dir = tempfile::tempdir().expect("failed to create temp dir");
    let script = dir.path().join("scripts/source/Example.psc");
    let lookup_root = dir.path().join("SharedLookup");
    let config = dir.path().join("config/doctor.yaml");
    write_file(&script, "ScriptName Example\n");
    fs::create_dir_all(&lookup_root).expect("failed to create lookup root");
    write_file(
        &config,
        &format!(
            "lookup_script_roots:\n  - {}\n",
            lookup_root.to_string_lossy()
        ),
    );

    let (code, stdout, stderr) = run_captured(&[
        "doctor".to_string(),
        "--config".to_string(),
        config.to_string_lossy().into_owned(),
        script.to_string_lossy().into_owned(),
    ]);

    assert_eq!(code, 0);
    assert!(stderr.is_empty());
    assert!(stdout.contains(&format!(
        "[ok] lookup script root (analysis only) {} exists",
        lookup_root.display()
    )));
    assert!(stdout.contains("PapyrusLinterCLI doctor: no problems found."));
}

#[test]
fn doctor_via_script_root_flag_is_checked_even_without_a_config_file() {
    let dir = tempfile::tempdir().expect("failed to create temp dir");
    let source_dir = dir.path().join("scripts/source");
    let script = source_dir.join("Example.psc");
    write_file(&script, "ScriptName Example\n");

    let (code, stdout, _stderr) = run_captured(&[
        "doctor".to_string(),
        "--script-root".to_string(),
        "NotThere".to_string(),
        script.to_string_lossy().into_owned(),
    ]);

    assert_eq!(code, 1);
    assert!(stdout.contains("NotThere"));
    assert!(stdout.contains("does not exist"));
}

#[test]
fn doctor_reports_error_for_a_malformed_config_file() {
    let dir = tempfile::tempdir().expect("failed to create temp dir");
    let source_dir = dir.path().join("scripts/source");
    let script = source_dir.join("Example.psc");
    write_file(&script, "ScriptName Example\n");
    write_file(
        &dir.path().join("papyrus-lint.yaml"),
        "semicolon: [not, a, bool]\n",
    );

    let (code, stdout, _stderr) =
        run_captured(&["doctor".to_string(), script.to_string_lossy().into_owned()]);

    assert_eq!(code, 1);
    assert!(stdout.contains("[error] failed to load lint config"));
}

#[test]
fn doctor_reports_error_for_a_missing_explicit_config_path() {
    let dir = tempfile::tempdir().expect("failed to create temp dir");
    let source_dir = dir.path().join("scripts/source");
    let script = source_dir.join("Example.psc");
    write_file(&script, "ScriptName Example\n");
    let missing_config = dir.path().join("missing-config.yaml");

    let (code, stdout, _stderr) = run_captured(&[
        "doctor".to_string(),
        "--config".to_string(),
        missing_config.to_string_lossy().into_owned(),
        script.to_string_lossy().into_owned(),
    ]);

    assert_eq!(code, 1);
    assert!(stdout.contains("[error] failed to load lint config"));
    assert!(stdout.contains(&missing_config.display().to_string()));
}

#[test]
fn doctor_reports_error_for_a_configured_compiler_path_that_does_not_exist() {
    let dir = tempfile::tempdir().expect("failed to create temp dir");
    let source_dir = dir.path().join("scripts/source");
    let script = source_dir.join("Example.psc");
    write_file(&script, "ScriptName Example\n");
    write_file(
        &dir.path().join("papyrus-lint.yaml"),
        "compiler_path: /nonexistent/PapyrusCompiler.exe\n",
    );

    let (code, stdout, _stderr) =
        run_captured(&["doctor".to_string(), script.to_string_lossy().into_owned()]);

    assert_eq!(code, 1);
    assert!(stdout.contains("configured compiler_path"));
    assert!(stdout.contains("does not exist"));
}

#[test]
fn doctor_warns_when_compile_check_is_enabled_without_a_resolvable_compiler() {
    let dir = tempfile::tempdir().expect("failed to create temp dir");
    let source_dir = dir.path().join("scripts/source");
    let script = source_dir.join("Example.psc");
    write_file(&script, "ScriptName Example\n");
    write_file(
        &dir.path().join("papyrus-lint.yaml"),
        "compile_check: true\n",
    );

    let (code, stdout, _stderr) =
        run_captured(&["doctor".to_string(), script.to_string_lossy().into_owned()]);

    assert_eq!(code, 1);
    assert!(stdout.contains(
        "[warning] compile_check is enabled but no PapyrusCompiler.exe could be resolved"
    ));
}

#[test]
fn doctor_warns_when_a_scanned_directory_has_no_psc_files() {
    let dir = tempfile::tempdir().expect("failed to create temp dir");
    fs::create_dir_all(dir.path().join("empty")).expect("failed to create empty dir");

    let (code, stdout, _stderr) = run_captured(&[
        "doctor".to_string(),
        dir.path().join("empty").to_string_lossy().into_owned(),
    ]);

    assert_eq!(code, 1);
    assert!(stdout.contains("[warning] no .psc files found under"));
}

#[test]
fn doctor_json_reports_the_full_check_list_and_success_flag() {
    let dir = tempfile::tempdir().expect("failed to create temp dir");
    let source_dir = dir.path().join("scripts/source");
    let script = source_dir.join("Example.psc");
    write_file(&script, "ScriptName Example\n");

    let (code, stdout, stderr) = run_captured(&[
        "doctor".to_string(),
        "--format=json".to_string(),
        script.to_string_lossy().into_owned(),
    ]);

    assert_eq!(code, 0);
    assert!(stderr.is_empty());
    let report: serde_json::Value =
        serde_json::from_str(&stdout).expect("doctor --json output should be valid JSON");
    assert_eq!(report["success"], serde_json::Value::Bool(true));
    assert!(report["checks"]
        .as_array()
        .is_some_and(|checks| !checks.is_empty()));
    assert!(report["checks"]
        .as_array()
        .unwrap()
        .iter()
        .all(|check| check["status"] == "ok"));
}

#[test]
fn doctor_reports_a_malformed_achlist_and_continues_other_checks() {
    let dir = tempfile::tempdir().expect("failed to create temp dir");
    let achlist = dir.path().join("broken.achlist");
    write_file(&achlist, "not json");

    let (code, stdout, stderr) =
        run_captured(&["doctor".to_string(), achlist.to_string_lossy().into_owned()]);

    assert_eq!(code, 1);
    assert!(stderr.is_empty());
    assert!(stdout.contains("[error] failed to parse achlist"));
    assert!(stdout.contains("[ok] project root resolved to"));
    assert!(stdout.contains("PapyrusLinterCLI doctor:"));
}

#[test]
fn doctor_reports_a_healthy_ppj_project_and_its_imports() {
    // Uses the conventional `scripts/source` layout so this only exercises
    // ppj-specific behavior, leaving the (unrelated, pre-existing)
    // conventional-directory warning untouched.
    let dir = tempfile::tempdir().expect("failed to create temp dir");
    write_file(
        &dir.path().join("scripts/source/Example.psc"),
        "ScriptName Example\n",
    );
    write_file(
        &dir.path().join("Project.ppj"),
        r#"<PapyrusProject>
    <Imports>
        <Import>scripts/source</Import>
    </Imports>
    <Folders>
        <Folder>scripts/source</Folder>
    </Folders>
</PapyrusProject>"#,
    );
    let ppj_path = dir.path().join("Project.ppj");

    let (code, stdout, stderr) = run_captured(&[
        "doctor".to_string(),
        ppj_path.to_string_lossy().into_owned(),
    ]);

    assert_eq!(code, 0, "stderr: {stderr}, stdout: {stdout}");
    assert!(stdout.contains("every entry in"));
    assert!(stdout.contains(&format!(
        "[ok] additional script root {} exists",
        dir.path().join("scripts/source").display()
    )));
    assert!(stdout.contains("no problems found"));
}

#[test]
fn doctor_reports_each_missing_ppj_script_entry() {
    let dir = tempfile::tempdir().expect("failed to create temp dir");
    write_file(
        &dir.path().join("Project.ppj"),
        r#"<PapyrusProject>
    <Folders>
        <Folder>Missing</Folder>
    </Folders>
    <Scripts>
        <Script>Absent</Script>
    </Scripts>
</PapyrusProject>"#,
    );
    let ppj_path = dir.path().join("Project.ppj");

    let (code, stdout, _stderr) = run_captured(&[
        "doctor".to_string(),
        ppj_path.to_string_lossy().into_owned(),
    ]);

    assert_eq!(code, 1);
    assert!(stdout.contains(&format!(
        "[error] ppj entry {} does not exist",
        dir.path().join("Absent.psc").display()
    )));
}

#[test]
fn doctor_reports_a_malformed_ppj_and_continues_other_checks() {
    let dir = tempfile::tempdir().expect("failed to create temp dir");
    let ppj_path = dir.path().join("broken.ppj");
    write_file(&ppj_path, "not xml at all <<<");

    let (code, stdout, stderr) = run_captured(&[
        "doctor".to_string(),
        ppj_path.to_string_lossy().into_owned(),
    ]);

    assert_eq!(code, 1);
    assert!(stderr.is_empty());
    assert!(stdout.contains("[error] failed to parse ppj"));
    assert!(stdout.contains("[ok] project root resolved to"));
    assert!(stdout.contains("PapyrusLinterCLI doctor:"));
}

#[test]
fn doctor_rejects_missing_flag_values() {
    for flag in ["--config", "--script-root"] {
        let (code, stdout, stderr) = run_captured(&["doctor".to_string(), flag.to_string()]);

        assert_eq!(code, 2, "flag: {flag}");
        assert!(stdout.is_empty(), "flag: {flag}");
        assert!(stderr.contains("Usage: PapyrusLinterCLI"), "flag: {flag}");
    }
}

#[test]
fn doctor_rejects_extra_positional_arguments() {
    let (code, stdout, stderr) = run_captured(&[
        "doctor".to_string(),
        "first.psc".to_string(),
        "second.psc".to_string(),
    ]);

    assert_eq!(code, 2);
    assert!(stdout.is_empty());
    assert!(stderr.contains("Usage: PapyrusLinterCLI"));
}

#[test]
fn doctor_reports_positive_directory_and_explicit_path_checks() {
    let dir = tempfile::tempdir().expect("failed to create temp dir");
    let project = dir.path().join("project");
    let scripts = project.join("scripts/source");
    let external = dir.path().join("shared-scripts");
    let compiler = dir.path().join("PapyrusCompiler.exe");
    let config_path = dir.path().join("doctor-config.yaml");
    write_file(&scripts.join("Example.psc"), "ScriptName Example\n");
    fs::create_dir_all(&external).expect("failed to create external script root");
    write_file(&compiler, "compiler fixture");
    write_file(
        &project.join("papyrus-lint.yaml"),
        &format!("compiler_path: {}\n", compiler.display()),
    );
    write_file(&config_path, "trailing_whitespace: true\n");

    let (code, stdout, stderr) = run_captured(&[
        "doctor".to_string(),
        "--config".to_string(),
        config_path.to_string_lossy().into_owned(),
        "--script-root".to_string(),
        external.to_string_lossy().into_owned(),
        project.to_string_lossy().into_owned(),
    ]);

    assert_eq!(code, 0);
    assert!(stderr.is_empty());
    assert!(stdout.contains("found 1 .psc file(s) under"));
    assert!(stdout.contains("explicit config"));
    assert!(stdout.contains("additional script root"));
    assert!(stdout.contains("configured compiler_path"));
    assert!(stdout.contains("no problems found"));
}
