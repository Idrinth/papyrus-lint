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
fn flags_if_and_elseif_with_the_same_body() {
    let diagnostics = check(
        "ScriptName Example\n\nFunction Test(Int a)\n    If a == 1\n        DoThing(x)\n    ElseIf a == 2\n        DoThing(x)\n    EndIf\nEndFunction\n",
    );

    assert_eq!(diagnostics.len(), 1);
    assert_eq!(diagnostics[0].rule, RULE);
    assert_eq!(diagnostics[0].line, 6);
    assert!(diagnostics[0].message.starts_with("[warning]"));
    assert!(diagnostics[0].message.contains("identical"));
}

#[test]
fn flags_consecutive_elseifs_with_the_same_body() {
    let diagnostics = check(
        "ScriptName Example\n\nFunction Test(Int a)\n    If a == 1\n        DoFirst()\n    ElseIf a == 2\n        DoThing(x)\n    ElseIf a == 3\n        DoThing(x)\n    EndIf\nEndFunction\n",
    );

    assert_eq!(diagnostics.len(), 1);
    assert_eq!(diagnostics[0].line, 8);
}

#[test]
fn flags_each_adjacent_pair_in_a_run_of_identical_bodies() {
    let diagnostics = check(
        "ScriptName Example\n\nFunction Test(Int a)\n    If a == 1\n        DoThing(x)\n    ElseIf a == 2\n        DoThing(x)\n    ElseIf a == 3\n        DoThing(x)\n    EndIf\nEndFunction\n",
    );

    assert_eq!(diagnostics.len(), 2);
    assert_eq!(diagnostics[0].line, 6);
    assert_eq!(diagnostics[1].line, 8);
}

#[test]
fn ignores_non_adjacent_matching_bodies() {
    let diagnostics = check(
        "ScriptName Example\n\nFunction Test(Int a)\n    If a == 1\n        DoThing(x)\n    ElseIf a == 2\n        DoOther(x)\n    ElseIf a == 3\n        DoThing(x)\n    EndIf\nEndFunction\n",
    );

    assert!(diagnostics.is_empty());
}

#[test]
fn flags_an_else_that_repeats_the_last_arm() {
    let diagnostics = check(
        "ScriptName Example\n\nFunction Test(Int a)\n    If a == 1\n        DoThing(x)\n    ElseIf a == 2\n        DoOther(x)\n    Else\n        DoOther(x)\n    EndIf\nEndFunction\n",
    );

    assert_eq!(diagnostics.len(), 1);
    assert_eq!(diagnostics[0].line, 8);
    assert!(diagnostics[0].message.contains("Else body"));
}

#[test]
fn flags_an_else_that_repeats_the_if_when_there_is_no_elseif() {
    let diagnostics = check(
        "ScriptName Example\n\nFunction Test(Bool flag)\n    If flag\n        DoThing(x)\n    Else\n        DoThing(x)\n    EndIf\nEndFunction\n",
    );

    assert_eq!(diagnostics.len(), 1);
    assert_eq!(diagnostics[0].line, 6);
}

#[test]
fn does_not_flag_an_else_with_a_different_body() {
    let diagnostics = check(
        "ScriptName Example\n\nFunction Test(Int a)\n    If a == 1\n        DoThing(x)\n    Else\n        DoOther(x)\n    EndIf\nEndFunction\n",
    );

    assert!(diagnostics.is_empty());
}

#[test]
fn ignores_empty_bodies() {
    let diagnostics = check(
        "ScriptName Example\n\nFunction Test(Int a)\n    If a == 1\n    ElseIf a == 2\n    EndIf\nEndFunction\n",
    );

    assert!(diagnostics.is_empty());
}

#[test]
fn ignores_different_bodies() {
    let diagnostics = check(
        "ScriptName Example\n\nFunction Test(Int a)\n    If a == 1\n        DoThing(x)\n    ElseIf a == 2\n        DoOther(x)\n    EndIf\nEndFunction\n",
    );

    assert!(diagnostics.is_empty());
}

#[test]
fn treats_identifier_case_as_the_same_body() {
    let diagnostics = check(
        "ScriptName Example\n\nFunction Test(Int a)\n    If a == 1\n        DoThing(X)\n    ElseIf a == 2\n        dothing(x)\n    EndIf\nEndFunction\n",
    );

    assert_eq!(diagnostics.len(), 1);
}

#[test]
fn flags_nested_if_chains() {
    let diagnostics = check(
        "ScriptName Example\n\nFunction Test(Int a, Int b)\n    If a == 1\n        If b == 1\n            DoThing(x)\n        ElseIf b == 2\n            DoThing(x)\n        EndIf\n    EndIf\nEndFunction\n",
    );

    assert_eq!(diagnostics.len(), 1);
    assert_eq!(diagnostics[0].line, 7);
}

#[test]
fn disable_directives_suppress_diagnostics() {
    let line_disabled = crate::lint(
        "ScriptName Example\n\nFunction Test(Int a)\n    If a == 1\n        DoThing(x)\n    ElseIf a == 2 ; @disable duplicate-conditional-body\n        DoThing(x)\n    EndIf\nEndFunction\n",
        &crate::config::Config::default(),
    );
    let file_disabled = crate::lint(
        "; @disable-file duplicate-conditional-body\nScriptName Example\n\nFunction Test(Int a)\n    If a == 1\n        DoThing(x)\n    ElseIf a == 2\n        DoThing(x)\n    EndIf\nEndFunction\n",
        &crate::config::Config::default(),
    );

    assert!(line_disabled.iter().all(|diagnostic| diagnostic.rule != RULE));
    assert!(file_disabled.iter().all(|diagnostic| diagnostic.rule != RULE));
}

#[test]
fn config_off_switch_suppresses_diagnostics() {
    let source = "ScriptName Example\n\nFunction Test(Int a)\n    If a == 1\n        DoThing(x)\n    ElseIf a == 2\n        DoThing(x)\n    EndIf\nEndFunction\n";
    let mut config = crate::config::Config::default();
    config.rules.duplicate_conditional_body = false;

    let diagnostics = crate::lint(source, &config);

    assert!(diagnostics.iter().all(|diagnostic| diagnostic.rule != RULE));
}

#[test]
fn compares_every_supported_statement_shape_structurally() {
    let bodies = [
        "        Int value = 1\n",
        "        value += 1\n",
        "        Return value\n",
        "        If ready\n            DoThing()\n        Else\n            DoOther()\n        EndIf\n",
        "        While ready\n            DoThing()\n        EndWhile\n",
    ];

    for body in bodies {
        let source = format!(
            "ScriptName Example\n\nInt Function Test(Bool ready)\n    If ready\n{body}    Else\n{body}    EndIf\nEndFunction\n"
        );
        let diagnostics = check(&source);
        assert_eq!(diagnostics.len(), 1, "body was not compared: {body:?}");
    }
}

#[test]
fn compares_every_supported_expression_shape_structurally() {
    let expressions = [
        "42",
        "1.5",
        "\"text\"",
        "true",
        "none",
        "self",
        "parent",
        "left + right",
        "!ready",
        "DoThing(value, named = 1)",
        "object.memberValue",
        "values[index]",
        "value as Float",
        "new Int[3]",
    ];

    for expression in expressions {
        let source = format!(
            "ScriptName Example Extends ParentScript\n\nFunction Test(Bool ready)\n    If ready\n        Consume({expression})\n    Else\n        Consume({expression})\n    EndIf\nEndFunction\n"
        );
        let diagnostics = check(&source);
        assert_eq!(
            diagnostics.len(),
            1,
            "expression was not compared: {expression:?}"
        );
    }
}

#[test]
fn comparison_includes_declaration_and_expression_details() {
    let differing_pairs = [
        ("Int value", "Float value"),
        ("Int value", "Int other"),
        ("Int value = 1", "Int value = 2"),
        ("Consume(1)", "Consume(2)"),
        ("Consume(named = 1)", "Consume(other = 1)"),
        ("Consume(object.First)", "Consume(object.Second)"),
        ("Consume(values[0])", "Consume(values[1])"),
        ("Consume(value as Int)", "Consume(value as Float)"),
        ("Consume(new Int[2])", "Consume(new Float[2])"),
    ];

    for (left, right) in differing_pairs {
        let source = format!(
            "ScriptName Example\n\nFunction Test(Bool ready)\n    If ready\n        {left}\n    Else\n        {right}\n    EndIf\nEndFunction\n"
        );
        assert!(
            check(&source).is_empty(),
            "different bodies were treated as equal: {left:?} and {right:?}"
        );
    }
}

#[test]
fn compares_fallout_struct_construction() {
    let source = "ScriptName Example\n\nFunction Test(Bool ready, Var value)\n    If ready\n        Consume(new Coordinates)\n        Consume(value is Float)\n    Else\n        Consume(new coordinates)\n        Consume(value is float)\n    EndIf\nEndFunction\n";
    let ast = papyrus_parser::parse_with_mode(
        source,
        papyrus_parser::parser::GameEdition::Fallout4,
    )
    .expect("Fallout 4 struct construction should parse");
    let tokens = papyrus_parser::tokenize(source).unwrap();

    let diagnostics = super::check(
        source,
        Some(&ast),
        Some(&tokens),
        &crate::config::Config::default(),
        &mut crate::external_signatures::NoExternalSignatures,
    );

    assert_eq!(diagnostics.len(), 1);
}

#[test]
fn fallout_expression_comparison_includes_type_names() {
    let source = "ScriptName Example\n\nFunction Test(Bool ready, Var value)\n    If ready\n        Consume(new FirstStruct)\n        Consume(value is Int)\n    Else\n        Consume(new SecondStruct)\n        Consume(value is Float)\n    EndIf\nEndFunction\n";
    let ast = papyrus_parser::parse_with_mode(
        source,
        papyrus_parser::parser::GameEdition::Fallout4,
    )
    .expect("Fallout 4 expressions should parse");
    let tokens = papyrus_parser::tokenize(source).unwrap();

    let diagnostics = super::check(
        source,
        Some(&ast),
        Some(&tokens),
        &crate::config::Config::default(),
        &mut crate::external_signatures::NoExternalSignatures,
    );

    assert!(diagnostics.is_empty());
}

#[test]
fn compares_starfield_lock_guards() {
    let source = "ScriptName Example\n\nFunction Test(Bool ready)\n    If ready\n        TryLockGuard MainGuard\n            DoThing()\n        ElseTryLockGuard\n            Return\n        EndTryLockGuard\n    Else\n        TryLockGuard mainguard\n            DoThing()\n        ElseTryLockGuard\n            Return\n        EndTryLockGuard\n    EndIf\nEndFunction\n";
    let ast = papyrus_parser::parse_with_mode(
        source,
        papyrus_parser::parser::GameEdition::Starfield,
    )
    .expect("Starfield lock guards should parse");
    let tokens = papyrus_parser::tokenize(source).unwrap();

    let diagnostics = super::check(
        source,
        Some(&ast),
        Some(&tokens),
        &crate::config::Config::default(),
        &mut crate::external_signatures::NoExternalSignatures,
    );

    assert_eq!(diagnostics.len(), 1);
}
