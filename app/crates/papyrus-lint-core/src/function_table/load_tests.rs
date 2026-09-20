use super::super::test_support::write_script;
use super::*;
use std::collections::HashMap;
use std::fs;
use std::sync::Arc;

#[test]
fn new_with_additional_roots_resolves_a_script_outside_the_conventional_dirs() {
    let root = tempfile::tempdir().expect("failed to create temp dir");
    let shared = tempfile::tempdir().expect("failed to create temp dir");
    fs::write(
        shared.path().join("Shared.psc"),
        "ScriptName Shared\n\nInt Function DoThing()\nEndFunction\n",
    )
    .expect("failed to write shared script");

    let mut table = FunctionTable::new_with_additional_roots(
        root.path().to_path_buf(),
        vec![shared.path().to_string_lossy().into_owned()],
    );

    let signature = table
        .lookup_function("Shared", "DoThing")
        .expect("function should be found via the additional root");
    assert_eq!(signature.name, "DoThing");
}

#[test]
fn with_lookup_roots_resolves_a_script_as_a_fallback() {
    let root = tempfile::tempdir().expect("failed to create temp dir");
    let vanilla = tempfile::tempdir().expect("failed to create temp dir");
    fs::write(
        vanilla.path().join("Actor.psc"),
        "ScriptName Actor\n\nFunction DamageActorValue(String av, Float value)\nEndFunction\n",
    )
    .expect("failed to write vanilla script");

    let mut table = FunctionTable::new(root.path().to_path_buf())
        .with_lookup_roots(vec![vanilla.path().to_string_lossy().into_owned()]);

    let signature = table
        .lookup_function("Actor", "DamageActorValue")
        .expect("function should be found via the lookup root");
    assert_eq!(signature.name, "DamageActorValue");
    assert!(table.script_exists("Actor"));
}

#[test]
fn bundled_vanilla_scripts_resolve_without_a_file_on_disk() {
    let root = tempfile::tempdir().expect("failed to create temp dir");
    let mut table = FunctionTable::new(root.path().to_path_buf());

    assert!(table.script_exists("Actor"));
    assert!(table.script_exists("objectreference"));
    assert!(table.script_exists("Form"));
    assert!(!table.script_exists("DefinitelyMissingScript"));

    let signature = table
        .lookup_function("Actor", "GetActorValue")
        .expect("bundled Actor.psc should expose GetActorValue");
    assert_eq!(signature.name, "GetActorValue");
}

#[test]
fn lookup_roots_do_not_override_a_project_script_of_the_same_name() {
    let root = tempfile::tempdir().expect("failed to create temp dir");
    let source = root.path().join("scripts/source");
    fs::create_dir_all(&source).expect("failed to create source dir");
    fs::write(
        source.join("Actor.psc"),
        "ScriptName Actor\n\nFunction FromProject()\nEndFunction\n",
    )
    .expect("failed to write project script");
    let vanilla = tempfile::tempdir().expect("failed to create temp dir");
    fs::write(
        vanilla.path().join("Actor.psc"),
        "ScriptName Actor\n\nFunction FromVanilla()\nEndFunction\n",
    )
    .expect("failed to write vanilla script");

    let mut table = FunctionTable::new(root.path().to_path_buf())
        .with_lookup_roots(vec![vanilla.path().to_string_lossy().into_owned()]);

    assert!(table.lookup_function("Actor", "FromProject").is_some());
    assert!(table.lookup_function("Actor", "FromVanilla").is_none());
}

#[test]
fn with_known_scripts_still_falls_back_to_lookup_roots() {
    let root = tempfile::tempdir().expect("failed to create temp dir");
    let listed_dir = tempfile::tempdir().expect("failed to create temp dir");
    let listed_path = listed_dir.path().join("Listed.psc");
    fs::write(&listed_path, "ScriptName Listed\n").expect("failed to write listed script");
    let vanilla = tempfile::tempdir().expect("failed to create temp dir");
    fs::write(
        vanilla.path().join("Actor.psc"),
        "ScriptName Actor\n\nFunction DamageActorValue(String av, Float value)\nEndFunction\n",
    )
    .expect("failed to write vanilla script");

    let mut table = FunctionTable::new(root.path().to_path_buf())
        .with_lookup_roots(vec![vanilla.path().to_string_lossy().into_owned()])
        .with_known_scripts(&[listed_path]);

    assert!(table.script_exists("Listed"));
    assert!(table.script_exists("Actor"));
    assert!(!table.script_exists("Unlisted"));
    assert!(table.lookup_function("Actor", "DamageActorValue").is_some());
}

#[test]
fn with_script_index_resolves_without_searching_the_configured_roots() {
    let root = tempfile::tempdir().expect("failed to create temp dir");
    let indexed_dir = tempfile::tempdir().expect("failed to create temp dir");
    let indexed_path = indexed_dir.path().join("Shared.psc");
    fs::write(
        &indexed_path,
        "ScriptName Shared\n\nInt Function DoThing()\nEndFunction\n",
    )
    .expect("failed to write indexed script");
    let index = Arc::new(HashMap::from([(
        "shared.psc".to_string(),
        vec![indexed_path],
    )]));

    let mut table = FunctionTable::new(root.path().join("nonexistent")).with_script_index(index);

    assert!(table.script_exists("SHARED"));
    assert!(table.lookup_function("Shared", "DoThing").is_some());
    assert!(!table.script_exists("Missing"));
}

#[test]
fn with_known_scripts_resolves_a_script_named_explicitly_without_a_directory_scan() {
    let root = tempfile::tempdir().expect("failed to create temp dir");
    let mod_a = tempfile::tempdir().expect("failed to create temp dir");
    let mod_b = tempfile::tempdir().expect("failed to create temp dir");
    let base_path = mod_a.path().join("Base.psc");
    let child_path = mod_b.path().join("Child.psc");
    fs::write(
        &base_path,
        "ScriptName Base\n\nInt Function DoThing()\nEndFunction\n",
    )
    .expect("failed to write base script");
    fs::write(&child_path, "ScriptName Child Extends Base\n")
        .expect("failed to write child script");

    let mut table =
        FunctionTable::new(root.path().to_path_buf()).with_known_scripts(&[base_path, child_path]);

    let signature = table
        .lookup_function("Child", "DoThing")
        .expect("function inherited via a known script should be found");
    assert_eq!(signature.name, "DoThing");
}

#[test]
fn with_known_scripts_does_not_expose_other_files_in_the_same_directory() {
    let root = tempfile::tempdir().expect("failed to create temp dir");
    let shared_dir = tempfile::tempdir().expect("failed to create temp dir");
    let listed_path = shared_dir.path().join("Listed.psc");
    fs::write(&listed_path, "ScriptName Listed\n").expect("failed to write listed script");
    fs::write(
        shared_dir.path().join("Unlisted.psc"),
        "ScriptName Unlisted\n",
    )
    .expect("failed to write unlisted sibling script");

    let table = FunctionTable::new(root.path().to_path_buf()).with_known_scripts(&[listed_path]);

    assert!(table.script_exists("Listed"));
    assert!(!table.script_exists("Unlisted"));
}

#[test]
fn with_known_scripts_lets_the_first_listed_path_win_for_a_duplicate_stem() {
    let root = tempfile::tempdir().expect("failed to create temp dir");
    let first_dir = tempfile::tempdir().expect("failed to create temp dir");
    let second_dir = tempfile::tempdir().expect("failed to create temp dir");
    let first = first_dir.path().join("Example.psc");
    let second = second_dir.path().join("Example.psc");
    fs::write(
        &first,
        "ScriptName Example\n\nFunction FromFirst()\nEndFunction\n",
    )
    .expect("failed to write first script");
    fs::write(
        &second,
        "ScriptName Example\n\nFunction FromSecond()\nEndFunction\n",
    )
    .expect("failed to write second script");

    let mut table =
        FunctionTable::new(root.path().to_path_buf()).with_known_scripts(&[first.clone(), second]);

    assert!(table.lookup_function("Example", "FromFirst").is_some());
    assert!(table.lookup_function("Example", "FromSecond").is_none());
}

#[test]
fn known_scripts_take_precedence_over_a_same_named_script_under_the_conventional_directories() {
    let root = tempfile::tempdir().expect("failed to create temp dir");
    write_script(
        root.path(),
        "Foo",
        "ScriptName Foo\n\nFunction FromConventionalDir()\nEndFunction\n",
    );
    let shared = tempfile::tempdir().expect("failed to create temp dir");
    let known_path = shared.path().join("Foo.psc");
    fs::write(
        &known_path,
        "ScriptName Foo\n\nFunction FromKnownScript()\nEndFunction\n",
    )
    .expect("failed to write known script");

    let mut table = FunctionTable::new(root.path().to_path_buf()).with_known_scripts(&[known_path]);

    assert!(table.lookup_function("Foo", "FromKnownScript").is_some());
    assert!(table
        .lookup_function("Foo", "FromConventionalDir")
        .is_none());
}

#[test]
fn with_known_scripts_does_not_resolve_an_unlisted_script_under_the_conventional_directory() {
    // Regression test: known-scripts mode must not fall back to
    // `find_psc_file` at all, not even for the project's own
    // conventional `scripts/source` directory — otherwise a listed
    // script and an unlisted sibling sitting in that same conventional
    // directory would let the unlisted one resolve anyway, defeating
    // the whole point of known-scripts mode.
    let root = tempfile::tempdir().expect("failed to create temp dir");
    write_script(root.path(), "Listed", "ScriptName Listed\n");
    write_script(root.path(), "Unlisted", "ScriptName Unlisted\n");
    let listed_path = root.path().join("scripts/source/Listed.psc");

    let table = FunctionTable::new(root.path().to_path_buf()).with_known_scripts(&[listed_path]);

    assert!(table.script_exists("Listed"));
    assert!(!table.script_exists("Unlisted"));
}

#[test]
fn returns_none_when_script_file_cannot_be_found() {
    let root = tempfile::tempdir().expect("failed to create temp dir");

    let mut table = FunctionTable::new(root.path().to_path_buf());

    assert!(table.lookup_function("Missing", "Anything").is_none());
}

#[test]
fn caches_parsed_scripts_across_lookups() {
    let root = tempfile::tempdir().expect("failed to create temp dir");
    write_script(
        root.path(),
        "Foo",
        "ScriptName Foo\n\nFunction Bar()\nEndFunction\n",
    );

    let mut table = FunctionTable::new(root.path().to_path_buf());
    assert!(table.lookup_function("Foo", "Bar").is_some());
    assert!(table.lookup_function("Foo", "Bar").is_some());
}

#[test]
fn caches_scripts_under_a_single_lowercase_key() {
    let root = tempfile::tempdir().expect("failed to create temp dir");
    write_script(
        root.path(),
        "Actor",
        "ScriptName Actor\n\nFunction DoThing()\nEndFunction\n",
    );
    write_script(root.path(), "Child", "ScriptName Child Extends Actor\n");

    let mut table = FunctionTable::new(root.path().to_path_buf());
    assert!(table.lookup_function("Child", "DoThing").is_some());
    assert!(!table.has_property("CHILD", "NoSuchProperty"));
    assert!(table.lookup_function("ACTOR", "DoThing").is_some());
    assert!(table.is_subtype("Child", "actor"));
    let _ = table.list_members("cHiLd");

    let actor_keys: Vec<_> = table
        .scripts
        .keys()
        .filter(|name| name.eq_ignore_ascii_case("actor"))
        .cloned()
        .collect();
    assert_eq!(actor_keys, ["actor"]);
}

#[test]
fn reloads_a_script_when_its_mtime_changes() {
    let root = tempfile::tempdir().expect("failed to create temp dir");
    write_script(root.path(), "Foo", "this is not a Papyrus script\n");

    let mut table = FunctionTable::new(root.path().to_path_buf());
    assert!(table.lookup_function("Foo", "Bar").is_none());
    assert!(table.script_exists("Foo"));

    write_script(
        root.path(),
        "Foo",
        "ScriptName Foo\n\nFunction Bar()\nEndFunction\n",
    );
    // `ensure_loaded` keys the cache by mtime-seconds; force a newer stamp
    // so this rewrite is visible even on filesystems with 1s resolution.
    let path = root.path().join("scripts/source/Foo.psc");
    let later = std::time::SystemTime::now() + std::time::Duration::from_secs(2);
    fs::File::open(&path)
        .expect("failed to open rewritten script")
        .set_modified(later)
        .expect("failed to bump script mtime");

    assert!(table.lookup_function("Foo", "Bar").is_some());
}

#[test]
fn drops_a_cached_script_when_the_file_disappears() {
    let root = tempfile::tempdir().expect("failed to create temp dir");
    write_script(
        root.path(),
        "Foo",
        "ScriptName Foo\n\nFunction Bar()\nEndFunction\n",
    );

    let mut table = FunctionTable::new(root.path().to_path_buf());
    assert!(table.lookup_function("Foo", "Bar").is_some());

    fs::remove_file(root.path().join("scripts/source/Foo.psc"))
        .expect("failed to remove script file");

    assert!(table.lookup_function("Foo", "Bar").is_none());
}

#[test]
fn script_exists_true_for_a_script_found_under_the_project_root() {
    let root = tempfile::tempdir().expect("failed to create temp dir");
    write_script(root.path(), "Foo", "ScriptName Foo\n");

    let table = FunctionTable::new(root.path().to_path_buf());

    assert!(table.script_exists("Foo"));
    assert!(table.script_exists("foo"));
}

#[test]
fn script_exists_true_for_a_known_native_singleton_script() {
    let root = tempfile::tempdir().expect("failed to create temp dir");

    let table = FunctionTable::new(root.path().to_path_buf());

    assert!(table.script_exists("Game"));
    assert!(table.script_exists("utility"));
    assert!(table.script_exists("Debug"));
}

#[test]
fn script_exists_false_for_a_script_that_cannot_be_found() {
    let root = tempfile::tempdir().expect("failed to create temp dir");

    let table = FunctionTable::new(root.path().to_path_buf());

    assert!(!table.script_exists("MyMissingScript"));
}

#[test]
fn lookup_roots_write_through_the_ast_cache() {
    let root = tempfile::tempdir().expect("failed to create temp dir");
    let vanilla = tempfile::tempdir().expect("failed to create temp dir");
    let path = vanilla.path().join("Actor.psc");
    let source =
        "ScriptName Actor\n\nFunction DamageActorValue(String av, Float value)\nEndFunction\n";
    fs::write(&path, source).expect("failed to write vanilla script");

    let mut table = FunctionTable::new(root.path().to_path_buf())
        .with_lookup_roots(vec![vanilla.path().to_string_lossy().into_owned()]);

    assert!(table.lookup_function("Actor", "DamageActorValue").is_some());
    assert!(
        crate::ast_cache::get(papyrus_parser::Game::Skyrim, &path, source).is_some(),
        "a lookup-root script should be stored in ast_cache like a project source file"
    );
}

#[test]
fn lookup_roots_reuse_a_process_cache_when_mtime_is_unchanged() {
    let root = tempfile::tempdir().expect("failed to create temp dir");
    let vanilla = tempfile::tempdir().expect("failed to create temp dir");
    let path = vanilla.path().join("Actor.psc");
    fs::write(
        &path,
        "ScriptName Actor\n\nFunction FromFirst()\nEndFunction\n",
    )
    .expect("failed to write vanilla script");
    let mtime = fs::metadata(&path)
        .expect("failed to read metadata")
        .modified()
        .expect("failed to read mtime");

    let mut first = FunctionTable::new(root.path().to_path_buf())
        .with_lookup_roots(vec![vanilla.path().to_string_lossy().into_owned()]);
    assert!(first.lookup_function("Actor", "FromFirst").is_some());

    fs::write(
        &path,
        "ScriptName Actor\n\nFunction FromSecond()\nEndFunction\n",
    )
    .expect("failed to overwrite vanilla script");
    let file = fs::File::open(&path).expect("failed to reopen vanilla script");
    if file.set_modified(mtime).is_err() {
        return;
    }
    drop(file);

    let mut second = FunctionTable::new(root.path().to_path_buf())
        .with_lookup_roots(vec![vanilla.path().to_string_lossy().into_owned()]);
    assert!(
        second.lookup_function("Actor", "FromFirst").is_some(),
        "unchanged mtime should reuse the process-wide lookup-script cache"
    );
    assert!(second.lookup_function("Actor", "FromSecond").is_none());
}

#[test]
fn lookup_roots_reload_when_mtime_changes() {
    let root = tempfile::tempdir().expect("failed to create temp dir");
    let vanilla = tempfile::tempdir().expect("failed to create temp dir");
    let path = vanilla.path().join("Actor.psc");
    fs::write(
        &path,
        "ScriptName Actor\n\nFunction FromFirst()\nEndFunction\n",
    )
    .expect("failed to write vanilla script");

    let mut first = FunctionTable::new(root.path().to_path_buf())
        .with_lookup_roots(vec![vanilla.path().to_string_lossy().into_owned()]);
    assert!(first.lookup_function("Actor", "FromFirst").is_some());

    fs::write(
        &path,
        "ScriptName Actor\n\nFunction FromSecond()\nEndFunction\n",
    )
    .expect("failed to overwrite vanilla script");
    let later = std::time::SystemTime::now() + std::time::Duration::from_secs(2);
    let file = fs::File::open(&path).expect("failed to reopen vanilla script");
    let _ = file.set_modified(later);
    drop(file);

    let mut second = FunctionTable::new(root.path().to_path_buf())
        .with_lookup_roots(vec![vanilla.path().to_string_lossy().into_owned()]);
    assert!(second.lookup_function("Actor", "FromSecond").is_some());
    assert!(second.lookup_function("Actor", "FromFirst").is_none());
}

#[test]
fn cached_lookup_index_is_reused_for_the_same_directories() {
    let root = tempfile::tempdir().expect("failed to create temp dir");
    let vanilla = tempfile::tempdir().expect("failed to create temp dir");
    fs::write(vanilla.path().join("Actor.psc"), "ScriptName Actor\n")
        .expect("failed to write vanilla script");
    let lookup = vec![vanilla.path().to_string_lossy().into_owned()];

    let first = crate::script_locator::cached_lookup_index(root.path(), &lookup);
    let second = crate::script_locator::cached_lookup_index(root.path(), &lookup);

    assert!(std::sync::Arc::ptr_eq(&first, &second));
    assert!(first.contains_key("actor.psc"));
}
