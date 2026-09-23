use super::super::*;
use tempfile::tempdir;

#[test]
fn lint_psc_file_stale_compiled_output_disable_comment_is_not_reported_as_unused() {
    let dir = tempdir().unwrap();
    let source_dir = dir.path().join("Scripts/Source");
    std::fs::create_dir_all(&source_dir).unwrap();
    let path = source_dir.join("Example.psc");
    let pex_path = dir.path().join("Scripts/Example.pex");
    std::fs::write(
        &path,
        "ScriptName Example ; @disable stale-compiled-output\n",
    )
    .unwrap();
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
            unused_disable: true,
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
    assert!(diagnostics
        .iter()
        .all(|diagnostic| diagnostic.rule != "unused-disable"));
}

#[test]
fn lint_psc_file_stale_compiled_output_disable_file_comment_is_not_reported_as_unused() {
    let dir = tempdir().unwrap();
    let source_dir = dir.path().join("Scripts/Source");
    std::fs::create_dir_all(&source_dir).unwrap();
    let path = source_dir.join("Example.psc");
    let pex_path = dir.path().join("Scripts/Example.pex");
    std::fs::write(
        &path,
        "ScriptName Example\n; @disable-file stale-compiled-output\n",
    )
    .unwrap();
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
            unused_disable: true,
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
    assert!(diagnostics
        .iter()
        .all(|diagnostic| diagnostic.rule != "unused-disable"));
}

#[test]
fn lint_psc_file_conflicting_script_versions_disable_comment_is_not_reported_as_unused() {
    let dir = tempdir().unwrap();
    let first_root = dir.path().join("scripts/source");
    let second_root = dir.path().join("source/scripts");
    std::fs::create_dir_all(&first_root).unwrap();
    std::fs::create_dir_all(&second_root).unwrap();
    let path = first_root.join("Example.psc");
    std::fs::write(
        &path,
        "ScriptName Example ; @disable conflicting-script-versions\n",
    )
    .unwrap();
    std::fs::write(
        second_root.join("Example.psc"),
        "ScriptName Example\n; a different version\n",
    )
    .unwrap();

    let config = papyrus_lints::Config {
        rules: papyrus_lints::config::Rules {
            unused_disable: true,
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
    assert!(diagnostics
        .iter()
        .all(|diagnostic| diagnostic.rule != "unused-disable"));
}

#[test]
fn lint_psc_file_conflicting_script_versions_disable_file_comment_is_not_reported_as_unused() {
    let dir = tempdir().unwrap();
    let first_root = dir.path().join("scripts/source");
    let second_root = dir.path().join("source/scripts");
    std::fs::create_dir_all(&first_root).unwrap();
    std::fs::create_dir_all(&second_root).unwrap();
    let path = first_root.join("Example.psc");
    std::fs::write(
        &path,
        "ScriptName Example\n; @disable-file conflicting-script-versions\n",
    )
    .unwrap();
    std::fs::write(
        second_root.join("Example.psc"),
        "ScriptName Example\n; a different version\n",
    )
    .unwrap();

    let config = papyrus_lints::Config {
        rules: papyrus_lints::config::Rules {
            unused_disable: true,
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
    assert!(diagnostics
        .iter()
        .all(|diagnostic| diagnostic.rule != "unused-disable"));
}

#[test]
fn lint_psc_file_script_filename_mismatch_disable_comment_is_not_reported_as_unused() {
    let dir = tempdir().unwrap();
    let source_dir = dir.path().join("Scripts/Source");
    std::fs::create_dir_all(&source_dir).unwrap();
    let path = source_dir.join("Other.psc");
    std::fs::write(
        &path,
        "ScriptName Example ; @disable script-filename-mismatch\n",
    )
    .unwrap();

    let config = papyrus_lints::Config {
        rules: papyrus_lints::config::Rules {
            unused_disable: true,
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
    assert!(diagnostics
        .iter()
        .all(|diagnostic| diagnostic.rule != "unused-disable"));
}

#[test]
fn lint_psc_file_script_filename_mismatch_disable_file_comment_is_not_reported_as_unused() {
    let dir = tempdir().unwrap();
    let source_dir = dir.path().join("Scripts/Source");
    std::fs::create_dir_all(&source_dir).unwrap();
    let path = source_dir.join("Other.psc");
    std::fs::write(
        &path,
        "ScriptName Example\n; @disable-file script-filename-mismatch\n",
    )
    .unwrap();

    let config = papyrus_lints::Config {
        rules: papyrus_lints::config::Rules {
            unused_disable: true,
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
    assert!(diagnostics
        .iter()
        .all(|diagnostic| diagnostic.rule != "unused-disable"));
}
