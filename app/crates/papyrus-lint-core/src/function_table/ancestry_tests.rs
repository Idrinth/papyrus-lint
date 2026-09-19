use super::super::test_support::write_script;
use super::*;
use papyrus_lints::ParamInfo;
use papyrus_parser::ast::TypeName;
use std::collections::HashSet;

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

#[test]
fn is_subtype_true_for_direct_and_transitive_extends() {
    let root = tempfile::tempdir().expect("failed to create temp dir");
    write_script(root.path(), "Form", "ScriptName Form\n");
    write_script(root.path(), "Armor", "ScriptName Armor Extends Form\n");
    write_script(
        root.path(),
        "ClothingArmor",
        "ScriptName ClothingArmor Extends Armor\n",
    );

    let mut table = FunctionTable::new(root.path().to_path_buf());

    assert!(table.is_subtype("Armor", "Form"));
    assert!(table.is_subtype("ClothingArmor", "Form"));
    assert!(table.is_subtype("armor", "form"));
    assert!(table.is_subtype("Form", "Form"));
}

#[test]
fn is_subtype_false_for_unrelated_or_unresolvable_types() {
    let root = tempfile::tempdir().expect("failed to create temp dir");
    write_script(root.path(), "Form", "ScriptName Form\n");
    write_script(root.path(), "Weapon", "ScriptName Weapon Extends Form\n");

    let mut table = FunctionTable::new(root.path().to_path_buf());

    assert!(!table.is_subtype("Form", "Weapon"));
    assert!(!table.is_subtype("Weapon", "Armor"));
    assert!(!table.is_subtype("Missing", "Form"));
}

#[test]
fn is_subtype_resolves_bundled_vanilla_types_with_no_project_script() {
    // Regression for https://github.com/Idrinth/papyrus-lint/issues/1082:
    // none of `Actor`/`ObjectReference`/`Form`/`Spell` have a `.psc` under
    // `root` (typical for a mod project), so this can only pass via the
    // bundled vanilla AST cache's name index.
    let root = tempfile::tempdir().expect("failed to create temp dir");
    let mut table = FunctionTable::new(root.path().to_path_buf());

    assert!(table.is_subtype("Actor", "ObjectReference"));
    assert!(table.is_subtype("Actor", "Form"));
    assert!(table.is_subtype("Spell", "Form"));
    assert!(!table.is_subtype("Form", "Actor"));
    assert!(!table.is_subtype("Spell", "ObjectReference"));
}

#[test]
fn is_subtype_falls_back_to_bundled_vanilla_types_past_a_project_scripts_extends_chain() {
    let root = tempfile::tempdir().expect("failed to create temp dir");
    write_script(
        root.path(),
        "MyQuestScript",
        "ScriptName MyQuestScript Extends Quest\n",
    );

    let mut table = FunctionTable::new(root.path().to_path_buf());

    assert!(table.is_subtype("MyQuestScript", "Quest"));
    assert!(table.is_subtype("MyQuestScript", "Form"));
}

#[test]
fn is_subtype_does_not_infinite_loop_on_circular_extends() {
    let root = tempfile::tempdir().expect("failed to create temp dir");
    write_script(root.path(), "A", "ScriptName A Extends B\n");
    write_script(root.path(), "B", "ScriptName B Extends A\n");

    let mut table = FunctionTable::new(root.path().to_path_buf());

    assert!(!table.is_subtype("A", "SomethingElse"));
}

#[test]
fn ancestry_fully_known_true_for_a_script_with_no_extends() {
    let root = tempfile::tempdir().expect("failed to create temp dir");
    write_script(root.path(), "Form", "ScriptName Form\n");

    let mut table = FunctionTable::new(root.path().to_path_buf());

    assert!(table.ancestry_fully_known("Form"));
}

#[test]
fn ancestry_fully_known_true_for_a_project_chain_ending_in_a_bundled_root() {
    let root = tempfile::tempdir().expect("failed to create temp dir");
    write_script(
        root.path(),
        "MyQuestScript",
        "ScriptName MyQuestScript Extends Quest\n",
    );

    let mut table = FunctionTable::new(root.path().to_path_buf());

    assert!(table.ancestry_fully_known("MyQuestScript"));
}

#[test]
fn ancestry_fully_known_true_for_bundled_vanilla_types_with_no_project_script() {
    let root = tempfile::tempdir().expect("failed to create temp dir");
    let mut table = FunctionTable::new(root.path().to_path_buf());

    assert!(table.ancestry_fully_known("Armor"));
    assert!(table.ancestry_fully_known("Weapon"));
    assert!(table.ancestry_fully_known("Actor"));
    assert!(table.ancestry_fully_known("Form"));
}

#[test]
fn ancestry_fully_known_false_for_an_unresolvable_type() {
    let root = tempfile::tempdir().expect("failed to create temp dir");

    let mut table = FunctionTable::new(root.path().to_path_buf());

    assert!(!table.ancestry_fully_known("SomeModsQuestScript"));
}

#[test]
fn ancestry_fully_known_false_when_a_project_script_extends_an_unresolvable_type() {
    let root = tempfile::tempdir().expect("failed to create temp dir");
    write_script(
        root.path(),
        "Child",
        "ScriptName Child Extends SomeModsQuestScript\n",
    );

    let mut table = FunctionTable::new(root.path().to_path_buf());

    assert!(!table.ancestry_fully_known("Child"));
}

#[test]
fn ancestry_fully_known_does_not_infinite_loop_on_circular_extends() {
    let root = tempfile::tempdir().expect("failed to create temp dir");
    write_script(root.path(), "A", "ScriptName A Extends B\n");
    write_script(root.path(), "B", "ScriptName B Extends A\n");

    let mut table = FunctionTable::new(root.path().to_path_buf());

    assert!(!table.ancestry_fully_known("A"));
}

#[test]
fn has_property_true_for_a_property_declared_directly_on_the_type() {
    let root = tempfile::tempdir().expect("failed to create temp dir");
    write_script(
        root.path(),
        "Foo",
        "ScriptName Foo\n\nInt Property MyValue Auto\n",
    );

    let mut table = FunctionTable::new(root.path().to_path_buf());

    assert!(table.has_property("Foo", "MyValue"));
    assert!(table.has_property("foo", "myvalue"));
}

#[test]
fn has_property_true_for_a_property_inherited_through_extends_chain() {
    let root = tempfile::tempdir().expect("failed to create temp dir");
    write_script(
        root.path(),
        "Grandparent",
        "ScriptName Grandparent\n\nBool Property IsAwesome Auto\n",
    );
    write_script(
        root.path(),
        "Middle",
        "ScriptName Middle Extends Grandparent\n",
    );
    write_script(root.path(), "Child", "ScriptName Child Extends Middle\n");

    let mut table = FunctionTable::new(root.path().to_path_buf());

    assert!(table.has_property("Child", "IsAwesome"));
}

#[test]
fn has_property_false_for_unrelated_or_unresolvable_types() {
    let root = tempfile::tempdir().expect("failed to create temp dir");
    write_script(
        root.path(),
        "Foo",
        "ScriptName Foo\n\nInt Property MyValue Auto\n",
    );

    let mut table = FunctionTable::new(root.path().to_path_buf());

    assert!(!table.has_property("Foo", "DoesNotExist"));
    assert!(!table.has_property("Missing", "Anything"));
}

#[test]
fn property_types_lists_only_this_scripts_own_declared_property_types() {
    let root = tempfile::tempdir().expect("failed to create temp dir");
    write_script(
        root.path(),
        "Base",
        "ScriptName Base\n\nInt Property Inherited Auto\n",
    );
    write_script(
        root.path(),
        "Foo",
        "ScriptName Foo Extends Base\n\nBar Property MyBar Auto\nInt Property MyValue Auto\n",
    );

    let mut table = FunctionTable::new(root.path().to_path_buf());
    let mut types = table.property_types("Foo");
    types.sort();

    assert_eq!(types, vec!["Bar".to_string(), "Int".to_string()]);
}

#[test]
fn property_types_is_empty_for_an_unresolvable_type() {
    let root = tempfile::tempdir().expect("failed to create temp dir");

    let mut table = FunctionTable::new(root.path().to_path_buf());

    assert!(table.property_types("Missing").is_empty());
}

#[test]
fn has_property_does_not_infinite_loop_on_circular_extends() {
    let root = tempfile::tempdir().expect("failed to create temp dir");
    write_script(root.path(), "A", "ScriptName A Extends B\n");
    write_script(root.path(), "B", "ScriptName B Extends A\n");

    let mut table = FunctionTable::new(root.path().to_path_buf());

    assert!(!table.has_property("A", "Anything"));
}

#[test]
fn has_field_true_for_a_variable_declared_directly_on_the_type() {
    let root = tempfile::tempdir().expect("failed to create temp dir");
    write_script(root.path(), "Foo", "ScriptName Foo\n\nInt MyValue = 1\n");

    let mut table = FunctionTable::new(root.path().to_path_buf());

    assert!(table.has_field("Foo", "MyValue"));
    assert!(table.has_field("foo", "myvalue"));
}

#[test]
fn has_field_true_for_a_variable_inherited_through_extends_chain() {
    let root = tempfile::tempdir().expect("failed to create temp dir");
    write_script(
        root.path(),
        "Grandparent",
        "ScriptName Grandparent\n\nBool IsAwesome = false\n",
    );
    write_script(
        root.path(),
        "Middle",
        "ScriptName Middle Extends Grandparent\n",
    );
    write_script(root.path(), "Child", "ScriptName Child Extends Middle\n");

    let mut table = FunctionTable::new(root.path().to_path_buf());

    assert!(table.has_field("Child", "IsAwesome"));
}

#[test]
fn has_field_false_for_unrelated_or_unresolvable_types() {
    let root = tempfile::tempdir().expect("failed to create temp dir");
    write_script(root.path(), "Foo", "ScriptName Foo\n\nInt MyValue = 1\n");

    let mut table = FunctionTable::new(root.path().to_path_buf());

    assert!(!table.has_field("Foo", "DoesNotExist"));
    assert!(!table.has_field("Missing", "Anything"));
}

#[test]
fn has_field_does_not_match_a_same_named_property() {
    let root = tempfile::tempdir().expect("failed to create temp dir");
    write_script(
        root.path(),
        "Foo",
        "ScriptName Foo\n\nInt Property MyValue Auto\n",
    );

    let mut table = FunctionTable::new(root.path().to_path_buf());

    assert!(!table.has_field("Foo", "MyValue"));
}

#[test]
fn has_field_does_not_infinite_loop_on_circular_extends() {
    let root = tempfile::tempdir().expect("failed to create temp dir");
    write_script(root.path(), "A", "ScriptName A Extends B\n");
    write_script(root.path(), "B", "ScriptName B Extends A\n");

    let mut table = FunctionTable::new(root.path().to_path_buf());

    assert!(!table.has_field("A", "Anything"));
}

#[test]
fn has_state_true_for_a_state_declared_directly_on_the_type() {
    let root = tempfile::tempdir().expect("failed to create temp dir");
    write_script(
        root.path(),
        "Foo",
        "ScriptName Foo\n\nState Active\nEndState\n",
    );

    let mut table = FunctionTable::new(root.path().to_path_buf());

    assert!(table.has_state("Foo", "Active"));
    assert!(table.has_state("foo", "active"));
}

#[test]
fn has_state_true_for_a_state_declared_on_an_ancestor() {
    let root = tempfile::tempdir().expect("failed to create temp dir");
    write_script(
        root.path(),
        "Grandparent",
        "ScriptName Grandparent\n\nState Active\nEndState\n",
    );
    write_script(
        root.path(),
        "Middle",
        "ScriptName Middle Extends Grandparent\n",
    );
    write_script(root.path(), "Child", "ScriptName Child Extends Middle\n");

    let mut table = FunctionTable::new(root.path().to_path_buf());

    assert!(table.has_state("Child", "Active"));
}

#[test]
fn has_state_false_for_unrelated_or_unresolvable_types() {
    let root = tempfile::tempdir().expect("failed to create temp dir");
    write_script(
        root.path(),
        "Foo",
        "ScriptName Foo\n\nState Active\nEndState\n",
    );

    let mut table = FunctionTable::new(root.path().to_path_buf());

    assert!(!table.has_state("Foo", "DoesNotExist"));
    assert!(!table.has_state("Missing", "Anything"));
}

#[test]
fn has_state_does_not_infinite_loop_on_circular_extends() {
    let root = tempfile::tempdir().expect("failed to create temp dir");
    write_script(root.path(), "A", "ScriptName A Extends B\n");
    write_script(root.path(), "B", "ScriptName B Extends A\n");

    let mut table = FunctionTable::new(root.path().to_path_buf());

    assert!(!table.has_state("A", "Anything"));
}

#[test]
fn ancestor_states_includes_the_types_own_and_inherited_states() {
    let root = tempfile::tempdir().expect("failed to create temp dir");
    write_script(
        root.path(),
        "Base",
        "ScriptName Base\n\nAuto State Idle\nEndState\n",
    );
    write_script(
        root.path(),
        "Child",
        "ScriptName Child Extends Base\n\nState Active\nEndState\n",
    );

    let mut table = FunctionTable::new(root.path().to_path_buf());
    let mut states = table.ancestor_states("Child");
    states.sort();

    assert_eq!(
        states,
        vec![("active".to_string(), false), ("idle".to_string(), true)]
    );
}

#[test]
fn ancestor_states_is_empty_for_an_unresolvable_type() {
    let root = tempfile::tempdir().expect("failed to create temp dir");

    let mut table = FunctionTable::new(root.path().to_path_buf());

    assert!(table.ancestor_states("Missing").is_empty());
}

#[test]
fn ancestor_states_does_not_infinite_loop_on_circular_extends() {
    let root = tempfile::tempdir().expect("failed to create temp dir");
    write_script(
        root.path(),
        "A",
        "ScriptName A Extends B\n\nState FromA\nEndState\n",
    );
    write_script(
        root.path(),
        "B",
        "ScriptName B Extends A\n\nState FromB\nEndState\n",
    );

    let mut table = FunctionTable::new(root.path().to_path_buf());
    let mut states = table.ancestor_states("A");
    states.sort();

    assert_eq!(
        states,
        vec![("froma".to_string(), false), ("fromb".to_string(), false)]
    );
}

#[test]
fn list_members_includes_functions_and_properties_declared_directly_on_the_type() {
    let root = tempfile::tempdir().expect("failed to create temp dir");
    write_script(
        root.path(),
        "Foo",
        "ScriptName Foo\n\nInt Property MyValue Auto\n\nInt Function Bar(Float a)\nEndFunction\n",
    );

    let mut table = FunctionTable::new(root.path().to_path_buf());
    let members = table.list_members("Foo");

    assert_eq!(members.len(), 2);
    assert!(members.iter().any(|m| matches!(
        m,
        Member::Function(signature) if signature.name == "Bar"
    )));
    assert!(members.iter().any(|m| matches!(
        m,
        Member::Property(signature) if signature.name == "MyValue"
    )));
}

#[test]
fn list_members_includes_a_function_declared_only_inside_a_state() {
    let root = tempfile::tempdir().expect("failed to create temp dir");
    write_script(
        root.path(),
        "Foo",
        "ScriptName Foo\n\nState Loud\n    Function Bar()\n    EndFunction\nEndState\n",
    );

    let mut table = FunctionTable::new(root.path().to_path_buf());
    let members = table.list_members("Foo");

    assert!(members.iter().any(|m| matches!(
        m,
        Member::Function(signature) if signature.name == "Bar" && signature.state.as_deref() == Some("Loud")
    )));
}

#[test]
fn list_members_includes_members_inherited_through_extends_chain() {
    let root = tempfile::tempdir().expect("failed to create temp dir");
    write_script(
        root.path(),
        "Grandparent",
        "ScriptName Grandparent\n\nBool Property IsAwesome Auto\n\nFunction DoThing()\nEndFunction\n",
    );
    write_script(
        root.path(),
        "Middle",
        "ScriptName Middle Extends Grandparent\n\nFunction DoOtherThing()\nEndFunction\n",
    );
    write_script(root.path(), "Child", "ScriptName Child Extends Middle\n");

    let mut table = FunctionTable::new(root.path().to_path_buf());
    let members = table.list_members("Child");

    let names: HashSet<_> = members.iter().map(Member::name).collect();
    assert_eq!(
        names,
        HashSet::from(["IsAwesome", "DoThing", "DoOtherThing"])
    );
}

#[test]
fn list_members_stops_at_a_circular_extends_chain() {
    let root = tempfile::tempdir().expect("failed to create temp dir");
    write_script(
        root.path(),
        "A",
        "ScriptName A Extends B\n\nFunction FromA()\nEndFunction\n",
    );
    write_script(
        root.path(),
        "B",
        "ScriptName B Extends A\n\nFunction FromB()\nEndFunction\n",
    );

    let mut table = FunctionTable::new(root.path().to_path_buf());
    let members = table.list_members("A");

    let names: HashSet<_> = members.iter().map(Member::name).collect();
    assert_eq!(names, HashSet::from(["FromA", "FromB"]));
}

#[test]
fn list_members_lets_a_closer_declaration_shadow_an_ancestors_member_of_the_same_name() {
    let root = tempfile::tempdir().expect("failed to create temp dir");
    write_script(
        root.path(),
        "Base",
        "ScriptName Base\n\nFunction DoThing()\nEndFunction\n",
    );
    write_script(
        root.path(),
        "Child",
        "ScriptName Child Extends Base\n\nBool Function DoThing()\nEndFunction\n",
    );

    let mut table = FunctionTable::new(root.path().to_path_buf());
    let members = table.list_members("Child");

    let matches: Vec<_> = members
        .iter()
        .filter(|m| m.name().eq_ignore_ascii_case("DoThing"))
        .collect();
    assert_eq!(matches.len(), 1);
    assert!(matches!(
        matches[0],
        Member::Function(signature) if signature.return_type == Some(TypeName {
            name: "Bool".to_string(),
            is_array: false,
        })
    ));
}

#[test]
fn list_members_shadows_an_ancestor_member_even_when_the_member_kind_changes() {
    let root = tempfile::tempdir().expect("failed to create temp dir");
    write_script(
        root.path(),
        "Base",
        "ScriptName Base\n\nFunction Value()\nEndFunction\n",
    );
    write_script(
        root.path(),
        "Child",
        "ScriptName Child Extends Base\n\nInt Property Value Auto\n",
    );

    let mut table = FunctionTable::new(root.path().to_path_buf());
    let matching: Vec<_> = table
        .list_members("Child")
        .into_iter()
        .filter(|member| member.name().eq_ignore_ascii_case("Value"))
        .collect();

    assert_eq!(matching.len(), 1);
    assert!(matches!(matching[0], Member::Property(_)));
}

#[test]
fn list_members_is_empty_for_an_unresolvable_type() {
    let root = tempfile::tempdir().expect("failed to create temp dir");

    let mut table = FunctionTable::new(root.path().to_path_buf());

    assert!(table.list_members("Missing").is_empty());
}

#[test]
fn list_members_does_not_infinite_loop_on_circular_extends() {
    let root = tempfile::tempdir().expect("failed to create temp dir");
    write_script(
        root.path(),
        "A",
        "ScriptName A Extends B\n\nFunction DoA()\nEndFunction\n",
    );
    write_script(
        root.path(),
        "B",
        "ScriptName B Extends A\n\nFunction DoB()\nEndFunction\n",
    );

    let mut table = FunctionTable::new(root.path().to_path_buf());
    let members = table.list_members("A");
    let names: HashSet<_> = members.iter().map(Member::name).collect();

    assert_eq!(names, HashSet::from(["DoA", "DoB"]));
}

#[test]
fn list_members_includes_documentation_comments() {
    let root = tempfile::tempdir().expect("failed to create temp dir");
    write_script(
        root.path(),
        "Foo",
        "ScriptName Foo\n{A documented script}\n\nInt Property MyValue Auto\n{The stored value}\n\nInt Function Bar(Float a)\n{Does the thing}\n    Return 1\nEndFunction\n\nFunction Undocumented()\nEndFunction\n",
    );

    let mut table = FunctionTable::new(root.path().to_path_buf());
    let members = table.list_members("Foo");

    let bar = members.iter().find_map(|member| match member {
        Member::Function(signature) if signature.name == "Bar" => Some(signature),
        _ => None,
    });
    assert_eq!(
        bar.and_then(|signature| signature.doc.as_deref()),
        Some("Does the thing")
    );

    let undocumented = members.iter().find_map(|member| match member {
        Member::Function(signature) if signature.name == "Undocumented" => Some(signature),
        _ => None,
    });
    assert_eq!(
        undocumented.and_then(|signature| signature.doc.as_ref()),
        None
    );

    let property = members.iter().find_map(|member| match member {
        Member::Property(signature) if signature.name == "MyValue" => Some(signature),
        _ => None,
    });
    assert_eq!(
        property.and_then(|signature| signature.doc.as_deref()),
        Some("The stored value")
    );
}

#[test]
fn list_members_carries_an_inherited_members_documentation_comment() {
    let root = tempfile::tempdir().expect("failed to create temp dir");
    write_script(
        root.path(),
        "Base",
        "ScriptName Base\n\nFunction DoThing()\n{Inherited help}\nEndFunction\n",
    );
    write_script(root.path(), "Child", "ScriptName Child Extends Base\n");

    let mut table = FunctionTable::new(root.path().to_path_buf());
    let members = table.list_members("Child");
    let do_thing = members.iter().find_map(|member| match member {
        Member::Function(signature) if signature.name == "DoThing" => Some(signature),
        _ => None,
    });

    assert_eq!(
        do_thing.and_then(|signature| signature.doc.as_deref()),
        Some("Inherited help")
    );
}

#[test]
fn list_members_carries_an_inherited_nodiscard_directive() {
    let root = tempfile::tempdir().expect("failed to create temp dir");
    write_script(
        root.path(),
        "Base",
        "ScriptName Base\n\nInt Function RegisterFoo() ; @nodiscard\n    Return 1\nEndFunction\n",
    );
    write_script(root.path(), "Child", "ScriptName Child Extends Base\n");

    let mut table = FunctionTable::new(root.path().to_path_buf());
    let members = table.list_members("Child");
    let register = members.iter().find_map(|member| match member {
        Member::Function(signature) if signature.name == "RegisterFoo" => Some(signature),
        _ => None,
    });

    assert_eq!(register.map(|signature| signature.nodiscard), Some(true));
}
