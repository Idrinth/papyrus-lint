//! Robustness tests for linting incomplete and non-ASCII source through the
//! crate's public API.

use papyrus_lints::{lint, repair_filtered, Config};

fn diagnostics_for<'a>(
    diagnostics: &'a [papyrus_lints::Diagnostic],
    rule: &str,
) -> Vec<&'a papyrus_lints::Diagnostic> {
    diagnostics
        .iter()
        .filter(|diagnostic| diagnostic.rule == rule)
        .collect()
}

#[test]
fn lexical_lints_still_report_findings_in_an_incomplete_function() {
    let source = "ScriptName Example\n\nFunction Run(Int left,Int right)  \n    Call(left,right)\n";
    let diagnostics = lint(source, &Config::default());

    assert_eq!(diagnostics_for(&diagnostics, "comma-spacing").len(), 2);
    assert_eq!(
        diagnostics_for(&diagnostics, "trailing-whitespace")
            .iter()
            .map(|diagnostic| (diagnostic.line, diagnostic.column))
            .collect::<Vec<_>>(),
        [(3, 33)]
    );
}

#[test]
fn comma_repair_ignores_commas_in_strings_and_comments_in_malformed_source() {
    let source = "ScriptName Example\nString value = \"alpha,beta\" ; keep,this\nCall(1,2\n";

    assert_eq!(
        repair_filtered(source, &Config::default(), Some("comma-spacing")),
        "ScriptName Example\nString value = \"alpha,beta\" ; keep,this\nCall(1, 2\n"
    );
}

#[test]
fn text_that_looks_like_a_disable_directive_inside_a_string_is_not_a_directive() {
    let source = "ScriptName Example\n\nFunction Run()\n    String Message = \"; @disable comma-spacing\"\n    Call(1,2)\nEndFunction\n";
    let diagnostics = lint(source, &Config::default());
    let commas = diagnostics_for(&diagnostics, "comma-spacing");

    assert_eq!(commas.len(), 1);
    assert_eq!((commas[0].line, commas[0].column), (5, 11));
}

#[test]
fn a_disable_directive_does_not_leak_to_the_following_line() {
    let source = "ScriptName Example\n\nFunction Run()\n    Call(1,2) ; @disable comma-spacing\n    Call(3,4)\nEndFunction\n";
    let diagnostics = lint(source, &Config::default());
    let commas = diagnostics_for(&diagnostics, "comma-spacing");

    assert_eq!(commas.len(), 1);
    assert_eq!((commas[0].line, commas[0].column), (5, 11));
}

#[test]
fn filtered_repairs_preserve_utf8_and_mixed_line_endings() {
    let source = "ScriptName Example\r\nString Message = \"café,λ\"\nCall(1,2)\r\n";

    assert_eq!(
        repair_filtered(source, &Config::default(), Some("comma-spacing")),
        "ScriptName Example\r\nString Message = \"café,λ\"\nCall(1, 2)\r\n"
    );
}
