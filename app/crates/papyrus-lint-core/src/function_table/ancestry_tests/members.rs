use super::super::test_support::write_script;
use super::super::*;
use papyrus_parser::ast::TypeName;
use std::collections::HashSet;

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
