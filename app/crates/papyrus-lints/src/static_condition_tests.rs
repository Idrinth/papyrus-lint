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
fn flags_literal_true_condition() {
    let diagnostics =
        check("ScriptName Example\n\nFunction Test()\n    If true\n    EndIf\nEndFunction\n");

    assert_eq!(diagnostics.len(), 1);
    assert_eq!(diagnostics[0].line, 4);
    assert_eq!(diagnostics[0].rule, RULE);
    assert!(diagnostics[0].message.contains("always true"));
}

#[test]
fn flags_literal_false_condition() {
    let diagnostics =
        check("ScriptName Example\n\nFunction Test()\n    If false\n    EndIf\nEndFunction\n");

    assert_eq!(diagnostics.len(), 1);
    assert!(diagnostics[0].message.contains("always false"));
}

#[test]
fn flags_constant_numeric_comparison() {
    let diagnostics =
        check("ScriptName Example\n\nFunction Test()\n    If 1 == 1\n    EndIf\nEndFunction\n");

    assert_eq!(diagnostics.len(), 1);
    assert!(diagnostics[0].message.contains("always true"));

    let diagnostics =
        check("ScriptName Example\n\nFunction Test()\n    If 1 == 2\n    EndIf\nEndFunction\n");

    assert_eq!(diagnostics.len(), 1);
    assert!(diagnostics[0].message.contains("always false"));
}

#[test]
fn flags_constant_logical_and_unary_expressions() {
    let diagnostics = check(
            "ScriptName Example\n\nFunction Test()\n    If true && false\n    EndIf\n    If !true\n    EndIf\n    If 1 < 2 || 3 > 4\n    EndIf\nEndFunction\n",
        );

    assert_eq!(diagnostics.len(), 3);
    assert!(diagnostics.iter().all(|d| d.message.contains("always")));
}

#[test]
fn folds_every_literal_kind_and_unary_negation() {
    let diagnostics = check(
            "ScriptName Example\n\nFunction Test()\n    If 1\n    EndIf\n    If 0.0\n    EndIf\n    If \"text\"\n    EndIf\n    If None\n    EndIf\n    If -1 < 0\n    EndIf\n    If -1.5 < 0.0\n    EndIf\nEndFunction\n",
        );

    assert_eq!(diagnostics.len(), 6);
    let outcomes: Vec<_> = diagnostics
        .iter()
        .map(|diagnostic| diagnostic.message.contains("always true"))
        .collect();
    assert_eq!(outcomes, vec![true, false, true, false, true, true]);
}

#[test]
fn folds_arithmetic_string_and_comparison_operators() {
    let diagnostics = check(
            "ScriptName Example\n\nFunction Test()\n    If 1 + 2 == 3\n    EndIf\n    If 5 - 2 == 3\n    EndIf\n    If 2 * 3 == 6\n    EndIf\n    If 8 / 2 == 4\n    EndIf\n    If 7 % 4 == 3\n    EndIf\n    If 1 + 0.5 == 1.5\n    EndIf\n    If \"foo\" + \"bar\" == \"foobar\"\n    EndIf\n    If 2 >= 2\n    EndIf\n    If 2 <= 3\n    EndIf\n    If 2 != 3\n    EndIf\nEndFunction\n",
        );

    assert_eq!(diagnostics.len(), 10);
    assert!(diagnostics
        .iter()
        .all(|diagnostic| diagnostic.message.contains("always true")));
}

#[test]
fn literal_equality_handles_bool_none_and_incompatible_values() {
    assert_eq!(
        literal_eq(&Literal::Bool(true), &Literal::Bool(false)),
        Some(false)
    );
    assert_eq!(literal_eq(&Literal::None, &Literal::None), Some(true));
    assert_eq!(literal_eq(&Literal::None, &Literal::int(1)), Some(false));
    assert_eq!(literal_eq(&Literal::int(1), &Literal::None), Some(false));
    assert_eq!(
        literal_eq(&Literal::String("1".into()), &Literal::int(1)),
        None
    );
}

#[test]
fn invalid_constant_operations_are_not_folded() {
    assert_eq!(eval_unary(UnaryOp::Neg, &Literal::Bool(true)), None);
    assert_eq!(as_number(&Literal::String("nope".into())), None);
    assert_eq!(
        eval_binary(
            &Literal::String("left".into()),
            BinaryOp::Add,
            &Literal::int(1),
        ),
        None
    );
    assert_eq!(
        eval_binary(&Literal::int(1), BinaryOp::Div, &Literal::int(0)),
        None
    );
    assert_eq!(
        eval_binary(&Literal::int(1), BinaryOp::Mod, &Literal::int(0)),
        None
    );
}

#[test]
fn flags_constant_while_condition() {
    let diagnostics =
            check("ScriptName Example\n\nFunction Test()\n    While 1 > 0\n        Return\n    EndWhile\nEndFunction\n");

    assert_eq!(diagnostics.len(), 1);
    assert_eq!(diagnostics[0].line, 4);
}

#[test]
fn does_not_flag_conditions_depending_on_a_runtime_value() {
    let diagnostics = check(
            "ScriptName Example\n\nFunction Test(Bool flag, Int a)\n    If flag\n    EndIf\n    If a > 0\n    EndIf\n    If a == 1 && flag\n    EndIf\n    If GetValue()\n    EndIf\n    If Self.SomeProperty\n    EndIf\nEndFunction\n",
        );

    assert!(diagnostics.is_empty());
}

#[test]
fn checks_each_elseif_branch_and_nested_and_state_bodies() {
    let diagnostics = check(
            "ScriptName Example\n\nState Active\n    Function Test(Bool flag)\n        If flag\n        ElseIf true\n            If false\n            EndIf\n        EndIf\n    EndFunction\nEndState\n",
        );

    assert_eq!(diagnostics.len(), 2);
    assert_eq!(diagnostics[0].line, 6);
    assert_eq!(diagnostics[1].line, 7);
}

#[test]
fn does_not_crash_on_unparseable_source() {
    assert!(check("ScriptName Example\n\nFunction Test(\nEndFunction\n").is_empty());
}
