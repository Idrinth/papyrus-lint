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
fn compiled_rules_are_loaded_from_yaml() {
    // Not a specific count: shared/rules/data/forbidden-functions.yaml's
    // entry list is expected to grow over time, so this only pins the
    // loader's shape (non-empty, each entry has its expected fields),
    // not its current content.
    assert!(!FORBIDDEN_FUNCTIONS.is_empty());
    assert!(FORBIDDEN_FUNCTIONS
        .iter()
        .all(|r| !r.script.is_empty() && !r.function.is_empty() && !r.level.is_empty()));
    assert!(FORBIDDEN_FUNCTIONS
        .iter()
        .any(|r| r.script == "Game" && r.function == "GetPlayer" && r.level == "error"));
}

#[test]
fn flags_qualified_call() {
    let diagnostics =
        check("ScriptName Example\n\nFunction DoThing()\n    Game.GetPlayer()\nEndFunction\n");
    assert_eq!(diagnostics.len(), 1);
    assert_eq!(diagnostics[0].line, 4);
    assert!(diagnostics[0]
        .message
        .starts_with("[error] Game.GetPlayer:"));
}

#[test]
fn flags_code_before_an_inline_comment_but_ignores_calls_in_comment_text() {
    let diagnostics = check(
            "ScriptName Example\n\nFunction DoThing()\n    Actor a = Game.GetPlayer(); artificially slow Game.GetPlayer()\n    ; Game.GetPlayer() is forbidden\nEndFunction\n",
        );

    assert_eq!(diagnostics.len(), 1);
    assert_eq!(diagnostics[0].line, 4);
    assert_eq!(diagnostics[0].column, 20);
    assert!(diagnostics[0].message.contains("Game.GetPlayer"));
}

#[test]
fn flags_unqualified_call() {
    let diagnostics = check(
            "ScriptName Example extends ObjectReference\n\nFunction DoThing()\n    GetLinkedRef()\nEndFunction\n",
        );
    assert_eq!(diagnostics.len(), 1);
    assert!(diagnostics[0].message.contains("GetLinkedRef"));
}

#[test]
fn flags_call_on_arbitrary_receiver() {
    let diagnostics = check(
            "ScriptName Example\n\nFunction DoThing(ObjectReference akRef)\n    akRef.RegisterForUpdate(1.0)\nEndFunction\n",
        );
    assert_eq!(diagnostics.len(), 1);
    assert!(diagnostics[0].message.contains("RegisterForUpdate"));
}

#[test]
fn does_not_flag_unrelated_calls() {
    let diagnostics = check(
            "ScriptName Example\n\nFunction DoThing()\n    Debug.MessageBox(\"hi\")\n    self.DoOtherThing()\nEndFunction\n\nFunction DoOtherThing()\nEndFunction\n",
        );
    assert!(diagnostics.is_empty());
}

#[test]
fn ignores_identifiers_that_are_not_calls() {
    let diagnostics = check("ScriptName Example\n\nInt GetPlayer = 1\n");
    assert!(diagnostics.is_empty());
}

#[test]
fn does_not_crash_on_unparseable_source() {
    let diagnostics =
        check("ScriptName Example\n\nFunction DoThing()\n    Game.GetPlayer(\"unterminated\n");
    assert!(diagnostics.is_empty());
}

#[test]
fn flags_global_singleton_call_qualified_by_its_own_name() {
    let diagnostics =
        check("ScriptName Example\n\nFunction DoThing()\n    Utility.Wait(1.0)\nEndFunction\n");
    assert_eq!(diagnostics.len(), 1);
    assert!(diagnostics[0].message.contains("Utility.Wait"));
}

#[test]
fn does_not_flag_same_named_function_on_a_different_script() {
    let diagnostics = check(
            "ScriptName Example\n\nFunction DoThing(MyScript akOther)\n    akOther.Wait(1.0)\nEndFunction\n",
        );
    assert!(diagnostics.is_empty());
}

#[test]
fn does_not_flag_global_function_called_on_an_expression_result() {
    let diagnostics = check(
        "ScriptName Example\n\nFunction DoThing()\n    GetUtility().Wait(1.0)\nEndFunction\n",
    );
    assert!(diagnostics.is_empty());
}

#[test]
fn does_not_flag_unqualified_call_to_a_global_singleton_function() {
    let diagnostics =
        check("ScriptName Example\n\nFunction DoThing()\n    Wait(1.0)\nEndFunction\n");
    assert!(diagnostics.is_empty());
}

#[test]
fn flags_unguarded_debug_trace() {
    let diagnostics = check(
        "ScriptName Example\n\nFunction DoThing()\n    Debug.Trace(\"left in\")\nEndFunction\n",
    );
    assert_eq!(diagnostics.len(), 1);
    assert!(diagnostics[0].message.contains("Debug.Trace"));
}

#[test]
fn does_not_flag_debug_trace_behind_a_simple_debug_flag() {
    let diagnostics = check(
            "ScriptName Example\n\nEvent OnEndState()\n  If IsDebugMode\n    Debug.Trace(\"State Transition -> Exited [Reverse]\", 0)\n  EndIf\nEndEvent\n",
        );
    assert!(diagnostics.is_empty());
}

#[test]
fn does_not_flag_debug_trace_behind_a_parenthesized_debug_flag() {
    let diagnostics = check(
            "ScriptName Example\n\nFunction DoThing(Bool IsDebugMode)\n    If (IsDebugMode)\n        Debug.Trace(\"guarded\")\n    EndIf\nEndFunction\n",
        );
    assert!(diagnostics.is_empty());
}

#[test]
fn does_not_flag_debug_trace_behind_a_member_debug_flag() {
    let diagnostics = check(
            "ScriptName Example\n\nFunction DoThing()\n    If Self.IsDebugMode\n        Debug.Trace(\"guarded\")\n    EndIf\nEndFunction\n",
        );
    assert!(diagnostics.is_empty());
}

#[test]
fn does_not_flag_debug_trace_behind_an_elseif_debug_flag() {
    let diagnostics = check(
            "ScriptName Example\n\nFunction DoThing(Bool ready, Bool IsDebugMode)\n    If ready\n        Return\n    ElseIf IsDebugMode\n        Debug.Trace(\"guarded\")\n    EndIf\nEndFunction\n",
        );
    assert!(diagnostics.is_empty());
}

#[test]
fn does_not_flag_nested_debug_trace_inside_a_debug_guard() {
    let diagnostics = check(
            "ScriptName Example\n\nFunction DoThing(Bool ready, Bool IsDebugMode)\n    If IsDebugMode\n        If ready\n            Debug.Trace(\"still guarded\")\n        EndIf\n    EndIf\nEndFunction\n",
        );
    assert!(diagnostics.is_empty());
}

#[test]
fn flags_debug_trace_behind_an_unrelated_bool() {
    let diagnostics = check(
            "ScriptName Example\n\nFunction DoThing(Bool ready)\n    If ready\n        Debug.Trace(\"not a debug flag\")\n    EndIf\nEndFunction\n",
        );
    assert_eq!(diagnostics.len(), 1);
    assert!(diagnostics[0].message.contains("Debug.Trace"));
}

#[test]
fn flags_debug_trace_in_the_else_of_a_debug_flag() {
    let diagnostics = check(
            "ScriptName Example\n\nFunction DoThing(Bool IsDebugMode)\n    If IsDebugMode\n        Return\n    Else\n        Debug.Trace(\"not guarded\")\n    EndIf\nEndFunction\n",
        );
    assert_eq!(diagnostics.len(), 1);
    assert!(diagnostics[0].message.contains("Debug.Trace"));
}

#[test]
fn flags_debug_trace_behind_a_negated_debug_flag() {
    let diagnostics = check(
            "ScriptName Example\n\nFunction DoThing(Bool IsDebugMode)\n    If !IsDebugMode\n        Debug.Trace(\"inverted\")\n    EndIf\nEndFunction\n",
        );
    assert_eq!(diagnostics.len(), 1);
    assert!(diagnostics[0].message.contains("Debug.Trace"));
}

#[test]
fn flags_debug_trace_behind_a_compound_debug_condition() {
    let diagnostics = check(
            "ScriptName Example\n\nFunction DoThing(Bool IsDebugMode, Bool ready)\n    If IsDebugMode && ready\n        Debug.Trace(\"not simple\")\n    EndIf\nEndFunction\n",
        );
    assert_eq!(diagnostics.len(), 1);
    assert!(diagnostics[0].message.contains("Debug.Trace"));
}

#[test]
fn still_flags_other_forbidden_calls_behind_a_debug_flag() {
    let diagnostics = check(
            "ScriptName Example\n\nFunction DoThing(Bool IsDebugMode)\n    If IsDebugMode\n        Game.GetPlayer()\n        Utility.Wait(1.0)\n    EndIf\nEndFunction\n",
        );
    assert_eq!(diagnostics.len(), 2);
    assert!(diagnostics[0].message.contains("Game.GetPlayer"));
    assert!(diagnostics[1].message.contains("Utility.Wait"));
}

#[test]
fn does_not_flag_debug_tracestack_or_notification_behind_a_debug_flag() {
    let diagnostics = check(
            "ScriptName Example\n\nFunction DoThing(Bool IsDebugMode)\n    If IsDebugMode\n        Debug.Trace(\"log\")\n        Debug.TraceStack(\"dump\")\n        Debug.Notification(\"hi\")\n    EndIf\nEndFunction\n",
        );
    assert!(diagnostics.is_empty());
}

#[test]
fn flags_unguarded_debug_tracestack_and_notification() {
    let diagnostics = check(
            "ScriptName Example\n\nFunction DoThing()\n    Debug.TraceStack(\"dump\")\n    Debug.Notification(\"hi\")\nEndFunction\n",
        );
    assert_eq!(diagnostics.len(), 2);
    assert!(diagnostics[0].message.contains("Debug.TraceStack"));
    assert!(diagnostics[1].message.contains("Debug.Notification"));
}

#[test]
fn debug_flag_match_is_case_insensitive() {
    let diagnostics = check(
            "ScriptName Example\n\nFunction DoThing(Bool isdebugmode)\n    If isdebugmode\n        Debug.Trace(\"guarded\")\n    EndIf\nEndFunction\n",
        );
    assert!(diagnostics.is_empty());
}
