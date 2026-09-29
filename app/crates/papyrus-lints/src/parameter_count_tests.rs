use super::*;

fn check(source: &str, max: usize) -> Vec<Diagnostic> {
    let ast = papyrus_parser::parse(source).ok();
    let tokens = papyrus_parser::tokenize(source).ok();
    let config = crate::config::Config {
        parameter_count_max: max,
        ..Default::default()
    };
    super::check(
        source,
        ast.as_ref(),
        tokens.as_deref(),
        &config,
        &mut crate::external_signatures::NoExternalSignatures,
    )
}

#[test]
fn does_not_flag_functions_at_or_below_the_maximum() {
    let source = "ScriptName Example\n\nFunction Setup(Actor akTarget, Int aiCount)\nEndFunction\n";
    assert!(check(source, 5).is_empty());
    assert!(check(source, 2).is_empty());
}

#[test]
fn flags_functions_above_the_maximum() {
    let source = "ScriptName Example\n\nFunction Setup(Actor a, ObjectReference b, Form c, Int d, Float e, Bool f)\nEndFunction\n";
    let diagnostics = check(source, 5);
    assert_eq!(diagnostics.len(), 1);
    assert!(diagnostics[0].message.starts_with("[warning]"));
    assert!(diagnostics[0].message.contains("has 6 parameters"));
    assert!(diagnostics[0].message.contains("maximum: 5"));
    assert_eq!(diagnostics[0].rule, RULE);
    assert_eq!(diagnostics[0].line, 3);
}

#[test]
fn counts_defaulted_parameters_toward_the_total() {
    let source =
        "ScriptName Example\n\nFunction Setup(Actor a, ObjectReference b, Form c, Int d = 1, Float e = 2.0, Bool f = false)\nEndFunction\n";
    let diagnostics = check(source, 5);
    assert_eq!(diagnostics.len(), 1);
    assert!(diagnostics[0].message.contains("has 6 parameters"));
}

#[test]
fn does_not_flag_engine_events() {
    let source = "ScriptName Example\n\nEvent OnActivate(ObjectReference akActionRef, Actor a, ObjectReference b, Form c, Int d, Float e)\nEndEvent\n";
    assert!(check(source, 2).is_empty());
}

#[test]
fn respects_the_strict_default_of_five() {
    let source = "ScriptName Example\n\nFunction Setup(Actor a, ObjectReference b, Form c, Int d, Float e, Bool f)\nEndFunction\n";
    let ast = papyrus_parser::parse(source).ok();
    let tokens = papyrus_parser::tokenize(source).ok();
    let diagnostics = super::check(
        source,
        ast.as_ref(),
        tokens.as_deref(),
        &crate::config::Config::default(),
        &mut crate::external_signatures::NoExternalSignatures,
    );
    assert_eq!(diagnostics.len(), 1);
    assert!(diagnostics[0].message.starts_with("[warning]"));
}

#[test]
fn does_not_flag_when_the_rule_is_disabled() {
    let source = "ScriptName Example\n\nFunction Setup(Actor a, ObjectReference b, Form c, Int d, Float e, Bool f)\nEndFunction\n";
    let mut config = crate::config::Config::default();
    config.rules.parameter_count = false;
    assert!(!crate::lint(source, &config)
        .iter()
        .any(|diagnostic| diagnostic.rule == RULE));
}

#[test]
fn honors_disable_line_comments_through_lint() {
    let source = "ScriptName Example\n\nFunction Setup(Actor a, ObjectReference b, Form c, Int d, Float e, Bool f) ; @disable parameter-count\nEndFunction\n";
    assert!(!crate::lint(source, &crate::config::Config::default())
        .iter()
        .any(|diagnostic| diagnostic.rule == RULE));
}
