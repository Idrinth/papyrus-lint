//! Robustness tests for linting incomplete and non-ASCII source through the
//! crate's public API.

use papyrus_lints::{lint, repair, repair_filtered, restrict_to_line, Config, FIXABLE_RULE_IDS};

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

#[test]
fn lexical_diagnostics_keep_correct_coordinates_with_crlf_input() {
    let source = "ScriptName Example\r\n\r\nFunction Run(Int left,Int right)  \r\nEndFunction\r\n";
    let diagnostics = lint(source, &Config::default());

    let commas = diagnostics_for(&diagnostics, "comma-spacing");
    assert_eq!(commas.len(), 1);
    assert_eq!((commas[0].line, commas[0].column), (3, 22));

    let trailing = diagnostics_for(&diagnostics, "trailing-whitespace");
    assert_eq!(trailing.len(), 1);
    assert_eq!((trailing[0].line, trailing[0].column), (3, 33));
}

#[test]
fn a_block_comment_cannot_trigger_or_suppress_comma_spacing() {
    let source = "ScriptName Example\n{ Call(1,2) ; @disable comma-spacing }\nCall(3,4)\n";
    let diagnostics = lint(source, &Config::default());
    let commas = diagnostics_for(&diagnostics, "comma-spacing");

    assert_eq!(commas.len(), 1);
    assert_eq!((commas[0].line, commas[0].column), (3, 7));
    assert_eq!(
        repair_filtered(source, &Config::default(), Some("comma-spacing")),
        "ScriptName Example\n{ Call(1,2) ; @disable comma-spacing }\nCall(3, 4)\n"
    );
}

#[test]
fn every_filtered_fixer_is_idempotent_on_its_own_output() {
    let mut config = Config::default();
    config.rules.property_sorting = true;
    let source = "ScriptName my_script  \n\nInt Property Zulu Auto\nActor Property Alpha Auto\n\nFunction do_thing(Int first,Int second);\n  If !ready&&first==second\n    Value . SetValueInt(3)\n  EndIf\nEndFunction\n";

    for rule in FIXABLE_RULE_IDS {
        let once = repair_filtered(source, &config, Some(rule));
        let twice = repair_filtered(&once, &config, Some(rule));
        assert_eq!(twice, once, "{rule} repair was not idempotent");
    }
}

#[test]
fn combined_repair_reaches_a_fixed_point() {
    let source = "ScriptName my_script  \n\nFunction do_thing(Int first,Int second);\n  If !ready&&first==second\n    Value . SetValueInt(3)  \n  EndIf\nEndFunction\n";
    let repaired = repair(source, &Config::default());

    assert_ne!(repaired, source, "fixture must exercise automatic fixes");
    assert_eq!(repair(&repaired, &Config::default()), repaired);
}

#[test]
fn line_restriction_preserves_a_utf8_line_byte_for_byte() {
    let original = "String Message = \"café λ\"\nCall(1,2)\n";
    let repaired = repair_filtered(original, &Config::default(), Some("comma-spacing"));

    assert_eq!(
        restrict_to_line(original, &repaired, 2),
        Some("String Message = \"café λ\"\nCall(1, 2)\n".to_string())
    );
}
