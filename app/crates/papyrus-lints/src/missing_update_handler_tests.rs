use super::*;

fn check(source: &str) -> Vec<Diagnostic> {
    let tokens = papyrus_parser::tokenize(source).ok();
    super::check(tokens.as_deref())
}

#[test]
fn compiled_pairs_are_loaded_from_yaml() {
    assert!(!UPDATE_EVENT_PAIRS.is_empty());
    assert!(UPDATE_EVENT_PAIRS
        .iter()
        .any(|pair| pair.register == "RegisterForUpdate" && pair.event == "OnUpdate"));
}

#[test]
fn does_not_flag_register_for_update_with_a_matching_event() {
    let diagnostics = check(
            "ScriptName Example\n\nFunction Start()\n    RegisterForUpdate(7.0)\nEndFunction\n\nEvent OnUpdate()\nEndEvent\n",
        );
    assert!(diagnostics.is_empty());
}

#[test]
fn flags_register_for_update_with_no_matching_event() {
    let diagnostics =
        check("ScriptName Example\n\nFunction Start()\n    RegisterForUpdate(7.0)\nEndFunction\n");
    assert_eq!(diagnostics.len(), 1);
    assert_eq!(diagnostics[0].line, 4);
    assert!(diagnostics[0].message.starts_with("[warning]"));
    assert!(diagnostics[0].message.contains("RegisterForUpdate"));
    assert!(diagnostics[0].message.contains("OnUpdate"));
}

#[test]
fn flags_register_for_single_update_with_no_matching_event() {
    let diagnostics = check(
        "ScriptName Example\n\nFunction Start()\n    RegisterForSingleUpdate(1.0)\nEndFunction\n",
    );
    assert_eq!(diagnostics.len(), 1);
    assert!(diagnostics[0].message.contains("RegisterForSingleUpdate"));
}

#[test]
fn register_for_single_update_also_accepts_a_plain_on_update_handler() {
    let diagnostics = check(
            "ScriptName Example\n\nFunction Start()\n    RegisterForSingleUpdate(1.0)\nEndFunction\n\nEvent OnUpdate()\nEndEvent\n",
        );
    assert!(diagnostics.is_empty());
}

#[test]
fn flags_register_for_update_game_time_with_no_matching_event() {
    let diagnostics = check(
        "ScriptName Example\n\nFunction Start()\n    RegisterForUpdateGameTime(1.0)\nEndFunction\n",
    );
    assert_eq!(diagnostics.len(), 1);
    assert!(diagnostics[0].message.contains("OnUpdateGameTime"));
}

#[test]
fn does_not_flag_register_for_update_game_time_with_its_own_event() {
    let diagnostics = check(
            "ScriptName Example\n\nFunction Start()\n    RegisterForUpdateGameTime(1.0)\nEndFunction\n\nEvent OnUpdateGameTime()\nEndEvent\n",
        );
    assert!(diagnostics.is_empty());
}

#[test]
fn on_update_game_time_does_not_satisfy_a_plain_register_for_update() {
    let diagnostics = check(
            "ScriptName Example\n\nFunction Start()\n    RegisterForUpdate(1.0)\nEndFunction\n\nEvent OnUpdateGameTime()\nEndEvent\n",
        );
    assert_eq!(diagnostics.len(), 1);
    assert!(diagnostics[0].message.contains("RegisterForUpdate"));
}

#[test]
fn flags_a_call_on_an_arbitrary_receiver() {
    let diagnostics = check(
            "ScriptName Example\n\nFunction Start(ObjectReference akRef)\n    akRef.RegisterForUpdate(1.0)\nEndFunction\n",
        );
    assert_eq!(diagnostics.len(), 1);
}

#[test]
fn matches_case_insensitively() {
    let diagnostics = check(
            "ScriptName Example\n\nFunction Start()\n    registerforupdate(1.0)\nEndFunction\n\nEvent onupdate()\nEndEvent\n",
        );
    assert!(diagnostics.is_empty());
}

#[test]
fn recognizes_an_event_declared_inside_a_state() {
    let diagnostics = check(
            "ScriptName Example\n\nFunction Start()\n    RegisterForUpdate(1.0)\nEndFunction\n\nState Active\n    Event OnUpdate()\n    EndEvent\nEndState\n",
        );
    assert!(diagnostics.is_empty());
}

#[test]
fn does_not_flag_unrelated_calls() {
    let diagnostics = check(
        "ScriptName Example\n\nFunction Start()\n    Debug.MessageBox(\"hi\")\nEndFunction\n",
    );
    assert!(diagnostics.is_empty());
}

#[test]
fn does_not_crash_on_unparseable_source() {
    let diagnostics =
        check("ScriptName Example\n\nFunction Start()\n    RegisterForUpdate(\"unterminated\n");
    assert!(diagnostics.is_empty());
}

#[test]
fn ignores_identifiers_that_are_not_calls() {
    let diagnostics = check("ScriptName Example\n\nInt RegisterForUpdate = 1\n");
    assert!(diagnostics.is_empty());
}
