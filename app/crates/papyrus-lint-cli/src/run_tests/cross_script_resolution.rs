//! Cross-script type/function resolution, including `strict_achlist_scope`.

use crate::test_support::*;

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
    assert!(stdout.contains("[argument-types]"));
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
        stdout.contains("[non-global-function-call]"),
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
    assert!(stdout.contains("[argument-types]"));
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
    assert!(stdout.contains("[goto-state]"));
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
    assert!(!stdout.contains("[unresolved-script]"), "stdout: {stdout}");
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
    assert!(stdout.contains("[unresolved-script]"), "stdout: {stdout}");
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
    assert!(stdout.contains("[unresolved-script]"), "stdout: {stdout}");
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
        stdout.matches("[conflicting-script-versions]").count(),
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
        stdout.matches("[conflicting-script-versions]").count(),
        2,
        "stdout: {stdout}"
    );
}
