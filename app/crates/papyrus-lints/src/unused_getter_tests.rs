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

#[test]
fn flags_discarded_qualified_and_unqualified_getters_case_insensitively() {
    let diagnostics = check(
        "Function Test()\n  GetValue()\n  object.gEtOtherValue(1, NestedCall())\nEndFunction\n",
    );

    assert_eq!(diagnostics.len(), 2);
    assert_eq!((diagnostics[0].line, diagnostics[0].column), (2, 3));
    assert_eq!((diagnostics[1].line, diagnostics[1].column), (3, 10));
}

#[test]
fn ignores_getter_results_that_are_used() {
    let diagnostics = check(
            "Function Test()\n  Int value = GetValue()\n  value = GetValue()\n  Return GetValue()\n  UseValue(GetValue())\n  GetValue().UseValue()\n  Bool equal = other == GetValue()\n  If GetValue()\n  EndIf\nEndFunction\n",
        );

    assert!(diagnostics.is_empty());
}

#[test]
fn flags_a_getter_whose_result_only_feeds_a_discarded_comparison() {
    // The comparison consumes GetDistance's return value, but the
    // comparison's own result is then discarded too, since the
    // statement is neither an assignment, a return, nor a condition.
    let diagnostics = check(
            "ScriptName ABC extends Actor\n\nActor Property B Auto\n\nFunction A()\n   GetDistance(B) > 0\nEndFunction\n",
        );

    assert_eq!(diagnostics.len(), 1);
    assert_eq!(diagnostics[0].line, 6);
    assert!(diagnostics[0].message.contains("GetDistance"));
}

#[test]
fn flags_getters_discarded_through_a_negation_or_equality_check() {
    let diagnostics = check("Function Test()\n  !GetValue()\n  other == GetValue()\nEndFunction\n");

    assert_eq!(diagnostics.len(), 2);
    assert!(diagnostics.iter().all(|d| d.message.contains("GetValue")));
}

#[test]
fn ignores_discarded_non_getter_calls_and_getter_declarations() {
    let diagnostics = check(
        "Int Function GetValue()\n  DoSomething()\n  ForgetSomething()\n  Return 1\nEndFunction\n",
    );

    assert!(diagnostics.is_empty());
}

#[test]
fn supports_multiline_calls() {
    let diagnostics = check("Function Test()\n  GetValue(\\\n    1)\nEndFunction\n");

    assert_eq!(diagnostics.len(), 1);
    assert_eq!(diagnostics[0].line, 2);
}

#[test]
fn flags_the_final_getter_in_a_getter_chain() {
    let diagnostics = check(
        "Function Test()\n  Game.GetPlayer().GetActorBase().GetAV(\"health\")\nEndFunction\n",
    );

    assert_eq!(diagnostics.len(), 1);
    assert_eq!(diagnostics[0].line, 2);
    assert!(diagnostics[0].message.contains("GetAV"));
}

#[test]
fn flags_only_the_first_getter_in_one_discarded_compound_expression() {
    let diagnostics =
        check("Function Test()\n  GetFirst() + GetSecond() * GetThird()\nEndFunction\n");

    assert_eq!(diagnostics.len(), 1);
    assert_eq!((diagnostics[0].line, diagnostics[0].column), (2, 3));
    assert!(diagnostics[0].message.contains("GetFirst"));
}

#[test]
fn operators_inside_call_arguments_do_not_hide_a_discarded_getter() {
    let diagnostics =
        check("Function Test(Int Left, Int Right)\n  GetValue((Left + Right) * 2)\nEndFunction\n");

    assert_eq!(diagnostics.len(), 1);
    assert_eq!((diagnostics[0].line, diagnostics[0].column), (2, 3));
    assert!(diagnostics[0].message.contains("GetValue"));
}

#[test]
fn ignores_getters_consumed_by_compound_assignments() {
    let diagnostics = check(
            "Function Test()\n  Value += GetValue()\n  Value -= GetValue()\n  Value *= GetValue()\n  Value /= GetValue()\n  Value %= GetValue()\nEndFunction\n",
        );

    assert!(diagnostics.is_empty());
}

#[test]
fn ignores_get_prefixed_identifiers_that_are_not_calls() {
    let diagnostics = check(
        "Function Test()\n  GetValue\n  object.GetValue\n  values[GetIndex()]\nEndFunction\n",
    );

    assert!(diagnostics.is_empty());
}
