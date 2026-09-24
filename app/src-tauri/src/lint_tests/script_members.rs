use super::super::*;
use tempfile::tempdir;

#[test]
fn resolve_completion_query_finds_declared_receiver_types() {
    let source = "ScriptName Example Extends Quest\nActor[] actors\nFunction Run(ObjectReference target)\ntarget.\nactors[0].Disa\nEndFunction";
    let cursor = source.find("Disa").unwrap() + 4;

    assert_eq!(
        resolve_completion_query(source.to_string(), cursor),
        Some(CompletionQuery {
            receiver_type: "Actor".to_string(),
            prefix: "Disa".to_string(),
            prefix_start: cursor - 4,
        })
    );
    // The command receives the complete editor buffer; only the cursor limits
    // the member-access match. Keep the closing function header available so
    // its parameter declaration can still be resolved.
    let target_cursor = source.find("target.").unwrap() + "target.".len();
    assert_eq!(
        resolve_completion_query(source.to_string(), target_cursor)
            .unwrap()
            .receiver_type,
        "ObjectReference"
    );
}

#[test]
fn resolve_completion_query_handles_self_parent_and_comments() {
    let source = "ScriptName Example Extends Quest\n; Actor ignored\nself.\nparent.Get";
    let self_cursor = source.find("self.").unwrap() + 5;
    assert_eq!(
        resolve_completion_query(source.to_string(), self_cursor)
            .unwrap()
            .receiver_type,
        "Example"
    );
    assert_eq!(
        resolve_completion_query(source.to_string(), source.len())
            .unwrap()
            .receiver_type,
        "Quest"
    );
    let ignored_cursor = source.find("ignored").unwrap() + "ignored".len();
    assert!(resolve_completion_query(
        format!("{}.\n", &source[..ignored_cursor]),
        ignored_cursor + 1
    )
    .is_none());
}

#[test]
fn resolve_completion_query_rejects_unknown_and_compound_receivers() {
    assert!(resolve_completion_query("ScriptName Example\nunknown.".to_string(), 28).is_none());
    let source = "ScriptName Example\nActor target\ntarget.GetActor().GetName";
    assert!(resolve_completion_query(source.to_string(), source.len()).is_none());
}

#[test]
fn resolve_completion_query_accepts_browser_utf16_cursor_offsets() {
    let source = "ScriptName Example\n; 😀\nActor target\ntarget.Get";
    let cursor = source.encode_utf16().count();

    assert_eq!(
        resolve_completion_query(source.to_string(), cursor),
        Some(CompletionQuery {
            receiver_type: "Actor".to_string(),
            prefix: "Get".to_string(),
            prefix_start: cursor - 3,
        })
    );
}

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
