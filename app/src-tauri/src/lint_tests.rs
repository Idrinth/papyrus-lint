use super::*;
use tempfile::tempdir;

#[test]
fn compile_psc_file_rejects_a_blank_compiler_path_before_spawning() {
    assert!(compile_psc_file("Example.psc".to_string(), "  \t".to_string(), Vec::new()).is_err());
}

#[test]
fn compile_psc_file_reports_a_spawn_error_from_the_compiler_module() {
    let dir = tempdir().unwrap();
    let source_dir = dir.path().join("Scripts/Source");
    std::fs::create_dir_all(&source_dir).unwrap();
    let script_path = source_dir.join("Example.psc");
    std::fs::write(&script_path, "ScriptName Example\n").unwrap();

    let error = compile_psc_file(
        script_path.to_string_lossy().into_owned(),
        dir.path()
            .join("missing-compiler")
            .to_string_lossy()
            .into_owned(),
        Vec::new(),
    )
    .unwrap_err();

    assert!(error.contains("failed to run"));
}

#[test]
fn lint_psc_file_lints_source_from_disk() {
    let dir = tempdir().unwrap();
    let path = dir.path().join("Example.psc");
    std::fs::write(
        &path,
        "ScriptName Example\n\nFunction Run()\n    Game.GetPlayer()\nEndFunction\n",
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
        .any(|diagnostic| diagnostic.rule == "forbidden-functions"));
}

#[test]
fn lint_psc_file_ignores_compile_check_when_disabled() {
    let dir = tempdir().unwrap();
    let path = dir.path().join("Example.psc");
    std::fs::write(&path, "ScriptName Example\n").unwrap();

    let diagnostics = lint_psc_file(
        path.to_string_lossy().into_owned(),
        ProjectLintContext {
            root: dir.path().to_string_lossy().into_owned(),
            compiler_path: "/does/not/matter".to_string(),
            ..Default::default()
        },
    )
    .unwrap();

    assert!(diagnostics.is_empty());
}

#[test]
fn lint_psc_file_ignores_compile_check_when_no_compiler_path_is_set() {
    let dir = tempdir().unwrap();
    let path = dir.path().join("Example.psc");
    std::fs::write(&path, "ScriptName Example\n").unwrap();

    let diagnostics = lint_psc_file(
        path.to_string_lossy().into_owned(),
        ProjectLintContext {
            root: dir.path().to_string_lossy().into_owned(),
            compiler_path: "   ".to_string(),
            compile_check: true,
            ..Default::default()
        },
    )
    .unwrap();

    assert!(diagnostics.is_empty());
}

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

#[test]
#[cfg(unix)]
fn lint_psc_file_merges_in_compiler_reported_errors_when_enabled() {
    use std::os::unix::fs::PermissionsExt;

    let dir = tempdir().unwrap();
    let source_dir = dir.path().join("Scripts/Source");
    std::fs::create_dir_all(&source_dir).unwrap();
    let path = source_dir.join("Example.psc");
    std::fs::write(&path, "ScriptName Example\n").unwrap();
    let compiler_path = dir.path().join("compiler.sh");
    std::fs::write(
            &compiler_path,
            "#!/bin/sh\necho \"Example.psc(3,4): no viable alternative at character ';'\" >&2\nexit 1\n",
        )
        .unwrap();
    std::fs::set_permissions(&compiler_path, std::fs::Permissions::from_mode(0o755)).unwrap();

    let diagnostics = lint_psc_file(
        path.to_string_lossy().into_owned(),
        ProjectLintContext {
            root: dir.path().to_string_lossy().into_owned(),
            compiler_path: compiler_path.to_string_lossy().into_owned(),
            compile_check: true,
            ..Default::default()
        },
    )
    .unwrap();

    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic.rule == compile_diagnostics::RULE
            && diagnostic.line == 3
            && diagnostic.column == 4
    }));
}

#[test]
#[cfg(unix)]
fn lint_psc_file_omits_compiler_diagnostics_when_the_compiler_reports_success() {
    use std::os::unix::fs::PermissionsExt;

    let dir = tempdir().unwrap();
    let source_dir = dir.path().join("Scripts/Source");
    std::fs::create_dir_all(&source_dir).unwrap();
    let path = source_dir.join("Example.psc");
    std::fs::write(&path, "ScriptName Example\n").unwrap();
    let compiler_path = dir.path().join("compiler.sh");
    std::fs::write(&compiler_path, "#!/bin/sh\nexit 0\n").unwrap();
    std::fs::set_permissions(&compiler_path, std::fs::Permissions::from_mode(0o755)).unwrap();

    let diagnostics = lint_psc_file(
        path.to_string_lossy().into_owned(),
        ProjectLintContext {
            root: dir.path().to_string_lossy().into_owned(),
            compiler_path: compiler_path.to_string_lossy().into_owned(),
            compile_check: true,
            ..Default::default()
        },
    )
    .unwrap();

    assert!(diagnostics
        .iter()
        .all(|d| d.rule != compile_diagnostics::RULE));
}

#[test]
fn lint_psc_file_does_not_fail_when_the_configured_compiler_cannot_be_run() {
    let dir = tempdir().unwrap();
    let path = dir.path().join("Example.psc");
    std::fs::write(&path, "ScriptName Example\n").unwrap();

    let diagnostics = lint_psc_file(
        path.to_string_lossy().into_owned(),
        ProjectLintContext {
            root: dir.path().to_string_lossy().into_owned(),
            compiler_path: dir
                .path()
                .join("missing-compiler")
                .to_string_lossy()
                .into_owned(),
            compile_check: true,
            ..Default::default()
        },
    )
    .unwrap();

    assert!(diagnostics.is_empty());
}

#[test]
fn list_script_members_reports_functions_and_properties_including_inherited_ones() {
    let dir = tempdir().unwrap();
    let source_dir = dir.path().join("scripts/source");
    std::fs::create_dir_all(&source_dir).unwrap();
    std::fs::write(
        source_dir.join("Base.psc"),
        "ScriptName Base\n\nBool Property IsAwesome Auto\n",
    )
    .unwrap();
    std::fs::write(
        source_dir.join("Child.psc"),
        "ScriptName Child Extends Base\n\nInt Function DoThing(Float a)\nEndFunction\n",
    )
    .unwrap();

    let members = list_script_members(
        dir.path().to_string_lossy().into_owned(),
        "Child".to_string(),
        Vec::new(),
        Vec::new(),
    );

    let names: std::collections::HashSet<_> =
        members.iter().map(function_table::Member::name).collect();
    assert_eq!(
        names,
        std::collections::HashSet::from(["DoThing", "IsAwesome"])
    );
}

#[test]
fn list_script_members_resolves_a_type_via_an_additional_script_root() {
    let dir = tempdir().unwrap();
    let shared = tempdir().unwrap();
    std::fs::write(
        shared.path().join("Shared.psc"),
        "ScriptName Shared\n\nInt Property MyValue Auto\n",
    )
    .unwrap();

    let members = list_script_members(
        dir.path().to_string_lossy().into_owned(),
        "Shared".to_string(),
        vec![shared.path().to_string_lossy().into_owned()],
        Vec::new(),
    );

    assert_eq!(members.len(), 1);
}

#[test]
fn list_script_members_resolves_a_type_via_a_lookup_script_root() {
    let dir = tempdir().unwrap();
    let vanilla = tempdir().unwrap();
    std::fs::write(
        vanilla.path().join("Shared.psc"),
        "ScriptName Shared\n\nInt Property MyValue Auto\n",
    )
    .unwrap();

    let members = list_script_members(
        dir.path().to_string_lossy().into_owned(),
        "Shared".to_string(),
        Vec::new(),
        vec![vanilla.path().to_string_lossy().into_owned()],
    );

    assert_eq!(members.len(), 1);
    assert_eq!(members[0].name(), "MyValue");
}

#[test]
fn list_script_members_is_empty_for_an_unresolvable_type() {
    let dir = tempdir().unwrap();

    assert!(list_script_members(
        dir.path().to_string_lossy().into_owned(),
        "Missing".to_string(),
        Vec::new(),
        Vec::new(),
    )
    .is_empty());
}

#[test]
fn list_script_members_matches_type_names_case_insensitively() {
    let dir = tempdir().unwrap();
    let source_dir = dir.path().join("scripts/source");
    std::fs::create_dir_all(&source_dir).unwrap();
    std::fs::write(
        source_dir.join("Example.psc"),
        "ScriptName Example\n\nString Property DisplayName Auto\n",
    )
    .unwrap();

    let members = list_script_members(
        dir.path().to_string_lossy().into_owned(),
        "eXaMpLe".to_string(),
        Vec::new(),
        Vec::new(),
    );

    assert_eq!(members.len(), 1);
    assert_eq!(members[0].name(), "DisplayName");
}

#[test]
#[cfg(unix)]
fn compile_command_trims_the_executable_path_and_returns_its_output() {
    use std::os::unix::fs::symlink;

    let dir = tempdir().unwrap();
    let source_dir = dir.path().join("Scripts/Source");
    std::fs::create_dir_all(&source_dir).unwrap();
    let script_path = source_dir.join("Example.psc");
    std::fs::write(&script_path, "echo command wrapper\n").unwrap();
    let compiler_path = dir.path().join("compiler.sh");
    symlink("/bin/sh", &compiler_path).unwrap();

    let outcome = compile_psc_file(
        script_path.to_string_lossy().into_owned(),
        format!("  {}  ", compiler_path.display()),
        Vec::new(),
    )
    .unwrap();

    assert!(outcome.success);
    assert_eq!(outcome.stdout, "command wrapper\n");
}

#[test]
#[cfg(unix)]
fn compile_command_returns_a_failed_compiler_outcome() {
    use std::os::unix::fs::PermissionsExt;

    let dir = tempdir().unwrap();
    let source_dir = dir.path().join("Scripts/Source");
    std::fs::create_dir_all(&source_dir).unwrap();
    let script_path = source_dir.join("Example.psc");
    std::fs::write(&script_path, "ScriptName Example\n").unwrap();
    let compiler_path = dir.path().join("compiler.sh");
    std::fs::write(
        &compiler_path,
        "#!/bin/sh\necho compile failed >&2\nexit 1\n",
    )
    .unwrap();
    std::fs::set_permissions(&compiler_path, std::fs::Permissions::from_mode(0o755)).unwrap();

    let outcome = compile_psc_file(
        script_path.to_string_lossy().into_owned(),
        compiler_path.to_string_lossy().into_owned(),
        Vec::new(),
    )
    .unwrap();

    assert!(!outcome.success);
    assert_eq!(outcome.stderr, "compile failed\n");
}

#[test]
#[cfg(unix)]
fn compile_command_forwards_additional_script_roots() {
    use std::os::unix::fs::PermissionsExt;

    let dir = tempdir().unwrap();
    let source_dir = dir.path().join("Scripts/Source");
    let additional_root = dir.path().join("Shared Scripts");
    std::fs::create_dir_all(&source_dir).unwrap();
    std::fs::create_dir_all(&additional_root).unwrap();
    let script_path = source_dir.join("Example.psc");
    std::fs::write(&script_path, "ScriptName Example\n").unwrap();
    let compiler_path = dir.path().join("compiler.sh");
    std::fs::write(&compiler_path, "#!/bin/sh\nprintf '%s\\n' \"$@\"\n").unwrap();
    std::fs::set_permissions(&compiler_path, std::fs::Permissions::from_mode(0o755)).unwrap();

    let outcome = compile_psc_file(
        script_path.to_string_lossy().into_owned(),
        compiler_path.to_string_lossy().into_owned(),
        vec![additional_root.to_string_lossy().into_owned()],
    )
    .unwrap();

    assert!(outcome.success);
    assert!(outcome
        .stdout
        .contains(&additional_root.to_string_lossy().into_owned()));
    assert!(outcome.stdout.contains("Example.psc"));
}

#[test]
fn lint_psc_file_resolves_argument_types_through_additional_script_roots() {
    let extra = tempdir().unwrap();
    std::fs::write(
        extra.path().join("Helpers.psc"),
        "ScriptName Helpers\n\nFunction NeedInt(Int count)\nEndFunction\n",
    )
    .unwrap();
    let dir = tempdir().unwrap();
    let path = dir.path().join("Caller.psc");
    std::fs::write(
            &path,
            "ScriptName Caller\n\nFunction Run(Helpers helper)\n    helper.NeedInt(\"nope\")\nEndFunction\n",
        )
        .unwrap();
    let extra_root = extra.path().to_string_lossy().into_owned();

    let without_roots = lint_psc_file(
        path.to_string_lossy().into_owned(),
        ProjectLintContext {
            root: dir.path().to_string_lossy().into_owned(),
            ..Default::default()
        },
    )
    .unwrap();
    assert!(without_roots
        .iter()
        .all(|diagnostic| diagnostic.rule != "argument-types"));

    let with_roots = lint_psc_file(
        path.to_string_lossy().into_owned(),
        ProjectLintContext {
            root: dir.path().to_string_lossy().into_owned(),
            additional_roots: vec![extra_root],
            ..Default::default()
        },
    )
    .unwrap();
    assert!(with_roots.iter().any(|diagnostic| {
        diagnostic.rule == "argument-types"
            && diagnostic.message.contains("expects Int")
            && diagnostic.message.contains("got String")
    }));
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

#[test]
fn project_function_table_reuses_one_table_for_the_same_roots() {
    let dir = tempdir().unwrap();
    let root = dir.path().to_string_lossy().into_owned();
    let additional = vec!["/extra".to_string()];
    let lookup = vec!["/vanilla".to_string()];

    let first = project_function_table(root.clone(), additional.clone(), lookup.clone());
    let second = project_function_table(root, additional, lookup);

    assert!(std::sync::Arc::ptr_eq(&first, &second));
}

#[test]
fn project_function_table_is_distinct_for_different_roots() {
    let first_dir = tempdir().unwrap();
    let second_dir = tempdir().unwrap();

    let first = project_function_table(
        first_dir.path().to_string_lossy().into_owned(),
        Vec::new(),
        Vec::new(),
    );
    let second = project_function_table(
        second_dir.path().to_string_lossy().into_owned(),
        Vec::new(),
        Vec::new(),
    );

    assert!(!std::sync::Arc::ptr_eq(&first, &second));
}
