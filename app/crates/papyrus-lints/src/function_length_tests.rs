use super::*;

fn check(source: &str, max_code: usize, max_lines: usize) -> Vec<Diagnostic> {
    let ast = papyrus_parser::parse(source).ok();
    let tokens = papyrus_parser::tokenize(source).ok();
    let config = crate::config::Config {
        function_length_max_code_lines: max_code,
        function_length_max_lines: max_lines,
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

fn function_with_code_lines(count: usize) -> String {
    let mut source = String::from("ScriptName Example\n\nFunction Test()\n");
    for index in 0..count {
        source.push_str(&format!("    Int x{index} = {index}\n"));
    }
    source.push_str("EndFunction\n");
    source
}

#[test]
fn does_not_flag_bodies_at_or_below_both_maxima() {
    let source = function_with_code_lines(3);
    assert!(check(&source, 3, 3).is_empty());
    assert!(check(&source, 10, 20).is_empty());
}

#[test]
fn flags_when_code_lines_exceed_the_maximum() {
    let source = function_with_code_lines(4);
    let diagnostics = check(&source, 3, 40);
    assert_eq!(diagnostics.len(), 1);
    assert!(diagnostics[0].message.starts_with("[warning]"));
    assert!(diagnostics[0].message.contains("Function 'Test'"));
    assert!(diagnostics[0].message.contains("has 4 lines of code"));
    assert!(diagnostics[0].message.contains("maximum: 3"));
    assert!(!diagnostics[0].message.contains("and 4 lines"));
    assert_eq!(diagnostics[0].rule, RULE);
    assert_eq!(diagnostics[0].line, 3);
}

#[test]
fn flags_when_physical_lines_exceed_the_maximum() {
    let source = "ScriptName Example\n\nFunction Test()\n    ; padding\n\n    ; more padding\n    ; still padding\nEndFunction\n";
    let diagnostics = check(source, 10, 3);
    assert_eq!(diagnostics.len(), 1);
    assert!(diagnostics[0].message.contains("has 4 lines (maximum: 3)"));
    assert!(!diagnostics[0].message.contains("lines of code"));
}

#[test]
fn flags_both_metrics_in_one_finding() {
    let source = function_with_code_lines(5);
    let diagnostics = check(&source, 2, 3);
    assert_eq!(diagnostics.len(), 1);
    assert!(diagnostics[0]
        .message
        .contains("has 5 lines of code (maximum: 2) and 5 lines (maximum: 3)"));
}

#[test]
fn comments_and_blanks_do_not_count_as_code_lines() {
    let source = "ScriptName Example\n\nFunction Test()\n    Int x = 1\n\n    ; a note\n    {doc}\n    ;/ block\n       still block /;\n    Int y = 2\nEndFunction\n";
    assert!(check(source, 2, 20).is_empty());
    let diagnostics = check(source, 1, 20);
    assert_eq!(diagnostics.len(), 1);
    assert!(diagnostics[0].message.contains("has 2 lines of code"));
}

#[test]
fn code_on_a_commented_line_still_counts() {
    let source = "ScriptName Example\n\nFunction Test()\n    Int x = 1 ; trailing\n    Int y = 2\nEndFunction\n";
    let diagnostics = check(source, 1, 20);
    assert_eq!(diagnostics.len(), 1);
    assert!(diagnostics[0].message.contains("has 2 lines of code"));
}

#[test]
fn checks_events_too() {
    let source = "ScriptName Example\n\nEvent OnInit()\n    Int a = 1\n    Int b = 2\n    Int c = 3\nEndEvent\n";
    let diagnostics = check(source, 2, 20);
    assert_eq!(diagnostics.len(), 1);
    assert!(diagnostics[0].message.contains("Event 'OnInit'"));
    assert!(diagnostics[0].message.contains("has 3 lines of code"));
}

#[test]
fn skips_native_stubs() {
    let source = "ScriptName Example\n\nFunction Hidden() Native\n";
    assert!(check(source, 0, 0).is_empty());
}

#[test]
fn skips_creationkit_fragment_wrappers() {
    let source = "ScriptName Example\n\n;BEGIN FRAGMENT CODE\nFunction Fragment_0()\n;BEGIN CODE\n    Int a = 1\n    Int b = 2\n    Int c = 3\n;END CODE\nEndFunction\n;END FRAGMENT CODE\n";
    assert!(check(source, 1, 1).is_empty());
}

#[test]
fn respects_the_strict_defaults() {
    let source = function_with_code_lines(11);
    let ast = papyrus_parser::parse(&source).ok();
    let tokens = papyrus_parser::tokenize(&source).ok();
    let diagnostics = super::check(
        &source,
        ast.as_ref(),
        tokens.as_deref(),
        &crate::config::Config::default(),
        &mut crate::external_signatures::NoExternalSignatures,
    );
    assert_eq!(diagnostics.len(), 1);
    assert!(diagnostics[0].message.contains("has 11 lines of code"));
    assert!(diagnostics[0].message.contains("maximum: 10"));
}

#[test]
fn does_not_flag_when_the_rule_is_disabled() {
    let source = function_with_code_lines(11);
    let mut config = crate::config::Config::default();
    config.rules.function_length = false;
    assert!(!crate::lint(&source, &config)
        .iter()
        .any(|diagnostic| diagnostic.rule == RULE));
}

#[test]
fn honors_disable_line_comments_through_lint() {
    let source = "ScriptName Example\n\nFunction Test() ; @disable function-length\n    Int a = 1\n    Int b = 2\n    Int c = 3\nEndFunction\n";
    let mut config = crate::config::Config::default();
    config.function_length_max_code_lines = 1;
    config.function_length_max_lines = 1;
    assert!(!crate::lint(source, &config)
        .iter()
        .any(|diagnostic| diagnostic.rule == RULE));
}
