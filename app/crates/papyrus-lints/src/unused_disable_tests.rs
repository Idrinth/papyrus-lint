use super::*;
use crate::disable_comments::Disables;

const KNOWN_RULES: &[&str] = &["comma-spacing", "trailing-whitespace"];

fn diagnostic(line: usize, rule: &'static str) -> Diagnostic {
    Diagnostic {
        line,
        column: 1,
        message: "[warning] test diagnostic".into(),
        rule,
    }
}

#[test]
fn bare_disable_is_used_when_any_diagnostic_occurs_on_the_line() {
    let disables = Disables::scan("Call(1,2) ; @disable\n");
    let diagnostics = [diagnostic(1, "comma-spacing")];

    assert!(check(&disables, &diagnostics, KNOWN_RULES).is_empty());
}

#[test]
fn bare_disable_is_unused_when_diagnostic_occurs_on_another_line() {
    let disables = Disables::scan("; @disable\nCall(1,2)\n");
    let diagnostics = [diagnostic(2, "comma-spacing")];

    let unused = check(&disables, &diagnostics, KNOWN_RULES);

    assert_eq!(unused.len(), 1);
    assert_eq!((unused[0].line, unused[0].column), (1, 3));
    assert!(unused[0]
        .message
        .contains("does not produce any diagnostics"));
}

#[test]
fn named_disable_matches_rule_ids_case_insensitively() {
    let disables = Disables::scan("Call(1,2) ; @disable COMMA-SPACING\n");
    let diagnostics = [diagnostic(1, "comma-spacing")];

    assert!(check(&disables, &diagnostics, KNOWN_RULES).is_empty());
}

#[test]
fn named_disables_distinguish_unknown_and_untriggered_rules() {
    let disables = Disables::scan("Call(1, 2) ; @disable mystery-rule, trailing-whitespace\n");

    let unused = check(&disables, &[], KNOWN_RULES);

    assert_eq!(unused.len(), 2);
    assert_eq!(unused[0].column, 23);
    assert!(unused[0].message.contains("mystery-rule"));
    assert!(unused[0].message.contains("unknown"));
    assert_eq!(unused[1].column, 37);
    assert!(unused[1].message.contains("trailing-whitespace"));
    assert!(unused[1].message.contains("does not produce"));
}

#[test]
fn reports_are_sorted_by_source_location() {
    let disables = Disables::scan(
        "; @disable trailing-whitespace, mystery\nclean\n; @disable comma-spacing\n",
    );

    let unused = check(&disables, &[], KNOWN_RULES);
    let locations: Vec<_> = unused
        .iter()
        .map(|diagnostic| (diagnostic.line, diagnostic.column))
        .collect();

    assert_eq!(locations, vec![(1, 12), (1, 33), (3, 12)]);
    assert!(unused.iter().all(|diagnostic| diagnostic.rule == RULE));
}

#[test]
fn named_disable_is_used_only_by_a_matching_diagnostic_on_the_same_line() {
    let disables = Disables::scan("Call(1,2) ; @disable comma-spacing\n");
    let diagnostics = [diagnostic(1, "trailing-whitespace")];

    let unused = check(&disables, &diagnostics, KNOWN_RULES);

    assert_eq!(unused.len(), 1);
    assert!(unused[0].message.contains("comma-spacing"));
    assert!(unused[0].message.contains("does not produce"));
}

#[test]
fn duplicate_named_disables_produce_only_one_report() {
    let disables =
        Disables::scan("Call(1, 2) ; @disable trailing-whitespace, TRAILING-WHITESPACE\n");

    let unused = check(&disables, &[], KNOWN_RULES);

    assert_eq!(unused.len(), 1);
    assert!(unused[0].message.contains("trailing-whitespace"));
}

#[test]
fn unknown_rule_is_reported_even_when_its_name_matches_a_diagnostic() {
    let disables = Disables::scan("Call() ; @disable made-up-rule\n");
    let diagnostics = [diagnostic(1, "made-up-rule")];

    let unused = check(&disables, &diagnostics, KNOWN_RULES);

    assert_eq!(unused.len(), 1);
    assert!(unused[0].message.contains("unknown"));
}

#[test]
fn bare_disable_file_is_used_when_any_diagnostic_occurs_anywhere_in_the_file() {
    let disables = Disables::scan("; @disable-file\nCall(1,2)\n");
    let diagnostics = [diagnostic(2, "comma-spacing")];

    assert!(check(&disables, &diagnostics, KNOWN_RULES).is_empty());
}

#[test]
fn bare_disable_file_is_unused_when_the_file_has_no_diagnostics() {
    let disables = Disables::scan("; @disable-file\nCall(1, 2)\n");

    let unused = check(&disables, &[], KNOWN_RULES);

    assert_eq!(unused.len(), 1);
    assert!(unused[0]
        .message
        .contains("this file does not produce any diagnostics"));
}

#[test]
fn named_disable_file_is_used_by_a_matching_diagnostic_on_any_line() {
    let disables = Disables::scan("Call(1,2)\n; @disable-file comma-spacing\nCall(3,4)\n");
    let diagnostics = [diagnostic(3, "comma-spacing")];

    assert!(check(&disables, &diagnostics, KNOWN_RULES).is_empty());
}

#[test]
fn named_disable_file_distinguishes_unknown_and_untriggered_rules() {
    let disables = Disables::scan("; @disable-file mystery-rule, trailing-whitespace\n");

    let unused = check(&disables, &[], KNOWN_RULES);

    assert_eq!(unused.len(), 2);
    assert!(unused[0].message.contains("mystery-rule"));
    assert!(unused[0].message.contains("unknown"));
    assert!(unused[1].message.contains("trailing-whitespace"));
    assert!(unused[1]
        .message
        .contains("this file does not produce a diagnostic"));
}

#[test]
fn repair_drops_an_unused_named_disable() {
    let source = "ScriptName Example\nFunction Test()\n    Int x = 1 ; @disable mystery-rule, trailing-whitespace\nEndFunction\n";
    let mut config = crate::config::Config::default();
    config.rules.unused_disable = true;
    let repaired = super::repair(source, None, None, &config);
    assert!(!repaired.contains("mystery-rule"));
    assert!(!repaired.contains("trailing-whitespace"));
    assert!(!repaired.contains("@disable"));
}

#[test]
fn repair_keeps_a_disable_that_still_applies() {
    let source = "Call(1,2) ; @disable comma-spacing, mystery-rule\n";
    let mut config = crate::config::Config::default();
    config.rules.unused_disable = true;
    let repaired = super::repair(source, None, None, &config);
    assert!(repaired.contains("@disable comma-spacing"));
    assert!(!repaired.contains("mystery-rule"));
}

#[test]
fn named_disable_file_matches_rule_ids_case_insensitively() {
    let disables = Disables::scan("; @disable-file COMMA-SPACING\nCall(1,2)\n");
    let diagnostics = [diagnostic(2, "comma-spacing")];

    assert!(check(&disables, &diagnostics, KNOWN_RULES).is_empty());
}

#[test]
fn repair_returns_an_unchanged_source_when_there_are_no_unused_directives() {
    let source = "ScriptName Example\r\n";

    assert_eq!(
        super::repair(source, None, None, &crate::config::Config::default()),
        source
    );
}

#[test]
fn repair_preserves_line_endings_and_a_missing_final_newline() {
    let source = "; @file-disable mystery-rule\r\nScriptName Example";
    let repaired = super::repair(source, None, None, &crate::config::Config::default());

    assert_eq!(repaired, "\r\nScriptName Example");
}

#[test]
fn rewrite_removes_only_unused_ids_and_normalizes_the_remaining_list() {
    let line = "Call(1,2) ; @disable MYSTERY-rule, comma-spacing trailing-whitespace";
    let unused = vec!["mystery-rule".to_string(), "TRAILING-WHITESPACE".to_string()];

    assert_eq!(
        rewrite_disable_line(line, &unused),
        "Call(1,2) ; @disable comma-spacing"
    );
}

#[test]
fn rewrite_removes_a_whole_directive_without_damaging_surrounding_comments() {
    let unused = vec!["mystery-rule".to_string()];

    assert_eq!(
        rewrite_disable_line(
            "Int value = 1 ; @disable mystery-rule @nodiscard",
            &unused
        ),
        "Int value = 1 ; @nodiscard"
    );
    assert_eq!(
        rewrite_disable_line("; @disable mystery-rule @nodiscard", &unused),
        "; @nodiscard"
    );
    assert_eq!(
        rewrite_disable_line("Int value = 1 ; @disable mystery-rule", &unused),
        "Int value = 1"
    );
    assert_eq!(
        rewrite_disable_line("; @disable mystery-rule", &unused),
        ""
    );
}

#[test]
fn rewrite_handles_bare_directives_aliases_and_unrelated_lines() {
    assert_eq!(
        rewrite_disable_line("; @disable", &["*".to_string()]),
        ""
    );
    assert_eq!(
        rewrite_disable_line(
            "; @file-disable mystery-rule, comma-spacing",
            &["mystery-rule".to_string()]
        ),
        "; @file-disable comma-spacing"
    );
    assert_eq!(
        rewrite_disable_line("Int value = 1 ; ordinary comment", &["*".to_string()]),
        "Int value = 1 ; ordinary comment"
    );
}

#[test]
fn unused_rule_id_extracts_named_and_bare_directives() {
    assert_eq!(
        unused_rule_id("[warning] Unused @disable `Mixed-Case`: reason"),
        "mixed-case"
    );
    assert_eq!(
        unused_rule_id("[warning] Unused @disable-file `some-rule`: reason"),
        "some-rule"
    );
    assert_eq!(
        unused_rule_id("[warning] Unused @disable: no diagnostics"),
        "*"
    );
    assert_eq!(unused_rule_id("unexpected message"), "*");
}

#[test]
fn unused_ids_are_grouped_by_source_line() {
    let diagnostics = [
        Diagnostic {
            line: 3,
            column: 4,
            message: "[warning] Unused @disable `first-rule`: reason".into(),
            rule: RULE,
        },
        Diagnostic {
            line: 3,
            column: 20,
            message: "[warning] Unused @disable-file `SECOND-RULE`: reason".into(),
            rule: RULE,
        },
    ];

    let grouped = unused_ids_by_line(&diagnostics);

    assert_eq!(
        grouped.get(&3),
        Some(&vec!["first-rule".to_string(), "second-rule".to_string()])
    );
}
