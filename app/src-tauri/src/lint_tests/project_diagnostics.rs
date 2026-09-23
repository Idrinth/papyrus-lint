use super::super::*;
use tempfile::tempdir;

#[test]
fn lint_psc_file_flags_a_script_newer_than_its_compiled_pex() {
    let dir = tempdir().unwrap();
    let source_dir = dir.path().join("Scripts/Source");
    std::fs::create_dir_all(&source_dir).unwrap();
    let path = source_dir.join("Example.psc");
    let pex_path = dir.path().join("Scripts/Example.pex");
    std::fs::write(&path, "ScriptName Example\n").unwrap();
    std::fs::write(&pex_path, "").unwrap();

    let now = std::time::SystemTime::now();
    std::fs::File::open(&pex_path)
        .unwrap()
        .set_modified(now - std::time::Duration::from_secs(60))
        .unwrap();
    std::fs::File::open(&path)
        .unwrap()
        .set_modified(now)
        .unwrap();

    let diagnostics = lint_psc_file(
        path.to_string_lossy().into_owned(),
        ProjectLintContext {
            root: dir.path().to_string_lossy().into_owned(),
            ..Default::default()
        },
    )
    .unwrap();

    assert!(diagnostics
        .iter()
        .any(|diagnostic| diagnostic.rule == stale_pex::RULE
            && diagnostic.message.starts_with("[info]")));
}

#[test]
fn lint_psc_file_ignores_stale_compiled_output_when_disabled() {
    let dir = tempdir().unwrap();
    let source_dir = dir.path().join("Scripts/Source");
    std::fs::create_dir_all(&source_dir).unwrap();
    let path = source_dir.join("Example.psc");
    let pex_path = dir.path().join("Scripts/Example.pex");
    std::fs::write(&path, "ScriptName Example\n").unwrap();
    std::fs::write(&pex_path, "").unwrap();

    let now = std::time::SystemTime::now();
    std::fs::File::open(&pex_path)
        .unwrap()
        .set_modified(now - std::time::Duration::from_secs(60))
        .unwrap();
    std::fs::File::open(&path)
        .unwrap()
        .set_modified(now)
        .unwrap();

    let config = papyrus_lints::Config {
        rules: papyrus_lints::config::Rules {
            stale_compiled_output: false,
            ..Default::default()
        },
        ..Default::default()
    };
    let diagnostics = lint_psc_file(
        path.to_string_lossy().into_owned(),
        ProjectLintContext {
            root: dir.path().to_string_lossy().into_owned(),
            config,
            ..Default::default()
        },
    )
    .unwrap();

    assert!(diagnostics
        .iter()
        .all(|diagnostic| diagnostic.rule != stale_pex::RULE));
}

#[test]
fn lint_psc_file_reports_script_filename_mismatch() {
    let dir = tempdir().unwrap();
    let source_dir = dir.path().join("Scripts/Source");
    std::fs::create_dir_all(&source_dir).unwrap();
    let path = source_dir.join("Other.psc");
    std::fs::write(&path, "ScriptName Example\n").unwrap();

    let diagnostics = lint_psc_file(
        path.to_string_lossy().into_owned(),
        ProjectLintContext {
            root: dir.path().to_string_lossy().into_owned(),
            ..Default::default()
        },
    )
    .unwrap();

    assert!(diagnostics.iter().any(
        |diagnostic| diagnostic.rule == script_filename_mismatch::RULE
            && diagnostic.message.starts_with("[error]")
    ));
}

#[test]
fn lint_psc_file_ignores_script_filename_mismatch_when_disabled() {
    let dir = tempdir().unwrap();
    let source_dir = dir.path().join("Scripts/Source");
    std::fs::create_dir_all(&source_dir).unwrap();
    let path = source_dir.join("Other.psc");
    std::fs::write(&path, "ScriptName Example\n").unwrap();

    let config = papyrus_lints::Config {
        rules: papyrus_lints::config::Rules {
            script_filename_mismatch: false,
            ..Default::default()
        },
        ..Default::default()
    };
    let diagnostics = lint_psc_file(
        path.to_string_lossy().into_owned(),
        ProjectLintContext {
            root: dir.path().to_string_lossy().into_owned(),
            config,
            ..Default::default()
        },
    )
    .unwrap();

    assert!(diagnostics
        .iter()
        .all(|diagnostic| diagnostic.rule != script_filename_mismatch::RULE));
}

#[test]
fn lint_psc_file_honors_a_disable_comment_for_script_filename_mismatch() {
    let dir = tempdir().unwrap();
    let source_dir = dir.path().join("Scripts/Source");
    std::fs::create_dir_all(&source_dir).unwrap();
    let path = source_dir.join("Other.psc");
    std::fs::write(
        &path,
        "ScriptName Example ; @disable script-filename-mismatch\n",
    )
    .unwrap();

    let diagnostics = lint_psc_file(
        path.to_string_lossy().into_owned(),
        ProjectLintContext {
            root: dir.path().to_string_lossy().into_owned(),
            ..Default::default()
        },
    )
    .unwrap();

    assert!(diagnostics
        .iter()
        .all(|diagnostic| diagnostic.rule != script_filename_mismatch::RULE));
}

#[test]
fn lint_psc_file_honors_a_disable_file_comment_for_script_filename_mismatch() {
    let dir = tempdir().unwrap();
    let source_dir = dir.path().join("Scripts/Source");
    std::fs::create_dir_all(&source_dir).unwrap();
    let path = source_dir.join("Other.psc");
    std::fs::write(
        &path,
        "ScriptName Example\n; @disable-file script-filename-mismatch\n",
    )
    .unwrap();

    let diagnostics = lint_psc_file(
        path.to_string_lossy().into_owned(),
        ProjectLintContext {
            root: dir.path().to_string_lossy().into_owned(),
            ..Default::default()
        },
    )
    .unwrap();

    assert!(diagnostics
        .iter()
        .all(|diagnostic| diagnostic.rule != script_filename_mismatch::RULE));
}

#[test]
fn lint_psc_file_reports_conflicting_script_versions() {
    let dir = tempdir().unwrap();
    let first_root = dir.path().join("scripts/source");
    let second_root = dir.path().join("source/scripts");
    std::fs::create_dir_all(&first_root).unwrap();
    std::fs::create_dir_all(&second_root).unwrap();
    let path = first_root.join("Example.psc");
    std::fs::write(&path, "ScriptName Example\n").unwrap();
    std::fs::write(
        second_root.join("Example.psc"),
        "ScriptName Example\n; a different version\n",
    )
    .unwrap();

    let diagnostics = lint_psc_file(
        path.to_string_lossy().into_owned(),
        ProjectLintContext {
            root: dir.path().to_string_lossy().into_owned(),
            ..Default::default()
        },
    )
    .unwrap();

    assert!(diagnostics
        .iter()
        .any(|diagnostic| { diagnostic.rule == script_locator::CONFLICTING_SCRIPT_VERSIONS_RULE }));
}

#[test]
fn lint_psc_file_ignores_conflicting_script_versions_when_disabled() {
    let dir = tempdir().unwrap();
    let first_root = dir.path().join("scripts/source");
    let second_root = dir.path().join("source/scripts");
    std::fs::create_dir_all(&first_root).unwrap();
    std::fs::create_dir_all(&second_root).unwrap();
    let path = first_root.join("Example.psc");
    std::fs::write(&path, "ScriptName Example\n").unwrap();
    std::fs::write(
        second_root.join("Example.psc"),
        "ScriptName Example\n; a different version\n",
    )
    .unwrap();
    let config = papyrus_lints::Config {
        rules: papyrus_lints::config::Rules {
            conflicting_script_versions: false,
            ..Default::default()
        },
        ..Default::default()
    };

    let diagnostics = lint_psc_file(
        path.to_string_lossy().into_owned(),
        ProjectLintContext {
            root: dir.path().to_string_lossy().into_owned(),
            config,
            ..Default::default()
        },
    )
    .unwrap();

    assert!(diagnostics
        .iter()
        .all(|diagnostic| { diagnostic.rule != script_locator::CONFLICTING_SCRIPT_VERSIONS_RULE }));
}

#[test]
fn lint_psc_file_reports_conflicting_script_versions_in_an_additional_root() {
    let dir = tempdir().unwrap();
    let source_dir = dir.path().join("scripts/source");
    std::fs::create_dir_all(&source_dir).unwrap();
    let path = source_dir.join("Example.psc");
    std::fs::write(&path, "ScriptName Example\n").unwrap();
    let extra = tempdir().unwrap();
    std::fs::write(
        extra.path().join("Example.psc"),
        "ScriptName Example\n; a different version\n",
    )
    .unwrap();

    let diagnostics = lint_psc_file(
        path.to_string_lossy().into_owned(),
        ProjectLintContext {
            root: dir.path().to_string_lossy().into_owned(),
            additional_roots: vec![extra.path().to_string_lossy().into_owned()],
            ..Default::default()
        },
    )
    .unwrap();

    assert!(diagnostics
        .iter()
        .any(|diagnostic| { diagnostic.rule == script_locator::CONFLICTING_SCRIPT_VERSIONS_RULE }));
}
