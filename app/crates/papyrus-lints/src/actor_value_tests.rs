use super::*;

#[test]
fn compiled_actor_values_are_loaded_from_yaml() {
    assert!(!ACTOR_VALUES.is_empty());
    assert!(ACTOR_VALUES.contains(&"Health"));
}

#[test]
fn does_not_flag_a_known_actor_value() {
    let diagnostics = check(
            "ScriptName Example\n\nFunction DoThing(Actor akActor)\n    akActor.GetActorValue(\"Health\")\nEndFunction\n",
        );
    assert!(diagnostics.is_empty());
}

#[test]
fn matches_a_known_actor_value_case_insensitively() {
    let diagnostics = check(
            "ScriptName Example\n\nFunction DoThing(Actor akActor)\n    akActor.GetActorValue(\"health\")\nEndFunction\n",
        );
    assert!(diagnostics.is_empty());
}

#[test]
fn flags_an_unrecognized_actor_value() {
    let diagnostics = check(
            "ScriptName Example\n\nFunction DoThing(Actor akActor)\n    akActor.GetActorValue(\"Helth\")\nEndFunction\n",
        );
    assert_eq!(diagnostics.len(), 1);
    assert_eq!(diagnostics[0].line, 4);
    assert!(diagnostics[0].message.starts_with("[warning]"));
    assert!(diagnostics[0].message.contains("GetActorValue"));
    assert!(diagnostics[0].message.contains("Helth"));
}

#[test]
fn flags_every_actor_value_function_in_the_family() {
    for function in ACTOR_VALUE_FUNCTIONS {
        let source = format!(
                "ScriptName Example\n\nFunction DoThing(Actor akActor)\n    akActor.{function}(\"NotARealActorValue\")\nEndFunction\n"
            );
        let diagnostics = check(&source);
        assert_eq!(diagnostics.len(), 1, "{function} was not flagged");
    }
}

#[test]
fn does_not_flag_a_variable_actor_value_argument() {
    let diagnostics = check(
            "ScriptName Example\n\nFunction DoThing(Actor akActor, String asValue)\n    akActor.GetActorValue(asValue)\nEndFunction\n",
        );
    assert!(diagnostics.is_empty());
}

#[test]
fn does_not_flag_unrelated_calls() {
    let diagnostics = check(
            "ScriptName Example\n\nFunction DoThing()\n    Debug.MessageBox(\"NotARealActorValue\")\nEndFunction\n",
        );
    assert!(diagnostics.is_empty());
}

#[test]
fn flags_an_unqualified_call() {
    let diagnostics = check(
            "ScriptName Example extends Actor\n\nFunction DoThing()\n    SetActorValue(\"NotARealActorValue\", 1.0)\nEndFunction\n",
        );
    assert_eq!(diagnostics.len(), 1);
    assert!(diagnostics[0].message.contains("SetActorValue"));
}

#[test]
fn flags_a_user_defined_actor_value_when_misspelled() {
    let diagnostics = check(
            "ScriptName Example\n\nFunction DoThing(Actor akActor)\n    akActor.SetAV(\"Varaible01\", 1.0)\nEndFunction\n",
        );
    assert_eq!(diagnostics.len(), 1);
}

#[test]
fn does_not_flag_a_user_defined_actor_value() {
    let diagnostics = check(
            "ScriptName Example\n\nFunction DoThing(Actor akActor)\n    akActor.SetAV(\"Variable01\", 1.0)\nEndFunction\n",
        );
    assert!(diagnostics.is_empty());
}

#[test]
fn does_not_crash_on_unparseable_source() {
    let diagnostics =
        check("ScriptName Example\n\nFunction DoThing()\n    GetActorValue(\"unterminated\n");
    assert!(diagnostics.is_empty());
}
