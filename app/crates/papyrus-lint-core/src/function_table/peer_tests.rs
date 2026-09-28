use std::fs;
use std::sync::Arc;

use super::super::FunctionTable;
use super::{
    deepest_namespace_dir, enter_peer_scope, index_package, namespace_root, parent_eq_paths,
    push_index_leaves, resolve_peer_path, LeafHit,
};
use crate::function_table::load::ScriptOrigin;
use crate::script_locator::{
    build_script_index, conflicting_script_versions_in_index, CONFLICTING_SCRIPT_VERSIONS_RULE,
};
use papyrus_lint_globals::Game;

fn write(dir: &std::path::Path, name: &str, source: &str) -> std::path::PathBuf {
    fs::create_dir_all(dir).expect("failed to create script dir");
    let path = dir.join(name);
    fs::write(&path, source).expect("failed to write script");
    path
}

#[test]
fn same_folder_peer_resolves_only_while_that_script_is_in_scope() {
    let root = tempfile::tempdir().expect("failed to create temp dir");
    let hardcore = root.path().join("scripts/source/Base/Hardcore");
    let manager = write(
        &hardcore,
        "HC_ManagerScript.psc",
        "ScriptName HC_ManagerScript\n\nFunction Ready()\nEndFunction\n",
    );
    let drink_source =
        "ScriptName HC_DrinkWaterEffectScript\n\nHC_ManagerScript Property HC_Manager Auto\n";
    let drink = write(&hardcore, "HC_DrinkWaterEffectScript.psc", drink_source);

    let index = build_script_index(root.path(), &[]);
    assert!(
        !index.contains_key("hc_managerscript.psc"),
        "a nested peer must not become an unqualified index key"
    );
    assert!(index.contains_key("base/hardcore/hc_managerscript.psc"));
    assert!(conflicting_script_versions_in_index(
        &manager,
        &index,
        root.path(),
        false,
        Game::Skyrim
    )
    .iter()
    .all(|diagnostic| diagnostic.rule != CONFLICTING_SCRIPT_VERSIONS_RULE));

    let mut table =
        FunctionTable::new(root.path().to_path_buf()).with_script_index(Arc::new(index));
    assert!(!table.script_exists("HC_ManagerScript"));

    let _scope = enter_peer_scope(&drink, drink_source);
    assert!(table.script_exists("HC_ManagerScript"));
    assert!(table.lookup_function("HC_ManagerScript", "Ready").is_some());

    let diagnostics = papyrus_lints::lint_with_external_arguments(
        drink_source,
        &papyrus_lints::Config::default(),
        &mut table,
    );
    assert!(diagnostics
        .iter()
        .all(|diagnostic| diagnostic.rule != "unresolved-script"));
}

#[test]
fn same_folder_peer_resolves_without_a_script_index() {
    let root = tempfile::tempdir().expect("failed to create temp dir");
    let hardcore = root.path().join("scripts/source/Base/Hardcore");
    write(
        &hardcore,
        "HC_ManagerScript.psc",
        "ScriptName HC_ManagerScript\n",
    );
    let drink = hardcore.join("HC_DrinkWaterEffectScript.psc");
    let source = "ScriptName HC_DrinkWaterEffectScript\n";
    fs::write(&drink, source).expect("failed to write referrer");

    let table = FunctionTable::new(root.path().to_path_buf());
    assert!(!table.script_exists("HC_ManagerScript"));
    let _scope = enter_peer_scope(&drink, source);
    assert!(table.script_exists("HC_ManagerScript"));
}

#[test]
fn namespaced_duplicates_stay_distinct_for_conflicts_and_global_lookup() {
    let root = tempfile::tempdir().expect("failed to create temp dir");
    let user = root.path().join("scripts/source/User");
    let other = root.path().join("scripts/source/Other");
    let user_foo = write(
        &user,
        "Foo.psc",
        "ScriptName User:Foo\n\nFunction FromUser()\nEndFunction\n",
    );
    write(
        &other,
        "Foo.psc",
        "ScriptName Other:Foo\n\nFunction FromOther()\nEndFunction\n",
    );
    let user_bar = user.join("Bar.psc");
    let bar_source = "ScriptName User:Bar\n";

    let index = build_script_index(root.path(), &[]);
    assert!(!index.contains_key("foo.psc"));
    assert!(conflicting_script_versions_in_index(
        &user_foo,
        &index,
        root.path(),
        false,
        Game::Skyrim
    )
    .is_empty());

    let mut table = FunctionTable::new(root.path().to_path_buf())
        .with_game(papyrus_lints::Game::Fallout4)
        .with_script_index(Arc::new(index));

    assert!(table.script_exists("User:Foo"));
    assert!(table.script_exists("Other:Foo"));
    assert!(!table.script_exists("Foo"));

    let _scope = enter_peer_scope(&user_bar, bar_source);
    assert!(table.script_exists("Foo"));
    assert!(table.lookup_function("Foo", "FromUser").is_some());
    assert!(table.lookup_function("Foo", "FromOther").is_none());
    assert!(table.lookup_function("Other:Foo", "FromOther").is_some());
}

#[test]
fn namespace_package_resolves_one_cousin_and_refuses_two() {
    let root = tempfile::tempdir().expect("failed to create temp dir");
    let terminals = root
        .path()
        .join("scripts/source/Base/CreationClub/Fragments/Terminals");
    let term = write(
        &terminals,
        "TERM_Example.psc",
        "ScriptName CreationClub:Fragments:Terminals:TERM_Example\n",
    );
    write(
        &root
            .path()
            .join("scripts/source/Base/CreationClub/VRWorkshops"),
        "VRWorkshopParentScript.psc",
        "ScriptName CreationClub:VRWorkshops:VRWorkshopParentScript\n\nFunction SetVRPodDestination()\nEndFunction\n",
    );

    let index = build_script_index(root.path(), &[]);
    let mut table = FunctionTable::new(root.path().to_path_buf())
        .with_game(papyrus_lints::Game::Fallout4)
        .with_script_index(Arc::new(index));
    let source = "ScriptName CreationClub:Fragments:Terminals:TERM_Example\n";
    let _scope = enter_peer_scope(&term, source);
    assert!(table.script_exists("VRWorkshopParentScript"));
    assert!(table
        .lookup_function("VRWorkshopParentScript", "SetVRPodDestination")
        .is_some());
    drop(_scope);

    write(
        &root.path().join("scripts/source/Base/CreationClub/Other"),
        "VRWorkshopParentScript.psc",
        "ScriptName CreationClub:Other:VRWorkshopParentScript\n",
    );
    let index = build_script_index(root.path(), &[]);
    let table = FunctionTable::new(root.path().to_path_buf()).with_script_index(Arc::new(index));
    let _scope = enter_peer_scope(&term, source);
    assert!(
        !table.script_exists("VRWorkshopParentScript"),
        "two cousins under the namespace must not pick an arbitrary file"
    );
}

#[test]
fn known_scripts_mode_does_not_resolve_an_unlisted_peer() {
    let root = tempfile::tempdir().expect("failed to create temp dir");
    let hardcore = root.path().join("scripts/source/Base/Hardcore");
    write(
        &hardcore,
        "HC_ManagerScript.psc",
        "ScriptName HC_ManagerScript\n",
    );
    let source = "ScriptName HC_DrinkWaterEffectScript\n";
    let drink = write(&hardcore, "HC_DrinkWaterEffectScript.psc", source);

    let table = FunctionTable::new(root.path().to_path_buf())
        .with_known_scripts(std::slice::from_ref(&drink));
    let _scope = enter_peer_scope(&drink, source);
    assert!(table.script_exists("HC_DrinkWaterEffectScript"));
    assert!(!table.script_exists("HC_ManagerScript"));
}

#[test]
fn nested_peer_scopes_restore_the_previous_directory() {
    let root = tempfile::tempdir().expect("failed to create temp dir");
    let outer_dir = root.path().join("Outer");
    let inner_dir = root.path().join("Inner");
    let outer_peer = write(&outer_dir, "Peer.psc", "ScriptName Peer\n");
    let inner_peer = write(&inner_dir, "Peer.psc", "ScriptName Peer\n");
    let mut leaves = std::collections::HashMap::new();
    leaves.insert(
        "peer".to_owned(),
        vec![
            LeafHit {
                path: outer_peer.clone(),
                origin: ScriptOrigin::Project,
            },
            LeafHit {
                path: inner_peer.clone(),
                origin: ScriptOrigin::Lookup,
            },
        ],
    );

    assert!(resolve_peer_path("peer", Some(&leaves), false).is_none());
    let outer = enter_peer_scope(&outer_dir.join("Referrer.psc"), "ScriptName Referrer\n");
    assert_eq!(
        resolve_peer_path("peer.psc", Some(&leaves), false)
            .expect("outer peer should resolve")
            .path,
        outer_peer
    );
    {
        let _inner = enter_peer_scope(&inner_dir.join("Referrer.psc"), "ScriptName Referrer\n");
        let hit =
            resolve_peer_path("peer", Some(&leaves), false).expect("inner peer should resolve");
        assert_eq!(hit.path, inner_peer);
        assert!(matches!(hit.origin, ScriptOrigin::Lookup));
    }
    assert_eq!(
        resolve_peer_path("peer", Some(&leaves), false)
            .expect("outer scope should be restored")
            .path,
        outer_peer
    );
    drop(outer);
    assert!(resolve_peer_path("peer", Some(&leaves), false).is_none());
}

#[test]
fn peer_lookup_rejects_empty_and_qualified_names() {
    let root = tempfile::tempdir().expect("failed to create temp dir");
    let referrer = root.path().join("Referrer.psc");
    let _scope = enter_peer_scope(&referrer, "ScriptName Referrer\n");

    for name in ["", ".psc", "User:Peer", "User/Peer", "User\\Peer"] {
        assert!(
            resolve_peer_path(name, None, true).is_none(),
            "{name:?} must not be treated as an unqualified peer"
        );
    }
}

#[test]
fn push_index_leaves_deduplicates_paths_and_preserves_the_first_origin() {
    let path = std::path::PathBuf::from("Scripts/Source/User/Foo.psc");
    let mut index = crate::script_locator::ScriptIndex::new();
    index.insert("user/foo.psc".to_owned(), vec![path.clone()]);
    index.insert("alias/foo.psc".to_owned(), vec![path.clone()]);
    let mut leaves = std::collections::HashMap::new();

    push_index_leaves(&index, ScriptOrigin::Project, &mut leaves);
    push_index_leaves(&index, ScriptOrigin::Lookup, &mut leaves);

    let hits = leaves.get("foo").expect("foo leaf should be indexed");
    assert_eq!(hits.len(), 1);
    assert_eq!(hits[0].path, path);
    assert!(matches!(hits[0].origin, ScriptOrigin::Project));
}

#[test]
fn namespace_helpers_handle_comments_case_and_the_deepest_match() {
    assert_eq!(
        namespace_root("\n ; header\nSCRIPTNAME CreationClub:Fragments:Example extends Quest\n"),
        "creationclub"
    );
    assert_eq!(namespace_root("ScriptName Unqualified\n"), "");
    assert_eq!(namespace_root("; comment only\n"), "");
    assert_eq!(namespace_root("Function BeforeDeclaration()\n"), "");

    let path = std::path::Path::new("/CreationClub/Other/creationclub/Fragments");
    assert_eq!(
        deepest_namespace_dir(path, "CREATIONCLUB"),
        Some(std::path::PathBuf::from("/CreationClub/Other/creationclub"))
    );
    assert!(deepest_namespace_dir(path, "").is_none());
    assert!(parent_eq_paths(
        std::path::Path::new("Scripts/Source/User"),
        std::path::Path::new("scripts/source/user")
    ));
    assert!(!parent_eq_paths(
        std::path::Path::new("Scripts/Source/User"),
        std::path::Path::new("Scripts/User")
    ));
}

#[test]
fn package_index_accepts_psc_case_insensitively_and_marks_duplicates_ambiguous() {
    let root = tempfile::tempdir().expect("failed to create temp dir");
    let package = root.path().join("Package");
    let unique = write(&package.join("One"), "Unique.PSC", "ScriptName Unique\n");
    write(&package.join("One"), "Shared.psc", "ScriptName SharedOne\n");
    write(&package.join("Two"), "SHARED.PSC", "ScriptName SharedTwo\n");
    write(&package, "NotPapyrus.txt", "ignored\n");

    let leaves = index_package(&package);
    assert_eq!(leaves.get("unique"), Some(&Some(unique)));
    assert_eq!(leaves.get("shared"), Some(&None));
    assert!(!leaves.contains_key("notpapyrus"));
}
