use super::*;

fn check(source: &str) -> Vec<Diagnostic> {
    let ast = papyrus_parser::parse(source).ok();
    let tokens = papyrus_parser::tokenize(source).ok();
    super::check(
        source,
        ast.as_ref(),
        tokens.as_deref(),
        &crate::config::Config::default(),
        &mut crate::argument_types::NoExternalSignatures,
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
fn ignores_correctly_spaced_operators() {
    let source = "\
ScriptName Example

Function Test(Int a, Int b)
    If a == b && a > 0 || b <= 1 && a != b
    EndIf
EndFunction
";
    assert!(check(source).is_empty());
    assert_eq!(repair(source), source);
}

#[test]
fn flags_missing_space_on_both_sides() {
    let diagnostics = check("If a==b\nEndIf\n");
    assert_eq!(diagnostics.len(), 2);
    assert!(diagnostics.iter().any(|d| d.message.contains("preceded")));
    assert!(diagnostics.iter().any(|d| d.message.contains("followed")));
}

#[test]
fn flags_missing_space_before_only() {
    let diagnostics = check("If a ==b\nEndIf\n");
    assert_eq!(diagnostics.len(), 1);
    assert!(diagnostics[0].message.contains("followed"));
}

#[test]
fn flags_missing_space_after_only() {
    let diagnostics = check("If a== b\nEndIf\n");
    assert_eq!(diagnostics.len(), 1);
    assert!(diagnostics[0].message.contains("preceded"));
}

#[test]
fn flags_extra_spaces_and_tabs() {
    let diagnostics = check("If a  ==\tb\nEndIf\n");
    assert_eq!(diagnostics.len(), 2);
}

#[test]
fn checks_every_relevant_operator_kind() {
    for source in [
        "If a&&b\nEndIf\n",
        "If a||b\nEndIf\n",
        "If a!=b\nEndIf\n",
        "If a>b\nEndIf\n",
        "If a<b\nEndIf\n",
        "If a>=b\nEndIf\n",
        "If a<=b\nEndIf\n",
    ] {
        let diagnostics = check(source);
        assert_eq!(diagnostics.len(), 2, "source: {source:?}");
    }
}

#[test]
fn ignores_unrelated_operators() {
    assert!(check("Int a = 1+2\nInt b = a-1\nBool c = !a\n").is_empty());
}

#[test]
fn does_not_flag_a_statement_continued_across_lines() {
    let source = "If a \\\n    && b\nEndIf\n";
    assert!(check(source).is_empty());
    assert_eq!(repair(source), source);
}

#[test]
fn fragment_code_wrapper_operators_are_left_alone() {
    let source = "\
;BEGIN FRAGMENT CODE - Do not edit anything between this and the end comment
Function Fragment_0(Int a,Int b)
;BEGIN CODE
If a==b
EndIf
;END CODE
EndFunction
;END FRAGMENT CODE - Do not edit anything between this and the begin comment
";
    let diagnostics = check(source);
    assert!(diagnostics.iter().all(|d| d.line == 4));
    assert_eq!(diagnostics.len(), 2);
    let repaired = repair(source);
    assert!(repaired.contains("If a == b\n"));
    assert!(repaired.contains("Function Fragment_0(Int a,Int b)"));
}

#[test]
fn repair_inserts_missing_spaces() {
    assert_eq!(repair("If a==b\nEndIf\n"), "If a == b\nEndIf\n");
}

#[test]
fn repair_collapses_extra_spaces_and_tabs() {
    assert_eq!(repair("If a  ==\tb\nEndIf\n"), "If a == b\nEndIf\n");
}

#[test]
fn repair_normalizes_every_operator_in_a_longer_expression() {
    assert_eq!(
        repair("If a==b&&c>1||d<=2\nEndIf\n"),
        "If a == b && c > 1 || d <= 2\nEndIf\n"
    );
}

#[test]
fn repair_leaves_a_line_continuation_alone() {
    let source = "If a \\\n    &&b\nEndIf\n";
    assert_eq!(repair(source), "If a \\\n    && b\nEndIf\n");
}

#[test]
fn repair_is_idempotent() {
    let repaired = repair("If a  ==  b  &&  c!=d\nEndIf\n");
    assert_eq!(repair(&repaired), repaired);
    assert!(check(&repaired).is_empty());
}
