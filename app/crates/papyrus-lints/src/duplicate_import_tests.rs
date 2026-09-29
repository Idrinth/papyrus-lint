use super::*;

fn check(source: &str) -> Vec<Diagnostic> {
    let ast = papyrus_parser::parse(source).ok();
    let tokens = papyrus_parser::tokenize(source).ok();
    super::check(
        source,
        ast.as_ref(),
        tokens.as_deref(),
        &crate::config::Config::default(),
        &mut crate::external_signatures::NoExternalSignatures,
    )
}

fn repair(source: &str) -> String {
    let ast = papyrus_parser::parse(source).ok();
    let tokens = papyrus_parser::tokenize(source).ok();
    super::repair(
        source,
        ast.as_ref(),
        tokens.as_deref(),
        &crate::config::Config::default(),
    )
}

#[test]
fn flags_an_exact_duplicate_import() {
    let diagnostics = check(
        "ScriptName MyQuest Extends Quest\n\nImport Utility\nImport Debug\nImport Utility\n",
    );

    assert_eq!(diagnostics.len(), 1);
    assert_eq!(diagnostics[0].rule, RULE);
    assert_eq!(diagnostics[0].line, 5);
    assert!(diagnostics[0].message.starts_with("[warning]"));
    assert!(diagnostics[0].message.contains("'Utility'"));
    assert!(diagnostics[0].message.contains("duplicated"));
}

#[test]
fn compares_import_targets_case_insensitively() {
    let diagnostics = check("ScriptName Example\n\nImport Utility\nImport utility\n");

    assert_eq!(diagnostics.len(), 1);
    assert_eq!(diagnostics[0].line, 4);
    assert!(diagnostics[0].message.contains("'utility'"));
}

#[test]
fn flags_each_later_duplicate_of_the_same_script() {
    let diagnostics = check("ScriptName Example\n\nImport Utility\nImport Utility\nImport Utility\n");

    assert_eq!(diagnostics.len(), 2);
    assert_eq!(diagnostics[0].line, 4);
    assert_eq!(diagnostics[1].line, 5);
}

#[test]
fn does_not_flag_distinct_imports() {
    let diagnostics = check("ScriptName Example\n\nImport Utility\nImport Debug\nImport Game\n");

    assert!(diagnostics.is_empty());
}

#[test]
fn does_not_flag_a_single_import() {
    let diagnostics = check("ScriptName Example\n\nImport Utility\n");

    assert!(diagnostics.is_empty());
}

#[test]
fn flags_a_used_import_that_is_listed_twice() {
    // Distinct from unused-import: both lines name a used script; only the
    // later duplicate is flagged here.
    let diagnostics = check(
        "ScriptName Example\n\nImport Utility\nImport Utility\n\nFunction Test()\n    Utility.Wait(1.0)\nEndFunction\n",
    );

    assert_eq!(diagnostics.len(), 1);
    assert_eq!(diagnostics[0].line, 4);
    assert_eq!(diagnostics[0].rule, RULE);
}

#[test]
fn does_not_crash_on_unparseable_source() {
    assert!(check("ScriptName Example\n\nImport Utility\nImport (\n").is_empty());
}

#[test]
fn repair_drops_later_duplicates_and_keeps_the_first() {
    let source = "ScriptName Example\n\nImport Utility\nImport Debug\nImport Utility\n";

    let repaired = repair(source);

    assert_eq!(
        repaired,
        "ScriptName Example\n\nImport Utility\nImport Debug\n"
    );
    assert!(check(&repaired).is_empty());
}

#[test]
fn repair_drops_case_variant_duplicates() {
    let source = "ScriptName Example\n\nImport Utility\nImport utility\n";

    assert_eq!(repair(source), "ScriptName Example\n\nImport Utility\n");
}

#[test]
fn repair_removes_multiple_later_duplicates() {
    let source = "ScriptName Example\n\nImport Utility\nImport Utility\nImport Utility\n";

    assert_eq!(repair(source), "ScriptName Example\n\nImport Utility\n");
}

#[test]
fn repair_leaves_unique_imports_unchanged() {
    let source = "ScriptName Example\n\nImport Utility\nImport Debug\n";

    assert_eq!(repair(source), source);
}

#[test]
fn repair_preserves_crlf_line_endings() {
    let source = "ScriptName Example\r\n\r\nImport Utility\r\nImport Utility\r\n";

    assert_eq!(
        repair(source),
        "ScriptName Example\r\n\r\nImport Utility\r\n"
    );
}

#[test]
fn repair_does_not_crash_on_unparseable_source() {
    let source = "ScriptName Example\n\nImport Utility\nImport (\n";

    assert_eq!(repair(source), source);
}

#[test]
fn repair_can_remove_a_final_line_without_a_line_ending() {
    let source = "ScriptName Example\nImport Utility\nImport Utility";

    assert_eq!(repair(source), "ScriptName Example\nImport Utility\n");
}

#[test]
fn disable_directives_suppress_diagnostics() {
    let line_disabled = crate::lint(
        "ScriptName Example\n\nImport Utility\nImport Utility ; @disable duplicate-import\n",
        &crate::config::Config::default(),
    );
    let file_disabled = crate::lint(
        "; @disable-file duplicate-import\nScriptName Example\n\nImport Utility\nImport Utility\n",
        &crate::config::Config::default(),
    );

    assert!(line_disabled
        .iter()
        .all(|diagnostic| diagnostic.rule != RULE));
    assert!(file_disabled
        .iter()
        .all(|diagnostic| diagnostic.rule != RULE));
}

#[test]
fn config_off_switch_suppresses_diagnostics() {
    let mut config = crate::config::Config::default();
    config.rules.duplicate_import = false;

    let diagnostics = crate::lint(
        "ScriptName Example\n\nImport Utility\nImport Utility\n",
        &config,
    );

    assert!(diagnostics
        .iter()
        .all(|diagnostic| diagnostic.rule != RULE));
}
