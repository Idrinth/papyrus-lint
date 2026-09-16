//! Repair, preview-repair, and per-line disable-comment commands.

use std::path::Path;

use papyrus_lint_core::ast_cache;
use papyrus_lint_core::source_encoding::{
    read_psc_source, read_psc_source_with_encoding, write_psc_source,
};

use crate::lint::{lint_with_compile_check, project_function_table};

/// Reads the `.psc` file at `path`, applies every automatic fix (honoring
/// the semicolon and indentation style `config` selects), writes the
/// repaired source back to disk, and returns the diagnostics that remain.
/// See [`lint_psc_file`] for `root`/`additional_roots`/`compiler_path`/
/// `compile_check`.
#[tauri::command(async)]
#[allow(clippy::too_many_arguments)]
pub(crate) fn repair_psc_file(
    path: String,
    root: String,
    config: papyrus_lints::Config,
    additional_roots: Vec<String>,
    lookup_roots: Vec<String>,
    compiler_path: String,
    compile_check: bool,
) -> Result<Vec<papyrus_lints::Diagnostic>, String> {
    let path = Path::new(&path);
    let (source, encoding) = read_psc_source_with_encoding(path).map_err(|err| err.to_string())?;
    // Built before the fix, rather than after it like `lint_psc_file`'s own
    // call, since "unused-import" -- unlike every other fixable rule -- can
    // only resolve which imports are unused through this project's own
    // cross-script resolver (see `papyrus_lints::repair_with_external_arguments`).
    let mut function_table = project_function_table(root, additional_roots.clone(), lookup_roots);
    let repaired =
        papyrus_lints::repair_with_external_arguments(&source, &config, &mut function_table);
    if repaired != source {
        write_psc_source(path, &repaired, encoding).map_err(|err| err.to_string())?;
    }
    ast_cache::ensure_primed(path, &repaired);
    Ok(lint_with_compile_check(
        path,
        &repaired,
        &config,
        &mut function_table,
        &additional_roots,
        &compiler_path,
        compile_check,
    ))
}

/// Like [`repair_psc_file`], but never writes anything to disk: computes the
/// same whole-file automatic fix and returns a standard unified diff (the
/// same `diff -u`/`git diff` hunk format `PapyrusLinterCLI fix --dry-run`
/// prints, via [`papyrus_lint_core::diff::unified_diff`]) between the
/// original source and what applying the fix would produce, or an empty
/// string if nothing would change. Drives the code viewer's "Preview
/// fixes" button, so a user can see what "Apply fixes" would do before
/// committing to it.
#[tauri::command(async)]
pub(crate) fn preview_repair_psc_file(
    path: String,
    config: papyrus_lints::Config,
) -> Result<String, String> {
    let path = Path::new(&path);
    let source = read_psc_source(path).map_err(|err| err.to_string())?;
    let repaired = papyrus_lints::repair(&source, &config);
    Ok(papyrus_lint_core::diff::unified_diff(
        &path.display().to_string(),
        &source,
        &repaired,
    ))
}

/// Applies only the automatic fix for `rule` (a
/// [`papyrus_lints::FIXABLE_RULE_IDS`] id) and returns what `line`
/// (1-indexed) would look like afterward, via
/// [`papyrus_lints::repaired_line`], without writing anything to disk.
/// Returns `None` when there's nothing meaningful to preview: the fix
/// doesn't change the file at all, it would shift the line count elsewhere
/// (e.g. `property-sorting` relocating a property's declaration), or it
/// simply doesn't touch `line`. Drives the "Export for AI" document's
/// per-finding `repair` preview (see `formatIssuesForAi` in
/// `app/src/main.ts`), so an AI reading the export can see each
/// auto-fixable finding's fix without applying it first.
#[tauri::command(async)]
pub(crate) fn preview_repair_psc_line(
    path: String,
    config: papyrus_lints::Config,
    rule: String,
    line: usize,
) -> Result<Option<String>, String> {
    let path = Path::new(&path);
    let source = read_psc_source(path).map_err(|err| err.to_string())?;
    Ok(papyrus_lints::repaired_line(&source, &config, &rule, line))
}

/// Like [`repair_psc_file`], but applies only the automatic fix for `rule`
/// (a [`papyrus_lints::FIXABLE_RULE_IDS`] id), and restricts its effect to
/// `line` (1-indexed) — leaving every other line untouched — via
/// [`papyrus_lints::restrict_to_line`]. Drives the frontend's per-finding
/// "Fix this issue" button. Fails if the named rule's fix would change
/// `line`'s line count elsewhere in the file (e.g. `property-sorting`
/// relocating a property's declaration), since a single original line
/// number then no longer identifies the same line in the result; the
/// frontend surfaces that error and points the user at "Apply fixes"
/// instead.
#[tauri::command(async)]
#[allow(clippy::too_many_arguments)]
pub(crate) fn repair_psc_finding(
    path: String,
    root: String,
    config: papyrus_lints::Config,
    additional_roots: Vec<String>,
    lookup_roots: Vec<String>,
    compiler_path: String,
    compile_check: bool,
    rule: String,
    line: usize,
) -> Result<Vec<papyrus_lints::Diagnostic>, String> {
    let path = Path::new(&path);
    let (source, encoding) = read_psc_source_with_encoding(path).map_err(|err| err.to_string())?;
    // See `repair_psc_file`'s own comment: built before the fix so
    // "unused-import"'s fix (if `rule` names it) can resolve through it too.
    let mut function_table = project_function_table(root, additional_roots.clone(), lookup_roots);
    let repaired = papyrus_lints::repair_filtered_with_external_arguments(
        &source,
        &config,
        &mut function_table,
        Some(rule.as_str()),
    );
    let repaired = papyrus_lints::restrict_to_line(&source, &repaired, line).ok_or_else(|| {
        "Fixing this issue would change other lines in the file; use \"Apply fixes\" instead."
            .to_string()
    })?;
    if repaired != source {
        write_psc_source(path, &repaired, encoding).map_err(|err| err.to_string())?;
    }
    ast_cache::ensure_primed(path, &repaired);
    Ok(lint_with_compile_check(
        path,
        &repaired,
        &config,
        &mut function_table,
        &additional_roots,
        &compiler_path,
        compile_check,
    ))
}

/// Like [`repair_psc_file`], but applies only the automatic fix for `rule`
/// (a [`papyrus_lints::FIXABLE_RULE_IDS`] id) across the whole file, rather
/// than every fixable rule. Unlike [`repair_psc_finding`], it isn't
/// restricted to a single line and can't fail on a line-count mismatch, so
/// it drives the frontend's "mass fix" action, which repeats this call
/// across every file in the current results to clear one issue project-wide
/// (e.g. every trailing-whitespace finding) in one go.
#[tauri::command(async)]
#[allow(clippy::too_many_arguments)]
pub(crate) fn repair_psc_file_rule(
    path: String,
    root: String,
    config: papyrus_lints::Config,
    additional_roots: Vec<String>,
    lookup_roots: Vec<String>,
    compiler_path: String,
    compile_check: bool,
    rule: String,
) -> Result<Vec<papyrus_lints::Diagnostic>, String> {
    let path = Path::new(&path);
    let (source, encoding) = read_psc_source_with_encoding(path).map_err(|err| err.to_string())?;
    // See `repair_psc_file`'s own comment: built before the fix so
    // "unused-import"'s fix (if `rule` names it) can resolve through it too.
    let mut function_table = project_function_table(root, additional_roots.clone(), lookup_roots);
    let repaired = papyrus_lints::repair_filtered_with_external_arguments(
        &source,
        &config,
        &mut function_table,
        Some(rule.as_str()),
    );
    if repaired != source {
        write_psc_source(path, &repaired, encoding).map_err(|err| err.to_string())?;
    }
    ast_cache::ensure_primed(path, &repaired);
    Ok(lint_with_compile_check(
        path,
        &repaired,
        &config,
        &mut function_table,
        &additional_roots,
        &compiler_path,
        compile_check,
    ))
}

/// Adds (or extends) an `; @disable <rules>` comment on `line` (1-indexed)
/// of the named `.psc` file, covering every id in `rules` — the code
/// viewer's per-line "Ignore" button, the inverse of
/// [`repair_psc_finding`]'s per-line "Fix": rather than fixing the findings
/// on that line, it silences them via
/// [`papyrus_lints::add_disable_comment`] instead. Re-lints the file
/// afterward and returns its updated diagnostics, the same as every other
/// mutating command here.
#[tauri::command(async)]
#[allow(clippy::too_many_arguments)]
pub(crate) fn add_disable_comment_to_psc_line(
    path: String,
    root: String,
    config: papyrus_lints::Config,
    additional_roots: Vec<String>,
    lookup_roots: Vec<String>,
    compiler_path: String,
    compile_check: bool,
    rules: Vec<String>,
    line: usize,
) -> Result<Vec<papyrus_lints::Diagnostic>, String> {
    let path = Path::new(&path);
    let (source, encoding) = read_psc_source_with_encoding(path).map_err(|err| err.to_string())?;
    let updated = papyrus_lints::add_disable_comment(&source, line, &rules);
    if updated != source {
        write_psc_source(path, &updated, encoding).map_err(|err| err.to_string())?;
    }
    ast_cache::ensure_primed(path, &updated);
    let mut function_table = project_function_table(root, additional_roots.clone(), lookup_roots);
    Ok(lint_with_compile_check(
        path,
        &updated,
        &config,
        &mut function_table,
        &additional_roots,
        &compiler_path,
        compile_check,
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
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
            dir.path().to_string_lossy().into_owned(),
            papyrus_lints::Config::default(),
            Vec::new(),
            Vec::new(),
            String::new(),
            false,
        )
        .unwrap();

        assert_eq!(
            std::fs::read_to_string(&path).unwrap(),
            "ScriptName Example\n\nFunction Run()\n\tGame.GetPlayer()\nEndFunction\n"
        );
        assert!(diagnostics
            .iter()
            .all(|diagnostic| diagnostic.rule != papyrus_lints::trailing_whitespace::RULE));
        assert!(diagnostics
            .iter()
            .any(|diagnostic| { diagnostic.rule == papyrus_lints::forbidden_functions::RULE }));
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
            dir.path().to_string_lossy().into_owned(),
            papyrus_lints::Config::default(),
            Vec::new(),
            Vec::new(),
            String::new(),
            false,
        )
        .unwrap();

        assert_eq!(
            std::fs::read_to_string(&path).unwrap(),
            "ScriptName Example\n\n\nFunction Test()\nEndFunction\n"
        );
        assert!(diagnostics
            .iter()
            .all(|diagnostic| diagnostic.rule != papyrus_lints::unused_import::RULE));
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
            papyrus_lints::comma_spacing::RULE.to_string(),
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
            dir.path().to_string_lossy().into_owned(),
            papyrus_lints::Config::default(),
            Vec::new(),
            Vec::new(),
            String::new(),
            false,
            papyrus_lints::comma_spacing::RULE.to_string(),
            3,
        )
        .unwrap();

        assert_eq!(
            std::fs::read_to_string(&path).unwrap(),
            "ScriptName Example  \n\nFunction Run(Int left, Int right)  \nEndFunction\n"
        );
        assert!(diagnostics.iter().all(|diagnostic| !(diagnostic.line == 3
            && diagnostic.rule == papyrus_lints::comma_spacing::RULE)));
        assert!(diagnostics.iter().any(|diagnostic| {
            diagnostic.line == 1 && diagnostic.rule == papyrus_lints::trailing_whitespace::RULE
        }));
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
            dir.path().to_string_lossy().into_owned(),
            papyrus_lints::Config::default(),
            Vec::new(),
            Vec::new(),
            String::new(),
            false,
            papyrus_lints::unused_import::RULE.to_string(),
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
            dir.path().to_string_lossy().into_owned(),
            config,
            Vec::new(),
            Vec::new(),
            String::new(),
            false,
            papyrus_lints::property_sorting::RULE.to_string(),
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
            dir.path().to_string_lossy().into_owned(),
            papyrus_lints::Config::default(),
            Vec::new(),
            Vec::new(),
            String::new(),
            false,
            papyrus_lints::trailing_whitespace::RULE.to_string(),
        )
        .unwrap();

        assert_eq!(
            std::fs::read_to_string(&path).unwrap(),
            "ScriptName Example\n\nFunction Run(Int left,Int right)\nEndFunction\n"
        );
        assert!(diagnostics
            .iter()
            .all(|diagnostic| diagnostic.rule != papyrus_lints::trailing_whitespace::RULE));
        assert!(diagnostics
            .iter()
            .any(|diagnostic| diagnostic.rule == papyrus_lints::comma_spacing::RULE));
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
            dir.path().to_string_lossy().into_owned(),
            papyrus_lints::Config::default(),
            Vec::new(),
            Vec::new(),
            String::new(),
            false,
            papyrus_lints::unused_import::RULE.to_string(),
        )
        .unwrap();

        assert_eq!(
            std::fs::read_to_string(&path).unwrap(),
            "ScriptName Example\n\n\nFunction Test()\n    Call(1,2)\nEndFunction\n"
        );
        assert!(diagnostics
            .iter()
            .all(|diagnostic| diagnostic.rule != papyrus_lints::unused_import::RULE));
        assert!(diagnostics
            .iter()
            .any(|diagnostic| diagnostic.rule == papyrus_lints::comma_spacing::RULE));
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
            root.clone(),
            Default::default(),
            Vec::new(),
            Vec::new(),
            String::new(),
            false,
            papyrus_lints::trailing_whitespace::RULE.to_string(),
            1,
        )
        .unwrap();
        let file_diagnostics = repair_psc_file_rule(
            path_string,
            root,
            Default::default(),
            Vec::new(),
            Vec::new(),
            String::new(),
            false,
            papyrus_lints::trailing_whitespace::RULE.to_string(),
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
            dir.path().to_string_lossy().into_owned(),
            papyrus_lints::Config::default(),
            Vec::new(),
            Vec::new(),
            String::new(),
            false,
            vec![papyrus_lints::comma_spacing::RULE.to_string()],
            1,
        )
        .unwrap();

        assert_eq!(
            std::fs::read_to_string(&path).unwrap(),
            "Call(1,2) ; @disable comma-spacing\n"
        );
        assert!(diagnostics
            .iter()
            .all(|diagnostic| diagnostic.rule != papyrus_lints::comma_spacing::RULE));
    }

    #[test]
    fn add_disable_comment_to_psc_line_leaves_the_file_untouched_for_an_empty_rule_list() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("Example.psc");
        std::fs::write(&path, "Call(1,2)\n").unwrap();

        add_disable_comment_to_psc_line(
            path.to_string_lossy().into_owned(),
            dir.path().to_string_lossy().into_owned(),
            papyrus_lints::Config::default(),
            Vec::new(),
            Vec::new(),
            String::new(),
            false,
            Vec::new(),
            1,
        )
        .unwrap();

        assert_eq!(std::fs::read_to_string(&path).unwrap(), "Call(1,2)\n");
    }

    #[test]
    fn targeted_repair_commands_report_io_errors_without_creating_a_file() {
        let dir = tempdir().unwrap();
        let missing = dir.path().join("missing.psc");
        let path = missing.to_string_lossy().into_owned();
        let root = dir.path().to_string_lossy().into_owned();

        assert!(repair_psc_finding(
            path.clone(),
            root.clone(),
            papyrus_lints::Config::default(),
            Vec::new(),
            Vec::new(),
            String::new(),
            false,
            papyrus_lints::trailing_whitespace::RULE.to_string(),
            1,
        )
        .is_err());
        assert!(repair_psc_file_rule(
            path.clone(),
            root.clone(),
            papyrus_lints::Config::default(),
            Vec::new(),
            Vec::new(),
            String::new(),
            false,
            papyrus_lints::trailing_whitespace::RULE.to_string(),
        )
        .is_err());
        assert!(add_disable_comment_to_psc_line(
            path,
            root,
            papyrus_lints::Config::default(),
            Vec::new(),
            Vec::new(),
            String::new(),
            false,
            vec![papyrus_lints::trailing_whitespace::RULE.to_string()],
            1,
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
            dir.path().to_string_lossy().into_owned(),
            Default::default(),
            Vec::new(),
            Vec::new(),
            String::new(),
            false,
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
            dir.path().to_string_lossy().into_owned(),
            papyrus_lints::Config::default(),
            Vec::new(),
            Vec::new(),
            String::new(),
            false,
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
            dir.path().to_string_lossy().into_owned(),
            Default::default(),
            Vec::new(),
            Vec::new(),
            String::new(),
            false,
            papyrus_lints::trailing_whitespace::RULE.to_string(),
            1,
        )
        .unwrap();

        let mut expected = b"ScriptName Example\n\n; caf".to_vec();
        expected.extend_from_slice(&[0xE9, b'\n']);
        assert_eq!(std::fs::read(&path).unwrap(), expected);

        std::fs::write(&path, &source).unwrap();
        repair_psc_file_rule(
            path.to_string_lossy().into_owned(),
            dir.path().to_string_lossy().into_owned(),
            Default::default(),
            Vec::new(),
            Vec::new(),
            String::new(),
            false,
            papyrus_lints::trailing_whitespace::RULE.to_string(),
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
            dir.path().to_string_lossy().into_owned(),
            papyrus_lints::Config::default(),
            vec![extra.path().to_string_lossy().into_owned()],
            Vec::new(),
            String::new(),
            false,
        )
        .unwrap();

        assert_eq!(
            std::fs::read_to_string(&path).unwrap(),
            "ScriptName Example\n\n\nFunction Test()\nEndFunction\n"
        );
        assert!(diagnostics
            .iter()
            .all(|diagnostic| diagnostic.rule != papyrus_lints::unused_import::RULE));
    }

    #[test]
    fn add_disable_comment_to_psc_line_covers_multiple_rules_and_merges_into_an_existing_directive()
    {
        let dir = tempdir().unwrap();
        let path = dir.path().join("Example.psc");
        std::fs::write(&path, "Call(1,2)  \n").unwrap();
        let path_string = path.to_string_lossy().into_owned();
        let root = dir.path().to_string_lossy().into_owned();

        add_disable_comment_to_psc_line(
            path_string.clone(),
            root.clone(),
            papyrus_lints::Config::default(),
            Vec::new(),
            Vec::new(),
            String::new(),
            false,
            vec![
                papyrus_lints::comma_spacing::RULE.to_string(),
                papyrus_lints::trailing_whitespace::RULE.to_string(),
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
            root,
            papyrus_lints::Config::default(),
            Vec::new(),
            Vec::new(),
            String::new(),
            false,
            vec![papyrus_lints::trailing_whitespace::RULE.to_string()],
            1,
        )
        .unwrap();

        assert_eq!(
            std::fs::read_to_string(&path).unwrap(),
            "Call(1,2)   ; @disable comma-spacing, trailing-whitespace\n"
        );
        assert!(diagnostics.iter().all(|diagnostic| {
            diagnostic.rule != papyrus_lints::comma_spacing::RULE
                && diagnostic.rule != papyrus_lints::trailing_whitespace::RULE
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
            dir.path().to_string_lossy().into_owned(),
            papyrus_lints::Config::default(),
            Vec::new(),
            Vec::new(),
            String::new(),
            false,
            vec![papyrus_lints::comma_spacing::RULE.to_string()],
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
            dir.path().to_string_lossy().into_owned(),
            Default::default(),
            Vec::new(),
            Vec::new(),
            compiler_path.to_string_lossy().into_owned(),
            true,
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
}
