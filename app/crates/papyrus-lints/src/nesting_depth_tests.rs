use super::*;

fn check(source: &str, info: usize, warning: usize, error: usize) -> Vec<Diagnostic> {
    let ast = papyrus_parser::parse(source).ok();
    let tokens = papyrus_parser::tokenize(source).ok();
    let config = crate::config::Config {
        nesting_depth_info: info,
        nesting_depth_warning: warning,
        nesting_depth_error: error,
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

fn nested_ifs(levels: usize) -> String {
    let mut source = String::from("ScriptName Example\n\nFunction Test()\n");
    for level in 0..levels {
        source.push_str(&format!("{}If a{level}\n", "    ".repeat(level + 1)));
    }
    source.push_str(&format!("{}Int x = 1\n", "    ".repeat(levels + 1)));
    for level in (0..levels).rev() {
        source.push_str(&format!("{}EndIf\n", "    ".repeat(level + 1)));
    }
    source.push_str("EndFunction\n");
    source
}

#[test]
fn does_not_flag_below_info_threshold() {
    assert!(check(&nested_ifs(3), 4, 6, 9).is_empty());
}

#[test]
fn flags_info_at_info_threshold() {
    let diagnostics = check(&nested_ifs(4), 4, 6, 9);

    assert_eq!(diagnostics.len(), 1);
    assert!(diagnostics[0].message.starts_with("[info]"));
    assert!(diagnostics[0].message.contains("nesting depth of 4"));
    assert_eq!(diagnostics[0].line, 3);
    assert_eq!(diagnostics[0].rule, RULE);
}

#[test]
fn flags_warning_at_warning_threshold() {
    let diagnostics = check(&nested_ifs(6), 4, 6, 9);

    assert_eq!(diagnostics.len(), 1);
    assert!(diagnostics[0].message.starts_with("[warning]"));
    assert!(diagnostics[0].message.contains("nesting depth of 6"));
}

#[test]
fn flags_error_at_error_threshold() {
    let diagnostics = check(&nested_ifs(9), 4, 6, 9);

    assert_eq!(diagnostics.len(), 1);
    assert!(diagnostics[0].message.starts_with("[error]"));
    assert!(diagnostics[0].message.contains("nesting depth of 9"));
}

#[test]
fn default_thresholds_stay_quiet_for_shallow_guards() {
    let source = "ScriptName Example\n\nFunction Test()\n    If ready\n        Return\n    EndIf\n    If other\n        Return\n    EndIf\nEndFunction\n";

    assert!(check(source, 4, 6, 9).is_empty());
}

#[test]
fn else_does_not_add_a_nesting_level() {
    let source = "ScriptName Example\n\nFunction Test()\n    If a\n        If b\n            If c\n                Int x = 1\n            EndIf\n        EndIf\n    Else\n        If d\n            If e\n                Int y = 2\n            EndIf\n        EndIf\n    EndIf\nEndFunction\n";

    // Deepest path is If a / If b / If c (or the Else side If d / If e): 3.
    assert!(check(source, 4, 6, 9).is_empty());
    let diagnostics = check(source, 3, 6, 9);
    assert_eq!(diagnostics.len(), 1);
    assert!(diagnostics[0].message.contains("nesting depth of 3"));
}

#[test]
fn elseif_is_the_same_level_as_if() {
    let source = "ScriptName Example\n\nFunction Test()\n    If a\n        Int x = 1\n    ElseIf b\n        If c\n            If d\n                Int y = 2\n            EndIf\n        EndIf\n    EndIf\nEndFunction\n";

    // If a and ElseIf b are depth 1; If c is 2; If d is 3.
    let diagnostics = check(source, 3, 6, 9);
    assert_eq!(diagnostics.len(), 1);
    assert!(diagnostics[0].message.contains("nesting depth of 3"));
}

#[test]
fn counts_while_loops() {
    let source = "ScriptName Example\n\nFunction Test()\n    While a\n        While b\n            While c\n                While d\n                    Int x = 1\n                EndWhile\n            EndWhile\n        EndWhile\n    EndWhile\nEndFunction\n";

    let diagnostics = check(source, 4, 6, 9);
    assert_eq!(diagnostics.len(), 1);
    assert!(diagnostics[0].message.contains("nesting depth of 4"));
}

#[test]
fn state_declaration_does_not_add_depth() {
    let source = "ScriptName Example\n\nState Active\n    Function Test()\n        If a\n            If b\n                If c\n                    If d\n                        Int x = 1\n                    EndIf\n                EndIf\n            EndIf\n        EndIf\n    EndFunction\nEndState\n";

    let diagnostics = check(source, 4, 6, 9);
    assert_eq!(diagnostics.len(), 1);
    assert!(diagnostics[0].message.contains("'Test'"));
    assert!(diagnostics[0].message.contains("nesting depth of 4"));
}

#[test]
fn checks_events_too() {
    let source = "ScriptName Example\n\nEvent OnInit()\n    If a\n        If b\n            If c\n                If d\n                    Int x = 1\n                EndIf\n            EndIf\n        EndIf\n    EndIf\nEndEvent\n";

    let diagnostics = check(source, 4, 6, 9);
    assert_eq!(diagnostics.len(), 1);
    assert!(diagnostics[0].message.contains("Event 'OnInit'"));
}

#[test]
fn treats_inverted_thresholds_as_non_decreasing() {
    let diagnostics = check(&nested_ifs(4), 4, 2, 1);

    assert_eq!(diagnostics.len(), 1);
    assert!(diagnostics[0].message.starts_with("[error]"));
    assert!(diagnostics[0]
        .message
        .contains("(info: 4, warning: 4, error: 4)"));
}

#[test]
fn treats_info_of_zero_as_one() {
    let source = "ScriptName Example\n\nFunction Test()\n    Int x = 1\nEndFunction\n";

    // Depth 0 stays quiet even if the configured info threshold is 0.
    assert!(check(source, 0, 6, 9).is_empty());

    let diagnostics = check(
        "ScriptName Example\n\nFunction Test()\n    If a\n        Int x = 1\n    EndIf\nEndFunction\n",
        0,
        6,
        9,
    );
    assert_eq!(diagnostics.len(), 1);
    assert!(diagnostics[0].message.contains("(info: 1, warning: 6, error: 9)"));
}

#[test]
fn does_not_crash_on_unparseable_source() {
    assert!(check(
        "ScriptName Example\n\nFunction Test(\nEndFunction\n",
        4,
        6,
        9
    )
    .is_empty());
}

#[test]
fn reports_each_over_threshold_function_independently() {
    let source = "ScriptName Example\n\nFunction First()\n    If a\n        If b\n            If c\n                If d\n                    Int x = 1\n                EndIf\n            EndIf\n        EndIf\n    EndIf\nEndFunction\n\nEvent OnInit()\n    While a\n        While b\n            While c\n                While d\n                    Int y = 1\n                EndWhile\n            EndWhile\n        EndWhile\n    EndWhile\nEndEvent\n";

    let diagnostics = check(source, 4, 6, 9);

    assert_eq!(diagnostics.len(), 2);
    assert!(diagnostics[0].message.contains("'First'"));
    assert!(diagnostics[1].message.contains("'OnInit'"));
}
