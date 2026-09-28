use std::fs;
use std::sync::Arc;

use super::super::FunctionTable;
use super::enter_peer_scope;
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
