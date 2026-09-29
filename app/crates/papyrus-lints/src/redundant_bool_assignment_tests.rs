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

const HEADER: &str = "ScriptName Example\n\n";

fn function(body: &str) -> String {
    format!("{HEADER}Function Test(Int x, Bool ready)\n{body}EndFunction\n")
}

#[test]
fn flags_if_else_that_only_assigns_true_and_false() {
    let source = function(
        "    Bool bResult = false\n    If x > 5\n        bResult = true\n    Else\n        bResult = false\n    EndIf\n",
    );
    let diagnostics = check(&source);

    assert_eq!(diagnostics.len(), 1);
    assert_eq!(diagnostics[0].rule, RULE);
    assert_eq!(diagnostics[0].line, 5);
    assert!(diagnostics[0].message.starts_with("[info]"));
}

#[test]
fn repairs_to_a_direct_boolean_assignment() {
    let source = function(
        "    Bool bResult = false\n    If x > 5\n        bResult = true\n    Else\n        bResult = false\n    EndIf\n",
    );

    assert_eq!(
        repair(&source),
        function("    Bool bResult = false\n    bResult = x > 5\n")
    );
}

#[test]
fn repairs_inverted_true_in_else_by_inverting_comparison() {
    let source = function(
        "    Bool bResult = false\n    If x > 5\n        bResult = false\n    Else\n        bResult = true\n    EndIf\n",
    );

    assert_eq!(
        repair(&source),
        function("    Bool bResult = false\n    bResult = x <= 5\n")
    );
}

#[test]
fn unwraps_a_negated_condition_when_inverted() {
    let source = function(
        "    Bool bResult = false\n    If ! ready\n        bResult = false\n    Else\n        bResult = true\n    EndIf\n",
    );

    assert_eq!(
        repair(&source),
        function("    Bool bResult = false\n    bResult = ready\n")
    );
}

#[test]
fn leaves_branches_with_extra_statements_alone() {
    let source = function(
        "    Bool bResult = false\n    If x > 5\n        bResult = true\n        Debug.Trace(\"hi\")\n    Else\n        bResult = false\n    EndIf\n",
    );

    assert!(check(&source).is_empty());
    assert_eq!(repair(&source), source);
}

#[test]
fn leaves_elseif_chains_alone() {
    let source = function(
        "    Bool bResult = false\n    If x > 5\n        bResult = true\n    ElseIf x > 0\n        bResult = false\n    Else\n        bResult = false\n    EndIf\n",
    );

    assert!(check(&source).is_empty());
}

#[test]
fn leaves_missing_else_alone() {
    let source = function(
        "    Bool bResult = false\n    If x > 5\n        bResult = true\n    EndIf\n",
    );

    assert!(check(&source).is_empty());
}

#[test]
fn leaves_different_targets_alone() {
    let source = function(
        "    Bool a = false\n    Bool b = false\n    If x > 5\n        a = true\n    Else\n        b = false\n    EndIf\n",
    );

    assert!(check(&source).is_empty());
}

#[test]
fn leaves_same_literal_on_both_sides_alone() {
    let source = function(
        "    Bool bResult = false\n    If x > 5\n        bResult = true\n    Else\n        bResult = true\n    EndIf\n",
    );

    assert!(check(&source).is_empty());
}

#[test]
fn matches_target_names_case_insensitively() {
    let source = function(
        "    Bool bResult = false\n    If ready\n        bResult = true\n    Else\n        BRESULT = false\n    EndIf\n",
    );

    assert_eq!(check(&source).len(), 1);
    assert_eq!(
        repair(&source),
        function("    Bool bResult = false\n    bResult = ready\n")
    );
}

#[test]
fn flags_member_targets() {
    let source = "\
ScriptName Example

Bool Property Flag Auto

Function Test(Int x)
    If x > 5
        Self.Flag = true
    Else
        Self.Flag = false
    EndIf
EndFunction
";
    let diagnostics = check(source);

    assert_eq!(diagnostics.len(), 1);
    assert_eq!(
        repair(source),
        "\
ScriptName Example

Bool Property Flag Auto

Function Test(Int x)
    Self.Flag = x > 5
EndFunction
"
    );
}

#[test]
fn respects_line_and_file_disable_comments() {
    let line_disabled = crate::lint(
        "ScriptName Example\n\nFunction Test(Int x)\n    Bool bResult = false\n    If x > 5 ; @disable redundant-bool-assignment\n        bResult = true\n    Else\n        bResult = false\n    EndIf\nEndFunction\n",
        &crate::config::Config::default(),
    );
    let file_disabled = crate::lint(
        "; @disable-file redundant-bool-assignment\nScriptName Example\n\nFunction Test(Int x)\n    Bool bResult = false\n    If x > 5\n        bResult = true\n    Else\n        bResult = false\n    EndIf\nEndFunction\n",
        &crate::config::Config::default(),
    );

    assert!(line_disabled
        .iter()
        .all(|diagnostic| diagnostic.line != 5 || diagnostic.rule != RULE));
    assert!(file_disabled
        .iter()
        .all(|diagnostic| diagnostic.rule != RULE));

    let source = "ScriptName Example\n\nFunction Test(Int x)\n    Bool bResult = false\n    If x > 5 ; @disable redundant-bool-assignment\n        bResult = true\n    Else\n        bResult = false\n    EndIf\nEndFunction\n";
    assert_eq!(repair(source), source);
}

#[test]
fn config_off_disables_the_rule() {
    let source = function(
        "    Bool bResult = false\n    If x > 5\n        bResult = true\n    Else\n        bResult = false\n    EndIf\n",
    );
    let mut config = crate::config::Config::default();
    config.rules.redundant_bool_assignment = false;
    let diagnostics = crate::lint(&source, &config);
    assert!(diagnostics
        .iter()
        .all(|diagnostic| diagnostic.rule != RULE));
}

#[test]
fn checks_functions_declared_in_states_too() {
    let source = "\
ScriptName Example

State Active
    Function Test(Int x)
        Bool bResult = false
        If x > 5
            bResult = true
        Else
            bResult = false
        EndIf
    EndFunction
EndState
";
    assert_eq!(check(source).len(), 1);
    assert!(repair(source).contains("bResult = x > 5"));
}

#[test]
fn leaves_non_bool_literals_alone() {
    let source = function(
        "    Int n = 0\n    If x > 5\n        n = 1\n    Else\n        n = 0\n    EndIf\n",
    );

    assert!(check(&source).is_empty());
}

#[test]
fn repairs_every_inverted_comparison_operator() {
    for (condition, inverted) in [
        ("x == 5", "x != 5"),
        ("x != 5", "x == 5"),
        ("x > 5", "x <= 5"),
        ("x < 5", "x >= 5"),
        ("x >= 5", "x < 5"),
        ("x <= 5", "x > 5"),
    ] {
        let source = function(&format!(
            "    Bool bResult = false\n    If {condition}\n        bResult = false\n    Else\n        bResult = true\n    EndIf\n"
        ));

        assert_eq!(
            repair(&source),
            function(&format!(
                "    Bool bResult = false\n    bResult = {inverted}\n"
            )),
            "failed to invert {condition}"
        );
    }
}

#[test]
fn parenthesizes_an_inverted_compound_condition() {
    let source = function(
        "    Bool bResult = false\n    If ready && x > 5\n        bResult = false\n    Else\n        bResult = true\n    EndIf\n",
    );

    assert_eq!(
        repair(&source),
        function("    Bool bResult = false\n    bResult = ! (ready && x > 5)\n")
    );
}

#[test]
fn repairs_indexed_targets_and_nested_control_flow() {
    let source = "\
ScriptName Example

Function Test(Int x, Bool ready, Bool[] flags)
    While ready
        If x > 5
            flags[x] = true
        Else
            FLAGS[x] = false
        EndIf
    EndWhile
EndFunction
";

    assert_eq!(check(source).len(), 1);
    assert_eq!(
        repair(source),
        "\
ScriptName Example

Function Test(Int x, Bool ready, Bool[] flags)
    While ready
        flags[x] = x > 5
    EndWhile
EndFunction
"
    );
}

#[test]
fn repair_helpers_cover_malformed_and_boundary_inputs() {
    assert_eq!(strip_outer_parens(" ((ready)) "), "(ready)");
    assert_eq!(strip_outer_parens("(ready) || other"), "(ready) || other");
    assert_eq!(strip_outer_parens("(ready"), "(ready");
    assert_eq!(strip_one_not(" != ready"), None);
    assert_eq!(strip_one_not("! (ready)"), Some(" (ready)"));

    assert!(!needs_parens("flags[index]"));
    assert!(needs_parens("ready != false"));
    assert!(needs_parens("x + 1"));

    assert_eq!(lexeme_end("\"a\\\"b\" tail", 0), 6);
    assert_eq!(lexeme_end("\"unterminated", 0), 13);
    assert_eq!(lexeme_end("123abc", 0), 3);
    assert_eq!(lexeme_end("name_12 tail", 0), 7);
    assert_eq!(lexeme_end("!= value", 0), 2);
    assert_eq!(lexeme_end("&& value", 0), 2);
    assert_eq!(lexeme_end(".value", 0), 1);
    assert_eq!(lexeme_end("short", 20), 20);

    let hits = vec![
        Hit {
            if_line: 1,
            start: 3,
            end: 6,
            replacement: "XYZ".to_string(),
        },
        Hit {
            if_line: 1,
            start: 0,
            end: 2,
            replacement: "AB".to_string(),
        },
    ];
    assert_eq!(apply_edits("012345", &hits), "AB2XYZ");

    let invalid_hits = vec![Hit {
        if_line: 1,
        start: 4,
        end: 3,
        replacement: "ignored".to_string(),
    }];
    assert_eq!(apply_edits("012345", &invalid_hits), "012345");
}
