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

fn check_starfield(source: &str) -> Vec<Diagnostic> {
    let ast = papyrus_parser::parse_with_mode(
        source,
        papyrus_parser::parser::GameEdition::Starfield,
    )
    .ok();
    let tokens = papyrus_parser::tokenize(source).ok();
    let config = crate::config::Config {
        game: crate::Game::Starfield,
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
fn flags_a_typed_function_with_no_return_at_all() {
    let diagnostics =
        check("ScriptName Example\n\nInt Function Test()\n    Int i = 1\nEndFunction\n");

    assert_eq!(diagnostics.len(), 1);
    assert_eq!(diagnostics[0].line, 3);
    assert!(diagnostics[0].message.contains("'Test'"));
    assert!(diagnostics[0].message.starts_with("[error]"));
    assert_eq!(diagnostics[0].rule, RULE);
}

#[test]
fn allows_a_trailing_unconditional_return() {
    let diagnostics =
        check("ScriptName Example\n\nInt Function Test()\n    Return 1\nEndFunction\n");

    assert!(diagnostics.is_empty());
}

#[test]
fn does_not_flag_a_function_with_no_declared_return_type() {
    let diagnostics = check("ScriptName Example\n\nFunction Test()\n    Int i = 1\nEndFunction\n");

    assert!(diagnostics.is_empty());
}

#[test]
fn does_not_flag_native_functions() {
    let diagnostics = check("ScriptName Example\n\nInt Function Test() Native\n");

    assert!(diagnostics.is_empty());
}

#[test]
fn flags_an_if_with_no_else() {
    let diagnostics = check(
            "ScriptName Example\n\nInt Function Test(Bool flag)\n    If flag\n        Return 1\n    EndIf\nEndFunction\n",
        );

    assert_eq!(diagnostics.len(), 1);
}

#[test]
fn flags_an_if_else_where_only_one_branch_returns() {
    let diagnostics = check(
            "ScriptName Example\n\nInt Function Test(Bool flag)\n    If flag\n        Return 1\n    Else\n        Int i = 1\n    EndIf\nEndFunction\n",
        );

    assert_eq!(diagnostics.len(), 1);
}

#[test]
fn allows_an_if_else_where_every_branch_returns() {
    let diagnostics = check(
            "ScriptName Example\n\nInt Function Test(Bool flag)\n    If flag\n        Return 1\n    Else\n        Return 2\n    EndIf\nEndFunction\n",
        );

    assert!(diagnostics.is_empty());
}

#[test]
fn allows_an_if_elseif_else_where_every_branch_returns() {
    let diagnostics = check(
            "ScriptName Example\n\nInt Function Test(Int n)\n    If n == 1\n        Return 1\n    ElseIf n == 2\n        Return 2\n    Else\n        Return 3\n    EndIf\nEndFunction\n",
        );

    assert!(diagnostics.is_empty());
}

#[test]
fn flags_an_if_elseif_else_where_one_elseif_branch_falls_through() {
    let diagnostics = check(
            "ScriptName Example\n\nInt Function Test(Int n)\n    If n == 1\n        Return 1\n    ElseIf n == 2\n        Int i = 1\n    Else\n        Return 3\n    EndIf\nEndFunction\n",
        );

    assert_eq!(diagnostics.len(), 1);
}

#[test]
fn a_return_after_the_if_still_covers_a_non_exhaustive_if() {
    let diagnostics = check(
            "ScriptName Example\n\nInt Function Test(Bool flag)\n    If flag\n        Return 1\n    EndIf\n    Return 2\nEndFunction\n",
        );

    assert!(diagnostics.is_empty());
}

#[test]
fn does_not_treat_a_while_loop_as_a_guaranteed_return() {
    let diagnostics = check(
            "ScriptName Example\n\nInt Function Test(Bool flag)\n    While flag\n        Return 1\n    EndWhile\nEndFunction\n",
        );

    assert_eq!(diagnostics.len(), 1);
}

#[test]
fn allows_a_bare_return_in_a_typed_function() {
    // A bare `Return` is still an explicit return statement on this
    // path; whether it carries a value matching the declared return
    // type is `return_types`' concern, not this lint's.
    let diagnostics = check("ScriptName Example\n\nInt Function Test()\n    Return\nEndFunction\n");

    assert!(diagnostics.is_empty());
}

#[test]
fn checks_functions_declared_in_states_too() {
    let diagnostics = check(
            "ScriptName Example\n\nState Active\n    Int Function Test()\n        Int i = 1\n    EndFunction\nEndState\n",
        );

    assert_eq!(diagnostics.len(), 1);
}

#[test]
fn does_not_crash_on_unparseable_source() {
    assert!(check("ScriptName Example\n\nInt Function Test(\nEndFunction\n").is_empty());
}

#[test]
fn nested_if_inside_while_still_requires_a_trailing_return() {
    let diagnostics = check(
            "ScriptName Example\n\nInt Function Test(Bool flag)\n    While flag\n        If flag\n            Return 1\n        Else\n            Return 2\n        EndIf\n    EndWhile\nEndFunction\n",
        );

    assert_eq!(diagnostics.len(), 1);
}

#[test]
fn lock_guard_return_covers_the_enclosing_function() {
    let diagnostics = check_starfield(
        "ScriptName Example\n\nInt Function Test()\n    LockGuard WorkGuard\n        Return 1\n    EndLockGuard\nEndFunction\n",
    );

    assert!(diagnostics.is_empty());
}

#[test]
fn lock_guard_without_a_return_does_not_cover_the_enclosing_function() {
    let diagnostics = check_starfield(
        "ScriptName Example\n\nInt Function Test()\n    LockGuard WorkGuard\n        Int value = 1\n    EndLockGuard\nEndFunction\n",
    );

    assert_eq!(diagnostics.len(), 1);
}

#[test]
fn try_lock_guard_requires_returns_from_both_paths() {
    let diagnostics = check_starfield(
        "ScriptName Example\n\nInt Function Test()\n    TryLockGuard WorkGuard\n        Return 1\n    ElseTryLockGuard\n        Int value = 2\n    EndTryLockGuard\nEndFunction\n",
    );

    assert_eq!(diagnostics.len(), 1);
}

#[test]
fn try_lock_guard_with_returns_on_both_paths_covers_the_function() {
    let diagnostics = check_starfield(
        "ScriptName Example\n\nInt Function Test()\n    TryLockGuard WorkGuard\n        Return 1\n    ElseTryLockGuard\n        Return 2\n    EndTryLockGuard\nEndFunction\n",
    );

    assert!(diagnostics.is_empty());
}
