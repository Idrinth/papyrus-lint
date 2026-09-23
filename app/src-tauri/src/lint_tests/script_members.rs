use super::super::*;
use tempfile::tempdir;

#[test]
fn list_script_members_reports_functions_and_properties_including_inherited_ones() {
    let dir = tempdir().unwrap();
    let source_dir = dir.path().join("scripts/source");
    std::fs::create_dir_all(&source_dir).unwrap();
    std::fs::write(
        source_dir.join("Base.psc"),
        "ScriptName Base\n\nBool Property IsAwesome Auto\n",
    )
    .unwrap();
    std::fs::write(
        source_dir.join("Child.psc"),
        "ScriptName Child Extends Base\n\nInt Function DoThing(Float a)\nEndFunction\n",
    )
    .unwrap();

    let members = list_script_members(
        dir.path().to_string_lossy().into_owned(),
        "Child".to_string(),
        Vec::new(),
        Vec::new(),
    );

    let names: std::collections::HashSet<_> =
        members.iter().map(function_table::Member::name).collect();
    assert_eq!(
        names,
        std::collections::HashSet::from(["DoThing", "IsAwesome"])
    );
}

#[test]
fn list_script_members_resolves_a_type_via_an_additional_script_root() {
    let dir = tempdir().unwrap();
    let shared = tempdir().unwrap();
    std::fs::write(
        shared.path().join("Shared.psc"),
        "ScriptName Shared\n\nInt Property MyValue Auto\n",
    )
    .unwrap();

    let members = list_script_members(
        dir.path().to_string_lossy().into_owned(),
        "Shared".to_string(),
        vec![shared.path().to_string_lossy().into_owned()],
        Vec::new(),
    );

    assert_eq!(members.len(), 1);
}

#[test]
fn list_script_members_resolves_a_type_via_a_lookup_script_root() {
    let dir = tempdir().unwrap();
    let vanilla = tempdir().unwrap();
    std::fs::write(
        vanilla.path().join("Shared.psc"),
        "ScriptName Shared\n\nInt Property MyValue Auto\n",
    )
    .unwrap();

    let members = list_script_members(
        dir.path().to_string_lossy().into_owned(),
        "Shared".to_string(),
        Vec::new(),
        vec![vanilla.path().to_string_lossy().into_owned()],
    );

    assert_eq!(members.len(), 1);
    assert_eq!(members[0].name(), "MyValue");
}

#[test]
fn list_script_members_is_empty_for_an_unresolvable_type() {
    let dir = tempdir().unwrap();

    assert!(list_script_members(
        dir.path().to_string_lossy().into_owned(),
        "Missing".to_string(),
        Vec::new(),
        Vec::new(),
    )
    .is_empty());
}

#[test]
fn list_script_members_matches_type_names_case_insensitively() {
    let dir = tempdir().unwrap();
    let source_dir = dir.path().join("scripts/source");
    std::fs::create_dir_all(&source_dir).unwrap();
    std::fs::write(
        source_dir.join("Example.psc"),
        "ScriptName Example\n\nString Property DisplayName Auto\n",
    )
    .unwrap();

    let members = list_script_members(
        dir.path().to_string_lossy().into_owned(),
        "eXaMpLe".to_string(),
        Vec::new(),
        Vec::new(),
    );

    assert_eq!(members.len(), 1);
    assert_eq!(members[0].name(), "DisplayName");
}
