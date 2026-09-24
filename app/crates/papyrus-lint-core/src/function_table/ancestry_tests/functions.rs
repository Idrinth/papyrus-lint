use super::super::super::test_support::write_script;
use super::super::*;
use papyrus_lints::ParamInfo;
use papyrus_parser::ast::TypeName;

#[test]
fn finds_function_declared_directly_on_the_type() {
    let root = tempfile::tempdir().expect("failed to create temp dir");
    write_script(
        root.path(),
        "Foo",
        "ScriptName Foo\n\nInt Function Bar(Float a, String b)\nEndFunction\n",
    );

    let mut table = FunctionTable::new(root.path().to_path_buf());
    let signature = table
        .lookup_function("Foo", "Bar")
        .expect("function should be found");

    assert_eq!(signature.name, "Bar");
    assert_eq!(
        signature.return_type,
        Some(TypeName {
            name: "Int".to_string(),
            is_array: false,
        })
    );
    assert_eq!(
        signature.params,
        vec![
            ParamInfo {
                name: "a".to_string(),
                type_name: TypeName {
                    name: "Float".to_string(),
                    is_array: false,
                },
            },
            ParamInfo {
                name: "b".to_string(),
                type_name: TypeName {
                    name: "String".to_string(),
                    is_array: false,
                },
            },
        ]
    );
    assert_eq!(signature.state, None);
}

#[test]
fn finds_a_function_declared_only_inside_a_state() {
    let root = tempfile::tempdir().expect("failed to create temp dir");
    write_script(
        root.path(),
        "Foo",
        "ScriptName Foo\n\nState Loud\n    Int Function Bar(Float a)\n    EndFunction\nEndState\n",
    );

    let mut table = FunctionTable::new(root.path().to_path_buf());
    let signature = table
        .lookup_function("Foo", "Bar")
        .expect("state-declared function should be found");

    assert_eq!(signature.name, "Bar");
    assert_eq!(signature.state.as_deref(), Some("Loud"));
}

#[test]
fn prefers_the_empty_state_signature_over_a_same_named_state_override() {
    let root = tempfile::tempdir().expect("failed to create temp dir");
    write_script(
        root.path(),
        "Foo",
        "ScriptName Foo\n\nInt Function Bar()\n    Return 1\nEndFunction\n\nState Loud\n    Int Function Bar()\n        Return 2\n    EndFunction\nEndState\n",
    );

    let mut table = FunctionTable::new(root.path().to_path_buf());
    let signature = table
        .lookup_function("Foo", "Bar")
        .expect("function should be found");

    assert_eq!(signature.state, None);
}

#[test]
fn finds_function_inherited_through_extends_chain() {
    let root = tempfile::tempdir().expect("failed to create temp dir");
    write_script(
        root.path(),
        "Grandparent",
        "ScriptName Grandparent\n\nBool Function IsAwesome()\nEndFunction\n",
    );
    write_script(
        root.path(),
        "Middle",
        "ScriptName Middle Extends Grandparent\n",
    );
    write_script(root.path(), "Child", "ScriptName Child Extends Middle\n");

    let mut table = FunctionTable::new(root.path().to_path_buf());
    let signature = table
        .lookup_function("Child", "IsAwesome")
        .expect("inherited function should be found");

    assert_eq!(signature.name, "IsAwesome");
    assert_eq!(
        signature.return_type,
        Some(TypeName {
            name: "Bool".to_string(),
            is_array: false,
        })
    );
}

#[test]
fn finds_inherited_function_when_extends_uses_different_casing() {
    let root = tempfile::tempdir().expect("failed to create temp dir");
    write_script(
        root.path(),
        "Actor",
        "ScriptName Actor\n\nFunction DoThing()\nEndFunction\n",
    );
    write_script(root.path(), "Child", "ScriptName Child Extends ACTOR\n");

    let mut table = FunctionTable::new(root.path().to_path_buf());
    let signature = table
        .lookup_function("child", "dothing")
        .expect("inherited function should be found despite Extends casing");
    assert_eq!(signature.name, "DoThing");
}

#[test]
fn type_and_function_names_are_case_insensitive() {
    let root = tempfile::tempdir().expect("failed to create temp dir");
    write_script(
        root.path(),
        "Foo",
        "ScriptName Foo\n\nFunction Bar()\nEndFunction\n",
    );

    let mut table = FunctionTable::new(root.path().to_path_buf());

    assert!(table.lookup_function("fOO", "bAR").is_some());
}

#[test]
fn returns_none_for_unknown_function() {
    let root = tempfile::tempdir().expect("failed to create temp dir");
    write_script(root.path(), "Foo", "ScriptName Foo\n");

    let mut table = FunctionTable::new(root.path().to_path_buf());

    assert!(table.lookup_function("Foo", "DoesNotExist").is_none());
}

#[test]
fn external_event_lookup_distinguishes_events_functions_and_unknown_ancestry() {
    let root = tempfile::tempdir().expect("failed to create temp dir");
    write_script(
        root.path(),
        "ParentScript",
        "ScriptName ParentScript\n\nFunction NotAnEvent()\nEndFunction\n\nEvent OnReady()\nEndEvent\n",
    );

    let mut table = FunctionTable::new(root.path().to_path_buf());
    assert_eq!(table.has_event("ParentScript", "onready"), Some(true));
    assert_eq!(table.has_event("ParentScript", "NotAnEvent"), Some(false));
    assert_eq!(table.has_event("ParentScript", "Missing"), Some(false));
    assert_eq!(table.has_event("MissingParent", "OnKnown"), None);
}

#[test]
fn preserves_function_modifiers_array_types_and_events_in_signatures() {
    let root = tempfile::tempdir().expect("failed to create temp dir");
    write_script(
        root.path(),
        "Foo",
        "ScriptName Foo\n\nString[] Function Build(Int[] values) Global Native\n\nEvent OnReady()\nEndEvent\n",
    );

    let mut table = FunctionTable::new(root.path().to_path_buf());
    let function = table
        .lookup_function("Foo", "Build")
        .expect("native function should be found");
    let event = table
        .lookup_function("Foo", "OnReady")
        .expect("event should be found");

    assert_eq!(
        function.return_type,
        Some(TypeName {
            name: "String".to_string(),
            is_array: true,
        })
    );
    assert_eq!(function.params[0].type_name.name, "Int");
    assert!(function.params[0].type_name.is_array);
    assert!(function.is_global);
    assert!(function.is_native);
    assert!(!function.is_event);
    assert!(event.is_event);
    assert!(!event.is_global);
    assert!(!event.is_native);
    assert_eq!(event.return_type, None);
}

#[test]
fn does_not_infinite_loop_on_circular_extends() {
    let root = tempfile::tempdir().expect("failed to create temp dir");
    write_script(root.path(), "A", "ScriptName A Extends B\n");
    write_script(root.path(), "B", "ScriptName B Extends A\n");

    let mut table = FunctionTable::new(root.path().to_path_buf());

    assert!(table.lookup_function("A", "Anything").is_none());
}
