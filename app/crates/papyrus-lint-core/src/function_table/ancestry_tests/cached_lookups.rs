use super::super::super::test_support::write_script;
use super::super::*;
use std::collections::HashSet;

#[test]
fn cached_lookups_report_a_miss_before_a_script_is_loaded() {
    let root = tempfile::tempdir().expect("failed to create temp dir");
    let table = FunctionTable::new(root.path().to_path_buf());

    assert!(matches!(
        table.lookup_function_cached("Foo", "Run"),
        CacheProbe::Miss
    ));
    assert!(matches!(
        table.is_subtype_cached("Foo", "Base"),
        CacheProbe::Miss
    ));
    assert!(matches!(
        table.ancestry_fully_known_cached("Foo"),
        CacheProbe::Miss
    ));
    assert!(matches!(
        table.has_property_cached("Foo", "Value"),
        CacheProbe::Miss
    ));
    assert!(matches!(
        table.has_field_cached("Foo", "Count"),
        CacheProbe::Miss
    ));
    assert!(matches!(
        table.has_state_cached("Foo", "Active"),
        CacheProbe::Miss
    ));
    assert!(matches!(
        table.ancestor_states_cached("Foo"),
        CacheProbe::Miss
    ));
    assert!(matches!(table.list_members_cached("Foo"), CacheProbe::Miss));
    assert!(matches!(
        table.property_types_cached("Foo"),
        CacheProbe::Miss
    ));
}

#[test]
fn cached_lookups_walk_loaded_ancestry_case_insensitively() {
    let root = tempfile::tempdir().expect("failed to create temp dir");
    write_script(
        root.path(),
        "Base",
        "ScriptName Base\n\nString Property Label Auto\nInt Count = 1\n\nAuto State Idle\nEndState\n\nFunction Run()\nEndFunction\n",
    );
    write_script(
        root.path(),
        "Child",
        "ScriptName Child Extends BASE\n\nState Active\nEndState\n",
    );
    let mut table = FunctionTable::new(root.path().to_path_buf());

    // Populate every ancestry slot before exercising the read-only probes.
    assert!(table.ancestry_fully_known("Child"));

    assert!(matches!(
        table.lookup_function_cached("CHILD", "RUN"),
        CacheProbe::Hit(Some(signature)) if signature.name == "Run"
    ));
    assert!(matches!(
        table.is_subtype_cached("CHILD", "base"),
        CacheProbe::Hit(true)
    ));
    assert!(matches!(
        table.ancestry_fully_known_cached("CHILD"),
        CacheProbe::Hit(true)
    ));
    assert!(matches!(
        table.has_property_cached("CHILD", "LABEL"),
        CacheProbe::Hit(true)
    ));
    assert!(matches!(
        table.has_field_cached("CHILD", "COUNT"),
        CacheProbe::Hit(true)
    ));
    assert!(matches!(
        table.has_state_cached("CHILD", "IDLE"),
        CacheProbe::Hit(true)
    ));

    let CacheProbe::Hit(mut states) = table.ancestor_states_cached("CHILD") else {
        panic!("loaded ancestry should be a cache hit");
    };
    states.sort();
    assert_eq!(
        states,
        vec![("active".to_string(), false), ("idle".to_string(), true)]
    );

    let CacheProbe::Hit(members) = table.list_members_cached("CHILD") else {
        panic!("loaded ancestry should be a cache hit");
    };
    let names: HashSet<_> = members.iter().map(Member::name).collect();
    assert_eq!(names, HashSet::from(["Label", "Run"]));
    assert!(matches!(
        table.property_types_cached("BASE"),
        CacheProbe::Hit(types) if types == vec!["String"]
    ));
}

#[test]
fn cached_lookups_return_definitive_results_for_an_unresolvable_script() {
    let root = tempfile::tempdir().expect("failed to create temp dir");
    let mut table = FunctionTable::new(root.path().to_path_buf());

    assert!(table.list_members("DefinitelyMissing").is_empty());

    assert!(matches!(
        table.lookup_function_cached("DefinitelyMissing", "Run"),
        CacheProbe::Hit(None)
    ));
    assert!(matches!(
        table.is_subtype_cached("DefinitelyMissing", "Base"),
        CacheProbe::Hit(false)
    ));
    assert!(matches!(
        table.ancestry_fully_known_cached("DefinitelyMissing"),
        CacheProbe::Hit(false)
    ));
    assert!(matches!(
        table.has_property_cached("DefinitelyMissing", "Value"),
        CacheProbe::Hit(false)
    ));
    assert!(matches!(
        table.has_field_cached("DefinitelyMissing", "Count"),
        CacheProbe::Hit(false)
    ));
    assert!(matches!(
        table.has_state_cached("DefinitelyMissing", "Active"),
        CacheProbe::Hit(false)
    ));
    assert!(matches!(
        table.ancestor_states_cached("DefinitelyMissing"),
        CacheProbe::Hit(states) if states.is_empty()
    ));
    assert!(matches!(
        table.list_members_cached("DefinitelyMissing"),
        CacheProbe::Hit(members) if members.is_empty()
    ));
    assert!(matches!(
        table.property_types_cached("DefinitelyMissing"),
        CacheProbe::Hit(types) if types.is_empty()
    ));
}

#[test]
fn cached_lookups_stop_at_a_circular_extends_chain() {
    let root = tempfile::tempdir().expect("failed to create temp dir");
    write_script(
        root.path(),
        "A",
        "ScriptName A Extends B\n\nState FromA\nEndState\n",
    );
    write_script(
        root.path(),
        "B",
        "ScriptName B Extends A\n\nFunction FromB()\nEndFunction\n",
    );
    let mut table = FunctionTable::new(root.path().to_path_buf());

    assert!(!table.ancestry_fully_known("A"));

    assert!(matches!(
        table.lookup_function_cached("A", "Missing"),
        CacheProbe::Hit(None)
    ));
    assert!(matches!(
        table.is_subtype_cached("A", "Missing"),
        CacheProbe::Hit(false)
    ));
    assert!(matches!(
        table.ancestry_fully_known_cached("A"),
        CacheProbe::Hit(false)
    ));
    assert!(matches!(
        table.has_property_cached("A", "Missing"),
        CacheProbe::Hit(false)
    ));
    assert!(matches!(
        table.ancestor_states_cached("A"),
        CacheProbe::Hit(states) if states == vec![("froma".to_string(), false)]
    ));
    assert!(matches!(
        table.list_members_cached("A"),
        CacheProbe::Hit(members) if members.len() == 1 && members[0].name() == "FromB"
    ));
}
