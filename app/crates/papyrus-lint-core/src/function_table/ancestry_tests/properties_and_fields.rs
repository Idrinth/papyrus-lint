use super::super::super::test_support::write_script;
use super::super::*;

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
