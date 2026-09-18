use super::*;
use crate::test_support::*;

#[test]
fn short_paths_strips_the_project_root_from_reported_paths() {
    let dir = tempfile::tempdir().expect("failed to create temp dir");
    write_file(
        &dir.path().join("scripts/source/Example.psc"),
        "ScriptName Example   \n",
    );
    write_file(
        &dir.path().join("sources.achlist"),
        r#"["scripts/source/Example.psc"]"#,
    );
    let achlist_path = dir.path().join("sources.achlist");

    let (code, stdout, _stderr) = run_captured(&[
        "--short-paths".to_string(),
        achlist_path.to_string_lossy().into_owned(),
    ]);

    assert_eq!(code, 0);
    assert!(stdout.contains("scripts/source/Example.psc:"));
    assert!(stdout.contains("[trailing-whitespace]"));
    assert!(!stdout.contains(dir.path().to_string_lossy().as_ref()));
}

#[test]
fn short_paths_strips_the_project_root_from_json_paths() {
    let dir = tempfile::tempdir().expect("failed to create temp dir");
    write_file(
        &dir.path().join("scripts/source/Example.psc"),
        "ScriptName Example   \n",
    );
    write_file(
        &dir.path().join("sources.achlist"),
        r#"["scripts/source/Example.psc"]"#,
    );
    let achlist_path = dir.path().join("sources.achlist");

    let (code, stdout, _stderr) = run_captured(&[
        "--json".to_string(),
        "--short-paths".to_string(),
        achlist_path.to_string_lossy().into_owned(),
    ]);

    assert_eq!(code, 0);
    let report: serde_json::Value = serde_json::from_str(&stdout).unwrap();
    assert_eq!(
        report["files"][0]["path"],
        "scripts/source/Example.psc".replace('/', std::path::MAIN_SEPARATOR_STR)
    );
}

#[test]
fn without_short_paths_reports_the_full_path() {
    let dir = tempfile::tempdir().expect("failed to create temp dir");
    write_file(
        &dir.path().join("scripts/source/Example.psc"),
        "ScriptName Example   \n",
    );
    write_file(
        &dir.path().join("sources.achlist"),
        r#"["scripts/source/Example.psc"]"#,
    );
    let achlist_path = dir.path().join("sources.achlist");

    let (code, stdout, _stderr) = run_captured(&[achlist_path.to_string_lossy().into_owned()]);

    assert_eq!(code, 0);
    assert!(stdout.contains(dir.path().to_string_lossy().as_ref()));
}

#[test]
fn directory_scan_finds_the_project_root_from_a_nested_scripts_source_pair() {
    let dir = tempfile::tempdir().expect("failed to create temp dir");
    write_file(
        &dir.path().join("scripts/source/Requiem/Nested.psc"),
        "ScriptName Nested   \n",
    );
    write_file(
        &dir.path().join("papyrus-lint.yaml"),
        "rules:\n  trailing_whitespace: false\n",
    );
    let target = dir.path().join("scripts/source");

    let (code, stdout, _stderr) = run_captured(&[target.to_string_lossy().into_owned()]);

    assert_eq!(code, 0);
    assert!(stdout.contains("no problems found"));
}

#[test]
fn directory_scan_falls_back_to_the_scanned_directory_as_project_root() {
    // No scripts/source or source/scripts pair anywhere in the path, so
    // the scanned directory itself must be used as the project root.
    let dir = tempfile::tempdir().expect("failed to create temp dir");
    write_file(
        &dir.path().join("Nested/Example.psc"),
        "ScriptName Example   \n",
    );
    write_file(
        &dir.path().join("papyrus-lint.yaml"),
        "rules:\n  trailing_whitespace: false\n",
    );

    let (code, stdout, _stderr) = run_captured(&[dir.path().to_string_lossy().into_owned()]);

    assert_eq!(code, 0);
    assert!(stdout.contains("no problems found"));
}

#[test]
fn honors_the_project_yaml_config_two_directories_above_a_single_psc_file() {
    // Mirrors the real layout a bare .psc file is found at (e.g. an
    // editor plugin invoking the CLI on a saved file), where the
    // project root sits two directories above the script, at
    // `<root>/scripts/source/Example.psc`.
    let dir = tempfile::tempdir().expect("failed to create temp dir");
    let script_path = dir.path().join("scripts/source/Example.psc");
    write_file(&script_path, "ScriptName Example   \n");
    write_file(
        &dir.path().join("papyrus-lint.yaml"),
        "rules:\n  trailing_whitespace: false\n",
    );

    let (code, stdout, _stderr) = run_captured(&[script_path.to_string_lossy().into_owned()]);

    assert_eq!(code, 0);
    assert!(stdout.contains("no problems found"));
}

#[test]
fn finds_the_project_root_for_a_psc_nested_under_a_namespaced_subfolder() {
    // A Fallout 4-style namespaced script, e.g. `ScriptName User:MyScript`
    // stored at `Scripts/Source/User/MyScript.psc`, sits three
    // directories under the project root rather than the conventional
    // two. A naive "two directories up" rule would land on `Scripts`
    // instead of the real root, missing the project's config and
    // breaking every cross-script lookup — this must still find the
    // real root and pick up the config there.
    let dir = tempfile::tempdir().expect("failed to create temp dir");
    let script_path = dir.path().join("scripts/source/User/MyScript.psc");
    write_file(&script_path, "ScriptName User:MyScript   \n");
    write_file(
        &dir.path().join("papyrus-lint.yaml"),
        "rules:\n  trailing_whitespace: false\n",
    );

    let (code, stdout, _stderr) = run_captured(&[script_path.to_string_lossy().into_owned()]);

    assert_eq!(code, 0);
    assert!(stdout.contains("no problems found"));
}

#[test]
fn finds_the_project_root_for_a_bare_psc_with_no_scripts_source_pair() {
    // A project laid out without a conventional scripts/source pair at
    // all (e.g. Requiem's own, arbitrarily nested layout), linted via a
    // single .psc file directly (e.g. an editor plugin invoking the CLI
    // on save). The old fixed "two directories up" guess would land
    // outside the project entirely here and silently ignore its config;
    // this must instead find the config file at the project's real root.
    let dir = tempfile::tempdir().expect("failed to create temp dir");
    let script_path = dir.path().join("MyMod/scripts/subsystem/Example.psc");
    write_file(&script_path, "ScriptName Example   \n");
    write_file(
        &dir.path().join("MyMod/papyrus-lint.yaml"),
        "rules:\n  trailing_whitespace: false\n",
    );

    let (code, stdout, _stderr) = run_captured(&[script_path.to_string_lossy().into_owned()]);

    assert_eq!(code, 0);
    assert!(stdout.contains("no problems found"));
}

#[test]
fn finds_the_project_root_from_script_position_when_the_achlist_lives_elsewhere() {
    // Users sometimes drop the .achlist somewhere other than the
    // project root (e.g. next to a game's Data directory) while the
    // actual project, including its papyrus-lint.yaml, lives deeper:
    //
    //   achlist
    //   somefolder/
    //     otherfolder/
    //       papyrus-lint.yaml
    //       scripts/source/AType.psc
    //       source/scripts/BType.psc
    //
    // The achlist's own parent directory (the top-level one) has no
    // config file at all, so the project root must instead be found
    // from the resolved scripts' own position under their
    // scripts/source or source/scripts pair.
    let dir = tempfile::tempdir().expect("failed to create temp dir");
    let project_root = dir.path().join("somefolder/otherfolder");
    write_file(
        &project_root.join("scripts/source/AType.psc"),
        "ScriptName AType   \n",
    );
    write_file(
        &project_root.join("source/scripts/BType.psc"),
        "ScriptName BType   \n",
    );
    write_file(
        &project_root.join("papyrus-lint.yaml"),
        "rules:\n  trailing_whitespace: false\n",
    );
    write_file(
        &dir.path().join("achlist"),
        r#"["somefolder/otherfolder/scripts/source/AType.psc", "somefolder/otherfolder/source/scripts/BType.psc"]"#,
    );
    let achlist_path = dir.path().join("achlist");

    let (code, stdout, _stderr) = run_captured(&[achlist_path.to_string_lossy().into_owned()]);

    // If the project root were (wrongly) taken as the achlist's own
    // parent directory, papyrus-lint.yaml wouldn't be found and the
    // trailing-whitespace lint (disabled by that config) would fire on
    // both scripts instead.
    assert_eq!(code, 0);
    assert!(stdout.contains("no problems found in 2 script"));
}

#[test]
fn is_psc_path_matches_psc_extensions_case_insensitively() {
    use std::path::Path;
    assert!(is_psc_path(Path::new("Example.psc")));
    assert!(is_psc_path(Path::new("Example.PSC")));
    assert!(!is_psc_path(Path::new("sources.achlist")));
    assert!(!is_psc_path(Path::new("scripts")));
}
