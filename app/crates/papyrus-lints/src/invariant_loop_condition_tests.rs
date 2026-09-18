use super::*;

fn check(source: &str) -> Vec<Diagnostic> {
    let ast = papyrus_parser::parse(source).ok();
    super::check(ast.as_ref())
}

#[test]
fn flags_a_loop_whose_counter_is_never_incremented() {
    let diagnostics = check(
            "ScriptName Example\n\nFunction Test()\n    Int n = 5\n    While n < 10\n        Debug.Trace(\"y\")\n    EndWhile\nEndFunction\n",
        );

    assert_eq!(diagnostics.len(), 1);
    assert_eq!(diagnostics[0].line, 5);
    assert_eq!(diagnostics[0].rule, RULE);
    assert!(diagnostics[0].message.contains("'n'"));
}

#[test]
fn does_not_flag_a_loop_that_increments_its_counter() {
    let diagnostics = check(
            "ScriptName Example\n\nFunction Test()\n    Int n = 0\n    While n < 10\n        n += 1\n    EndWhile\nEndFunction\n",
        );

    assert!(diagnostics.is_empty());
}

#[test]
fn does_not_flag_a_loop_that_plainly_reassigns_its_variable() {
    let diagnostics = check(
            "ScriptName Example\n\nFunction Test()\n    Int n = 0\n    While n < 10\n        n = n + 1\n    EndWhile\nEndFunction\n",
        );

    assert!(diagnostics.is_empty());
}

#[test]
fn does_not_flag_the_loop_priming_idiom_from_issue_378() {
    // https://github.com/Idrinth/papyrus-lint/issues/378: `a` is
    // reassigned every iteration and the condition calls `a.IsDead()`,
    // so whether this actually terminates depends on what
    // Game.GetPlayer() returns at runtime — not something this lint
    // can prove either way, so it stays quiet rather than guess.
    let diagnostics = check(
            "ScriptName Example\n\nFunction Test()\n    Actor a\n    Int c = 0\n    While a == None || a.IsDead()\n        a = Game.GetPlayer()\n        c += 1\n    EndWhile\nEndFunction\n",
        );

    assert!(diagnostics.is_empty());
}

#[test]
fn does_not_flag_a_condition_depending_on_a_property() {
    let diagnostics = check(
            "ScriptName Example\n\nBool Property Flag Auto\n\nFunction Test()\n    While Flag\n        Debug.Trace(\"waiting\")\n    EndWhile\nEndFunction\n",
        );

    assert!(diagnostics.is_empty());
}

#[test]
fn does_not_flag_a_condition_built_entirely_from_literals() {
    // Already covered by static-condition; this lint only fires when
    // the condition depends on at least one identifier.
    let diagnostics =
        check("ScriptName Example\n\nFunction Test()\n    While true\n    EndWhile\nEndFunction\n");

    assert!(diagnostics.is_empty());
}

#[test]
fn does_not_flag_a_condition_reaching_a_call() {
    let diagnostics = check(
            "ScriptName Example\n\nFunction Test()\n    While GetValue() > 0\n    EndWhile\nEndFunction\n",
        );

    assert!(diagnostics.is_empty());
}

#[test]
fn flags_a_parameter_never_reassigned_in_the_body() {
    let diagnostics = check(
            "ScriptName Example\n\nFunction Test(Int count)\n    While count > 0\n        Debug.Trace(\"spin\")\n    EndWhile\nEndFunction\n",
        );

    assert_eq!(diagnostics.len(), 1);
    assert!(diagnostics[0].message.contains("'count'"));
}

#[test]
fn does_not_flag_when_assignment_only_happens_inside_a_nested_if() {
    let diagnostics = check(
            "ScriptName Example\n\nFunction Test(Bool flag)\n    Int n = 0\n    While n < 10\n        If flag\n            n = 5\n        EndIf\n    EndWhile\nEndFunction\n",
        );

    assert!(diagnostics.is_empty());
}

#[test]
fn reports_every_identifier_when_the_condition_uses_more_than_one() {
    let diagnostics = check(
            "ScriptName Example\n\nFunction Test()\n    Int a = 0\n    Int b = 0\n    While a < 10 && b < 10\n        Debug.Trace(\"spin\")\n    EndWhile\nEndFunction\n",
        );

    assert_eq!(diagnostics.len(), 1);
    assert!(diagnostics[0].message.contains("'a'"));
    assert!(diagnostics[0].message.contains("'b'"));
    assert!(diagnostics[0].message.contains("Local variables"));
}

#[test]
fn checks_functions_declared_in_states_too() {
    let diagnostics = check(
            "ScriptName Example\n\nState Active\n    Function Test()\n        Int n = 5\n        While n < 10\n        EndWhile\n    EndFunction\nEndState\n",
        );

    assert_eq!(diagnostics.len(), 1);
}

#[test]
fn flags_a_variable_declared_inside_an_earlier_if_branch() {
    let diagnostics = check(
            "ScriptName Example\n\nFunction Test(Bool flag)\n    If flag\n        Int n = 5\n        While n < 10\n            Debug.Trace(\"y\")\n        EndWhile\n    EndIf\nEndFunction\n",
        );

    assert_eq!(diagnostics.len(), 1);
}

#[test]
fn does_not_crash_on_unparseable_source() {
    assert!(check("ScriptName Example\n\nFunction Test(\nEndFunction\n").is_empty());
}
