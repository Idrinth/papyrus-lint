use super::*;
use crate::lint::ProjectLintContext;
use tempfile::tempdir;

#[cfg(unix)]
use papyrus_lint_core::compile_diagnostics;

#[test]
fn repair_psc_file_persists_fixes_and_returns_only_remaining_findings() {
    let dir = tempdir().unwrap();
    let path = dir.path().join("Example.psc");
    std::fs::write(
        &path,
        "ScriptName Example  \n\nFunction Run()\n\tGame.GetPlayer()\nEndFunction\n",
    )
    .unwrap();

    let diagnostics = repair_psc_file(
        path.to_string_lossy().into_owned(),
        ProjectLintContext {
            root: dir.path().to_string_lossy().into_owned(),
            ..Default::default()
        },
    )
    .unwrap();

    assert_eq!(
        std::fs::read_to_string(&path).unwrap(),
        "ScriptName Example\n\nFunction Run()\n\tGame.GetPlayer()\nEndFunction\n"
    );
    assert!(diagnostics
        .iter()
        .all(|diagnostic| diagnostic.rule != "trailing-whitespace"));
    assert!(diagnostics
        .iter()
        .any(|diagnostic| { diagnostic.rule == "forbidden-functions" }));
}

#[test]
fn repair_psc_file_removes_an_unused_import_resolved_through_the_project() {
    let dir = tempdir().unwrap();
    std::fs::create_dir_all(dir.path().join("scripts/source")).unwrap();
    std::fs::write(
        dir.path().join("scripts/source/Helpers.psc"),
        "ScriptName Helpers\n\nGlobal Function Assist()\nEndFunction\n",
    )
    .unwrap();
    let path = dir.path().join("scripts/source/Example.psc");
    std::fs::write(
        &path,
        "ScriptName Example\n\nImport Helpers\n\nFunction Test()\nEndFunction\n",
    )
    .unwrap();

    let diagnostics = repair_psc_file(
        path.to_string_lossy().into_owned(),
        ProjectLintContext {
            root: dir.path().to_string_lossy().into_owned(),
            ..Default::default()
        },
    )
    .unwrap();

    assert_eq!(
        std::fs::read_to_string(&path).unwrap(),
        "ScriptName Example\n\n\nFunction Test()\nEndFunction\n"
    );
    assert!(diagnostics
        .iter()
        .all(|diagnostic| diagnostic.rule != "unused-import"));
}

#[test]
fn preview_repair_psc_file_returns_a_diff_without_writing_the_file() {
    let dir = tempdir().unwrap();
    let path = dir.path().join("Example.psc");
    let original = "ScriptName Example  \n\nFunction Run()\n\tGame.GetPlayer()\nEndFunction\n";
    std::fs::write(&path, original).unwrap();

    let diff = preview_repair_psc_file(
        path.to_string_lossy().into_owned(),
        papyrus_lints::Config::default(),
    )
    .unwrap();

    assert_eq!(std::fs::read_to_string(&path).unwrap(), original);
    assert!(diff.contains("-ScriptName Example  \n"));
    assert!(diff.contains("+ScriptName Example\n"));
}

#[test]
fn preview_repair_psc_file_returns_an_empty_diff_for_an_already_clean_file() {
    let dir = tempdir().unwrap();
    let path = dir.path().join("Example.psc");
    std::fs::write(&path, "ScriptName Example\n").unwrap();

    let diff = preview_repair_psc_file(
        path.to_string_lossy().into_owned(),
        papyrus_lints::Config::default(),
    )
    .unwrap();

    assert_eq!(diff, "");
    assert_eq!(
        std::fs::read_to_string(&path).unwrap(),
        "ScriptName Example\n"
    );
}

#[test]
fn preview_repair_psc_file_reports_io_errors_instead_of_panicking() {
    let missing = tempdir().unwrap().path().join("missing.psc");

    assert!(preview_repair_psc_file(
        missing.to_string_lossy().into_owned(),
        papyrus_lints::Config::default(),
    )
    .is_err());
}

#[test]
fn preview_repair_psc_line_returns_the_fixed_line_without_writing_the_file() {
    let dir = tempdir().unwrap();
    let path = dir.path().join("Example.psc");
    let original = "Function Run(Int left,Int right)\nEndFunction\n";
    std::fs::write(&path, original).unwrap();

    let repaired = preview_repair_psc_line(
        path.to_string_lossy().into_owned(),
        papyrus_lints::Config::default(),
        "comma-spacing".to_string(),
        1,
    )
    .unwrap();

    assert_eq!(
        repaired.as_deref(),
        Some("Function Run(Int left, Int right)")
    );
    assert_eq!(std::fs::read_to_string(&path).unwrap(), original);
}

#[test]
fn preview_repair_psc_line_is_none_when_nothing_would_change() {
    let dir = tempdir().unwrap();
    let path = dir.path().join("Example.psc");
    std::fs::write(&path, "ScriptName Example\n").unwrap();

    let repaired = preview_repair_psc_line(
        path.to_string_lossy().into_owned(),
        papyrus_lints::Config::default(),
        "trailing-whitespace".to_string(),
        1,
    )
    .unwrap();

    assert_eq!(repaired, None);
}

#[test]
fn preview_repair_psc_line_is_none_for_a_different_line() {
    let dir = tempdir().unwrap();
    let path = dir.path().join("Example.psc");
    let original = "ScriptName Example\n\nFunction Run(Int left,Int right)\nEndFunction\n";
    std::fs::write(&path, original).unwrap();

    let repaired = preview_repair_psc_line(
        path.to_string_lossy().into_owned(),
        papyrus_lints::Config::default(),
        "comma-spacing".to_string(),
        1,
    )
    .unwrap();

    assert_eq!(repaired, None);
    assert_eq!(std::fs::read_to_string(path).unwrap(), original);
}

#[test]
fn preview_repair_psc_line_reports_io_errors_instead_of_panicking() {
    let missing = tempdir().unwrap().path().join("missing.psc");

    assert!(preview_repair_psc_line(
        missing.to_string_lossy().into_owned(),
        papyrus_lints::Config::default(),
        "trailing-whitespace".to_string(),
        1,
    )
    .is_err());
}

#[test]
fn repair_psc_finding_fixes_only_the_named_rule_on_the_given_line() {
    let dir = tempdir().unwrap();
    let path = dir.path().join("Example.psc");
    std::fs::write(
        &path,
        "ScriptName Example  \n\nFunction Run(Int left,Int right)  \nEndFunction\n",
    )
    .unwrap();

    let diagnostics = repair_psc_finding(
        path.to_string_lossy().into_owned(),
        ProjectLintContext {
            root: dir.path().to_string_lossy().into_owned(),
            ..Default::default()
        },
        "comma-spacing".to_string(),
        3,
    )
    .unwrap();

    assert_eq!(
        std::fs::read_to_string(&path).unwrap(),
        "ScriptName Example  \n\nFunction Run(Int left, Int right)  \nEndFunction\n"
    );
    assert!(diagnostics
        .iter()
        .all(|diagnostic| !(diagnostic.line == 3 && diagnostic.rule == "comma-spacing")));
    assert!(diagnostics
        .iter()
        .any(|diagnostic| { diagnostic.line == 1 && diagnostic.rule == "trailing-whitespace" }));
}

#[test]
fn repair_psc_finding_rejects_removing_an_unused_import_since_it_shifts_the_line_count() {
    // Like `property-sorting`, removing an `Import` line always changes
    // the file's total line count, so the per-line "Fix this issue"
    // button can never apply it -- only "Apply fixes"/the mass-fix
    // button (see `repair_psc_file`/`repair_psc_file_rule` above) can.
    let dir = tempdir().unwrap();
    std::fs::create_dir_all(dir.path().join("scripts/source")).unwrap();
    std::fs::write(
        dir.path().join("scripts/source/Helpers.psc"),
        "ScriptName Helpers\n\nGlobal Function Assist()\nEndFunction\n",
    )
    .unwrap();
    let path = dir.path().join("scripts/source/Example.psc");
    let source = "ScriptName Example\n\nImport Helpers\n\nFunction Test()\nEndFunction\n";
    std::fs::write(&path, source).unwrap();

    let error = repair_psc_finding(
        path.to_string_lossy().into_owned(),
        ProjectLintContext {
            root: dir.path().to_string_lossy().into_owned(),
            ..Default::default()
        },
        "unused-import".to_string(),
        3,
    )
    .unwrap_err();

    assert!(error.contains("Apply fixes"));
    assert_eq!(std::fs::read_to_string(&path).unwrap(), source);
}

#[test]
fn repair_psc_finding_rejects_a_fix_that_would_change_the_line_count() {
    let dir = tempdir().unwrap();
    let path = dir.path().join("Example.psc");
    let source = "ScriptName Example\n\nInt Property zulu Auto\nInt Property alpha Auto\n";
    std::fs::write(&path, source).unwrap();
    let mut config = papyrus_lints::Config::default();
    config.rules.property_sorting = true;

    let error = repair_psc_finding(
        path.to_string_lossy().into_owned(),
        ProjectLintContext {
            root: dir.path().to_string_lossy().into_owned(),
            config,
            ..Default::default()
        },
        "property-sorting".to_string(),
        4,
    )
    .unwrap_err();

    assert!(error.contains("Apply fixes"));
    assert_eq!(std::fs::read_to_string(&path).unwrap(), source);
}

#[test]
fn repair_psc_file_rule_fixes_every_occurrence_of_the_named_rule_and_leaves_others() {
    let dir = tempdir().unwrap();
    let path = dir.path().join("Example.psc");
    std::fs::write(
        &path,
        "ScriptName Example  \n\nFunction Run(Int left,Int right)  \nEndFunction\n",
    )
    .unwrap();

    let diagnostics = repair_psc_file_rule(
        path.to_string_lossy().into_owned(),
        ProjectLintContext {
            root: dir.path().to_string_lossy().into_owned(),
            ..Default::default()
        },
        "trailing-whitespace".to_string(),
    )
    .unwrap();

    assert_eq!(
        std::fs::read_to_string(&path).unwrap(),
        "ScriptName Example\n\nFunction Run(Int left,Int right)\nEndFunction\n"
    );
    assert!(diagnostics
        .iter()
        .all(|diagnostic| diagnostic.rule != "trailing-whitespace"));
    assert!(diagnostics
        .iter()
        .any(|diagnostic| diagnostic.rule == "comma-spacing"));
}

#[test]
fn repair_psc_file_rule_removes_an_unused_import_resolved_through_the_project() {
    let dir = tempdir().unwrap();
    std::fs::create_dir_all(dir.path().join("scripts/source")).unwrap();
    std::fs::write(
        dir.path().join("scripts/source/Helpers.psc"),
        "ScriptName Helpers\n\nGlobal Function Assist()\nEndFunction\n",
    )
    .unwrap();
    let path = dir.path().join("scripts/source/Example.psc");
    std::fs::write(
        &path,
        "ScriptName Example\n\nImport Helpers\n\nFunction Test()\n    Call(1,2)\nEndFunction\n",
    )
    .unwrap();

    let diagnostics = repair_psc_file_rule(
        path.to_string_lossy().into_owned(),
        ProjectLintContext {
            root: dir.path().to_string_lossy().into_owned(),
            ..Default::default()
        },
        "unused-import".to_string(),
    )
    .unwrap();

    assert_eq!(
        std::fs::read_to_string(&path).unwrap(),
        "ScriptName Example\n\n\nFunction Test()\n    Call(1,2)\nEndFunction\n"
    );
    assert!(diagnostics
        .iter()
        .all(|diagnostic| diagnostic.rule != "unused-import"));
    assert!(diagnostics
        .iter()
        .any(|diagnostic| diagnostic.rule == "comma-spacing"));
}

#[test]
fn targeted_repairs_leave_a_clean_file_untouched() {
    let dir = tempdir().unwrap();
    let path = dir.path().join("Example.psc");
    let source = "ScriptName Example\n";
    std::fs::write(&path, source).unwrap();
    let path_string = path.to_string_lossy().into_owned();
    let root = dir.path().to_string_lossy().into_owned();

    let finding_diagnostics = repair_psc_finding(
        path_string.clone(),
        ProjectLintContext {
            root: root.clone(),
            ..Default::default()
        },
        "trailing-whitespace".to_string(),
        1,
    )
    .unwrap();
    let file_diagnostics = repair_psc_file_rule(
        path_string,
        ProjectLintContext {
            root,
            ..Default::default()
        },
        "trailing-whitespace".to_string(),
    )
    .unwrap();

    assert!(finding_diagnostics.is_empty());
    assert!(file_diagnostics.is_empty());
    assert_eq!(std::fs::read_to_string(path).unwrap(), source);
}

#[test]
fn add_disable_comment_to_psc_line_adds_the_directive_and_relints() {
    let dir = tempdir().unwrap();
    let path = dir.path().join("Example.psc");
    std::fs::write(&path, "Call(1,2)\n").unwrap();

    let diagnostics = add_disable_comment_to_psc_line(
        path.to_string_lossy().into_owned(),
        ProjectLintContext {
            root: dir.path().to_string_lossy().into_owned(),
            ..Default::default()
        },
        vec!["comma-spacing".to_string()],
        1,
    )
    .unwrap();

    assert_eq!(
        std::fs::read_to_string(&path).unwrap(),
        "Call(1,2) ; @disable comma-spacing\n"
    );
    assert!(diagnostics
        .iter()
        .all(|diagnostic| diagnostic.rule != "comma-spacing"));
}

#[test]
fn add_disable_comment_to_psc_line_leaves_the_file_untouched_for_an_empty_rule_list() {
    let dir = tempdir().unwrap();
    let path = dir.path().join("Example.psc");
    std::fs::write(&path, "Call(1,2)\n").unwrap();

    add_disable_comment_to_psc_line(
        path.to_string_lossy().into_owned(),
        ProjectLintContext {
            root: dir.path().to_string_lossy().into_owned(),
            ..Default::default()
        },
        Vec::new(),
        1,
    )
    .unwrap();

    assert_eq!(std::fs::read_to_string(&path).unwrap(), "Call(1,2)\n");
}

#[test]
fn add_nodiscard_comment_to_psc_line_adds_the_flag_and_relints() {
    let dir = tempdir().unwrap();
    let path = dir.path().join("Example.psc");
    std::fs::write(
        &path,
        "ScriptName Example\n\nInt Function RegisterFoo()\n    Return 1\nEndFunction\n",
    )
    .unwrap();

    add_nodiscard_comment_to_psc_line(
        path.to_string_lossy().into_owned(),
        ProjectLintContext {
            root: dir.path().to_string_lossy().into_owned(),
            ..Default::default()
        },
        3,
    )
    .unwrap();

    assert_eq!(
        std::fs::read_to_string(&path).unwrap(),
        "ScriptName Example\n\nInt Function RegisterFoo() ; @nodiscard\n    Return 1\nEndFunction\n"
    );
}

#[test]
fn add_nodiscard_comment_to_psc_line_leaves_an_already_flagged_header_untouched() {
    let dir = tempdir().unwrap();
    let path = dir.path().join("Example.psc");
    let source = "Int Function RegisterFoo() ; @nodiscard\n";
    std::fs::write(&path, source).unwrap();

    add_nodiscard_comment_to_psc_line(
        path.to_string_lossy().into_owned(),
        ProjectLintContext {
            root: dir.path().to_string_lossy().into_owned(),
            ..Default::default()
        },
        1,
    )
    .unwrap();

    assert_eq!(std::fs::read_to_string(&path).unwrap(), source);
}

#[test]
fn targeted_repair_commands_report_io_errors_without_creating_a_file() {
    let dir = tempdir().unwrap();
    let missing = dir.path().join("missing.psc");
    let path = missing.to_string_lossy().into_owned();
    let root = dir.path().to_string_lossy().into_owned();

    assert!(repair_psc_finding(
        path.clone(),
        ProjectLintContext {
            root: root.clone(),
            ..Default::default()
        },
        "trailing-whitespace".to_string(),
        1
    )
    .is_err());
    assert!(repair_psc_file_rule(
        path.clone(),
        ProjectLintContext {
            root: root.clone(),
            ..Default::default()
        },
        "trailing-whitespace".to_string()
    )
    .is_err());
    assert!(add_disable_comment_to_psc_line(
        path.clone(),
        ProjectLintContext {
            root: root.clone(),
            ..Default::default()
        },
        vec!["trailing-whitespace".to_string()],
        1
    )
    .is_err());
    assert!(add_nodiscard_comment_to_psc_line(
        path,
        ProjectLintContext {
            root,
            ..Default::default()
        },
        1
    )
    .is_err());
    assert!(!missing.exists());
}

#[test]
fn repair_does_not_rewrite_an_already_clean_file() {
    let dir = tempdir().unwrap();
    let path = dir.path().join("Example.psc");
    let source = "ScriptName Example\n";
    std::fs::write(&path, source).unwrap();

    let diagnostics = repair_psc_file(
        path.to_string_lossy().into_owned(),
        ProjectLintContext {
            root: dir.path().to_string_lossy().into_owned(),
            ..Default::default()
        },
    )
    .unwrap();

    assert!(diagnostics.is_empty());
    assert_eq!(std::fs::read_to_string(path).unwrap(), source);
}

#[test]
fn repair_psc_file_preserves_a_cp1252_encoded_files_encoding() {
    let dir = tempdir().unwrap();
    let path = dir.path().join("Example.psc");
    // "ScriptName Example  \n\n; caf\xE9\n" with trailing whitespace on
    // the first line to fix, and 0xE9 ("é" in Windows-1252) making the
    // file as a whole invalid UTF-8.
    let mut contents = b"ScriptName Example  \n\n; caf".to_vec();
    contents.push(0xE9);
    contents.push(b'\n');
    std::fs::write(&path, &contents).unwrap();

    repair_psc_file(
        path.to_string_lossy().into_owned(),
        ProjectLintContext {
            root: dir.path().to_string_lossy().into_owned(),
            ..Default::default()
        },
    )
    .unwrap();

    let mut expected = b"ScriptName Example\n\n; caf".to_vec();
    expected.push(0xE9);
    expected.push(b'\n');
    assert_eq!(std::fs::read(&path).unwrap(), expected);
}

#[test]
fn targeted_repairs_preserve_a_cp1252_encoded_files_encoding() {
    let dir = tempdir().unwrap();
    let path = dir.path().join("Example.psc");
    let mut source = b"ScriptName Example  \n\n; caf".to_vec();
    source.extend_from_slice(&[0xE9, b'\n']);
    std::fs::write(&path, &source).unwrap();

    repair_psc_finding(
        path.to_string_lossy().into_owned(),
        ProjectLintContext {
            root: dir.path().to_string_lossy().into_owned(),
            ..Default::default()
        },
        "trailing-whitespace".to_string(),
        1,
    )
    .unwrap();

    let mut expected = b"ScriptName Example\n\n; caf".to_vec();
    expected.extend_from_slice(&[0xE9, b'\n']);
    assert_eq!(std::fs::read(&path).unwrap(), expected);

    std::fs::write(&path, &source).unwrap();
    repair_psc_file_rule(
        path.to_string_lossy().into_owned(),
        ProjectLintContext {
            root: dir.path().to_string_lossy().into_owned(),
            ..Default::default()
        },
        "trailing-whitespace".to_string(),
    )
    .unwrap();

    assert_eq!(std::fs::read(path).unwrap(), expected);
}

#[test]
fn repair_psc_file_removes_an_unused_import_from_an_additional_script_root() {
    let extra = tempdir().unwrap();
    std::fs::write(
        extra.path().join("Helpers.psc"),
        "ScriptName Helpers\n\nGlobal Function Assist()\nEndFunction\n",
    )
    .unwrap();
    let dir = tempdir().unwrap();
    let path = dir.path().join("Example.psc");
    std::fs::write(
        &path,
        "ScriptName Example\n\nImport Helpers\n\nFunction Test()\nEndFunction\n",
    )
    .unwrap();

    let diagnostics = repair_psc_file(
        path.to_string_lossy().into_owned(),
        ProjectLintContext {
            root: dir.path().to_string_lossy().into_owned(),
            additional_roots: vec![extra.path().to_string_lossy().into_owned()],
            ..Default::default()
        },
    )
    .unwrap();

    assert_eq!(
        std::fs::read_to_string(&path).unwrap(),
        "ScriptName Example\n\n\nFunction Test()\nEndFunction\n"
    );
    assert!(diagnostics
        .iter()
        .all(|diagnostic| diagnostic.rule != "unused-import"));
}

#[test]
fn add_disable_comment_to_psc_line_covers_multiple_rules_and_merges_into_an_existing_directive() {
    let dir = tempdir().unwrap();
    let path = dir.path().join("Example.psc");
    std::fs::write(&path, "Call(1,2)  \n").unwrap();
    let path_string = path.to_string_lossy().into_owned();
    let root = dir.path().to_string_lossy().into_owned();

    add_disable_comment_to_psc_line(
        path_string.clone(),
        ProjectLintContext {
            root: root.clone(),
            ..Default::default()
        },
        vec![
            "comma-spacing".to_string(),
            "trailing-whitespace".to_string(),
        ],
        1,
    )
    .unwrap();
    assert_eq!(
        std::fs::read_to_string(&path).unwrap(),
        "Call(1,2)   ; @disable comma-spacing, trailing-whitespace\n"
    );

    let diagnostics = add_disable_comment_to_psc_line(
        path_string,
        ProjectLintContext {
            root,
            ..Default::default()
        },
        vec!["trailing-whitespace".to_string()],
        1,
    )
    .unwrap();

    assert_eq!(
        std::fs::read_to_string(&path).unwrap(),
        "Call(1,2)   ; @disable comma-spacing, trailing-whitespace\n"
    );
    assert!(diagnostics.iter().all(|diagnostic| {
        diagnostic.rule != "comma-spacing" && diagnostic.rule != "trailing-whitespace"
    }));
}

#[test]
fn add_disable_comment_to_psc_line_preserves_a_cp1252_encoded_files_encoding() {
    let dir = tempdir().unwrap();
    let path = dir.path().join("Example.psc");
    let mut source = b"Call(1,2)\n; caf".to_vec();
    source.extend_from_slice(&[0xE9, b'\n']);
    std::fs::write(&path, &source).unwrap();

    add_disable_comment_to_psc_line(
        path.to_string_lossy().into_owned(),
        ProjectLintContext {
            root: dir.path().to_string_lossy().into_owned(),
            ..Default::default()
        },
        vec!["comma-spacing".to_string()],
        1,
    )
    .unwrap();

    let mut expected = b"Call(1,2) ; @disable comma-spacing\n; caf".to_vec();
    expected.extend_from_slice(&[0xE9, b'\n']);
    assert_eq!(std::fs::read(path).unwrap(), expected);
}

#[test]
#[cfg(unix)]
fn repair_psc_file_merges_in_compiler_reported_errors_when_enabled() {
    use std::os::unix::fs::PermissionsExt;

    let dir = tempdir().unwrap();
    let source_dir = dir.path().join("Scripts/Source");
    std::fs::create_dir_all(&source_dir).unwrap();
    let path = source_dir.join("Example.psc");
    std::fs::write(&path, "ScriptName Example\n").unwrap();
    let compiler_path = dir.path().join("compiler.sh");
    std::fs::write(
        &compiler_path,
        "#!/bin/sh\necho \"Example.psc(2,1): missing EndFunction\" >&2\nexit 1\n",
    )
    .unwrap();
    std::fs::set_permissions(&compiler_path, std::fs::Permissions::from_mode(0o755)).unwrap();

    let diagnostics = repair_psc_file(
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
            && diagnostic.line == 2
            && diagnostic.column == 1
    }));
    assert_eq!(
        std::fs::read_to_string(path).unwrap(),
        "ScriptName Example\n"
    );
}
