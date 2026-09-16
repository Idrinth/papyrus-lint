use super::*;

#[test]
fn ignores_correctly_spaced_operators() {
    let source = "\
ScriptName Example

Function Test(Int a, Int b)
    a = b
    a += b
    a -= b
    a *= b
    a /= b
    a %= b
EndFunction
";
    assert!(check(source).is_empty());
    assert_eq!(repair(source), source);
}

#[test]
fn flags_missing_space_on_both_sides() {
    let diagnostics = check("a=b\n");
    assert_eq!(diagnostics.len(), 2);
    assert!(diagnostics.iter().any(|d| d.message.contains("preceded")));
    assert!(diagnostics.iter().any(|d| d.message.contains("followed")));
}

#[test]
fn flags_missing_space_before_only() {
    let diagnostics = check("a =b\n");
    assert_eq!(diagnostics.len(), 1);
    assert!(diagnostics[0].message.contains("followed"));
}

#[test]
fn flags_missing_space_after_only() {
    let diagnostics = check("a= b\n");
    assert_eq!(diagnostics.len(), 1);
    assert!(diagnostics[0].message.contains("preceded"));
}

#[test]
fn flags_extra_spaces_and_tabs() {
    let diagnostics = check("a  =\tb\n");
    assert_eq!(diagnostics.len(), 2);
}

#[test]
fn checks_every_relevant_operator_kind() {
    for source in ["a+=b\n", "a-=b\n", "a*=b\n", "a/=b\n", "a%=b\n"] {
        let diagnostics = check(source);
        assert_eq!(diagnostics.len(), 2, "source: {source:?}");
    }
}

#[test]
fn ignores_unrelated_operators() {
    assert!(check("If a==b\nEndIf\nIf a!=b\nEndIf\nIf a>=b\nEndIf\nIf a<=b\nEndIf\n").is_empty());
}

#[test]
fn does_not_flag_a_statement_continued_across_lines() {
    let source = "a \\\n    = b\n";
    assert!(check(source).is_empty());
    assert_eq!(repair(source), source);
}

#[test]
fn fragment_code_wrapper_operators_are_left_alone() {
    let source = "\
;BEGIN FRAGMENT CODE - Do not edit anything between this and the end comment
Function Fragment_0(Int a,Int b)
;BEGIN CODE
a=b
;END CODE
EndFunction
;END FRAGMENT CODE - Do not edit anything between this and the begin comment
";
    let diagnostics = check(source);
    assert!(diagnostics.iter().all(|d| d.line == 4));
    assert_eq!(diagnostics.len(), 2);
    let repaired = repair(source);
    assert!(repaired.contains("a = b\n"));
    assert!(repaired.contains("Function Fragment_0(Int a,Int b)"));
}

#[test]
fn repair_inserts_missing_spaces() {
    assert_eq!(repair("a=b\n"), "a = b\n");
}

#[test]
fn repair_collapses_extra_spaces_and_tabs() {
    assert_eq!(repair("a  =\tb\n"), "a = b\n");
}

#[test]
fn repair_normalizes_every_operator_in_a_longer_script() {
    assert_eq!(
        repair("a=b\na+=1\na-=1\na*=2\na/=2\na%=2\n"),
        "a = b\na += 1\na -= 1\na *= 2\na /= 2\na %= 2\n"
    );
}

#[test]
fn repair_leaves_a_line_continuation_alone() {
    let source = "a \\\n    =b\n";
    assert_eq!(repair(source), "a \\\n    = b\n");
}

#[test]
fn repair_does_not_touch_comparison_operators() {
    let source = "If a==b\nEndIf\n";
    assert_eq!(repair(source), source);
}

#[test]
fn repair_is_idempotent() {
    let repaired = repair("a  =  b\na+=\t1\n");
    assert_eq!(repair(&repaired), repaired);
    assert!(check(&repaired).is_empty());
}
