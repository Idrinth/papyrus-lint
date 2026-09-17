use super::*;
use papyrus_lints::{ExternalSignatures, ParamInfo};
use papyrus_parser::ast::TypeName;
use std::collections::HashSet;
use std::fs;
use std::path::Path;
use std::sync::Mutex;

fn write_script(dir: &Path, name: &str, contents: &str) {
    let source_dir = dir.join("scripts/source");
    fs::create_dir_all(&source_dir).expect("failed to create source dir");
    fs::write(source_dir.join(format!("{name}.psc")), contents)
        .expect("failed to write test script file");
}

fn diagnostics_for(
    rule: &str,
    source: &str,
    table: &mut FunctionTable,
) -> Vec<papyrus_lints::Diagnostic> {
    papyrus_lints::lint_with_external_arguments(source, &papyrus_lints::Config::default(), table)
        .into_iter()
        .filter(|diagnostic| diagnostic.rule == rule)
        .collect()
}

#[test]
fn exposes_the_configured_project_and_additional_roots() {
    let root = PathBuf::from("/example/project");
    let additional_roots = vec!["shared/scripts".to_string(), "/sdk/source".to_string()];

    let table = FunctionTable::new_with_additional_roots(root.clone(), additional_roots.clone());

    assert_eq!(table.root(), root);
    assert_eq!(table.additional_roots(), additional_roots);
}

#[test]
fn shared_function_table_forwards_every_external_signature_lookup() {
    let root = tempfile::tempdir().expect("failed to create temp dir");
    write_script(
        root.path(),
        "Helpers",
        "ScriptName Helpers\n\nFunction Run() Global\nEndFunction\n",
    );
    write_script(root.path(), "Child", "ScriptName Child Extends Helpers\n");
    write_script(
        root.path(),
        "Properties",
        "ScriptName Properties\n\nString Property Name Auto\n",
    );
    write_script(
        root.path(),
        "States",
        "ScriptName States\n\nState Active\nEndState\n",
    );
    let table = Mutex::new(FunctionTable::new(root.path().to_path_buf()));
    let mut shared = SharedFunctionTable(&table);

    let params = shared
        .lookup("Helpers", "Run")
        .expect("function should resolve through the adapter");
    assert!(params.is_empty());
    assert!(shared.is_subtype("Child", "Helpers"));
    assert!(shared.has_property("Properties", "Name"));
    assert_eq!(shared.property_types("Properties"), vec!["String"]);
    assert!(shared.script_exists("Child"));
    assert!(shared.can_resolve_script("Child"));
    assert!(!shared.can_resolve_script("Missing"));
    assert!(shared.type_exists("Int"));
    assert!(shared.has_state("States", "Active"));
    assert_eq!(
        shared.ancestor_states("States"),
        vec![("active".to_string(), false)]
    );
    assert_eq!(shared.is_global_function("Helpers", "Run"), Some(true));
    assert!(shared.ancestry_fully_known("Child"));
}

#[test]
#[cfg(unix)]
fn known_scripts_ignore_paths_whose_file_stem_is_not_utf8() {
    use std::ffi::OsString;
    use std::os::unix::ffi::OsStringExt;

    let root = tempfile::tempdir().expect("failed to create temp dir");
    let invalid_path = root.path().join(OsString::from_vec(vec![
        b'E', b'x', 0xFF, b'.', b'p', b's', b'c',
    ]));
    let valid_path = root.path().join("Example.psc");
    fs::write(&valid_path, "ScriptName Example\n").expect("failed to write valid script");

    let mut table = FunctionTable::new(root.path().to_path_buf())
        .with_known_scripts(&[invalid_path, valid_path]);

    assert!(table.script_exists("Example"));
}

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

    let mut table =
        FunctionTable::new(root.path().to_path_buf()).with_known_scripts(&[listed_path]);

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

    let mut table =
        FunctionTable::new(root.path().to_path_buf()).with_known_scripts(&[listed_path]);

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

    // Remove the backing file: a cached lookup must not touch disk again.
    fs::remove_file(root.path().join("scripts/source/Foo.psc"))
        .expect("failed to remove script file");

    assert!(table.lookup_function("Foo", "Bar").is_some());
}

#[test]
fn caches_an_unparseable_script_as_unresolved() {
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

    assert!(table.lookup_function("Foo", "Bar").is_none());
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
fn is_subtype_resolves_native_engine_types_with_no_project_script() {
    let root = tempfile::tempdir().expect("failed to create temp dir");

    let mut table = FunctionTable::new(root.path().to_path_buf());

    // None of `Actor`, `ObjectReference`, `Form`, `Spell` or `MagicItem`
    // have a `.psc` anywhere under `root` (typical for a mod project,
    // which doesn't ship copies of the game's own scripts), so this can
    // only pass via the native type fallback.
    assert!(table.is_subtype("Actor", "ObjectReference"));
    assert!(table.is_subtype("Actor", "Form"));
    assert!(table.is_subtype("Spell", "Form"));
    assert!(!table.is_subtype("Form", "Actor"));
    assert!(!table.is_subtype("Spell", "ObjectReference"));
}

#[test]
fn is_subtype_falls_back_to_native_types_past_a_project_scripts_extends_chain() {
    let root = tempfile::tempdir().expect("failed to create temp dir");
    // `MyQuestScript` is a project script, but the `Quest` it extends is
    // the native engine type and has no `.psc` under `root`.
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
fn ancestry_fully_known_true_for_a_project_chain_ending_in_a_native_root() {
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
fn ancestry_fully_known_true_for_native_engine_types_with_no_project_script() {
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
fn flags_a_local_variable_shadowing_a_parent_property_through_the_shadowing_lint() {
    let root = tempfile::tempdir().expect("failed to create temp dir");
    write_script(
        root.path(),
        "BaseScript",
        "ScriptName BaseScript\n\nInt Property MyValue Auto\n",
    );

    let mut table = FunctionTable::new(root.path().to_path_buf());
    let diagnostics = diagnostics_for(
        "local-variable-shadowing",
        "ScriptName Example Extends BaseScript\n\nFunction Test()\n    Int MyValue = 1\nEndFunction\n",
        &mut table,
    );

    assert_eq!(diagnostics.len(), 1);
    assert!(diagnostics[0]
        .message
        .contains("inherited from a parent script"));
}

#[test]
fn accepts_an_actor_argument_for_an_object_reference_parameter_with_no_native_scripts_in_project() {
    // Regression test: `Actor`/`ObjectReference`/`Form`/`Spell` are
    // native engine types with no `.psc` under `root` (the project
    // ships none of the game's own scripts), so this can only pass via
    // `FunctionTable::is_subtype`'s native type fallback.
    let root = tempfile::tempdir().expect("failed to create temp dir");
    write_script(
        root.path(),
        "UpcastProbe",
        "ScriptName UpcastProbe extends Quest\n",
    );

    let mut table = FunctionTable::new(root.path().to_path_buf());
    let diagnostics = papyrus_lints::check_argument_types(
        r#"
ScriptName UpcastProbe extends Quest

ObjectReference Property AnObjRef Auto
Actor           Property AnActor  Auto
Form            Property AForm    Auto
Spell           Property ASpell   Auto

Function Takes(ObjectReference akRef)
EndFunction

Function Probe()
Takes(AnObjRef)
Takes(AnActor)
Takes(AForm)
Takes(ASpell)
EndFunction
"#,
        &mut table,
    );

    assert_eq!(diagnostics.len(), 2);
    assert!(diagnostics[0].message.contains("Takes"));
    assert!(diagnostics[0].message.contains("got Form"));
    assert!(diagnostics[1].message.contains("Takes"));
    assert!(diagnostics[1].message.contains("got Spell"));
}

#[test]
fn flags_a_cast_to_a_native_ancestor_type_through_the_useless_downcast_lint() {
    // Regression test: `Actor`/`ObjectReference` are native engine types
    // with no `.psc` under `root`, so this can only pass via
    // `FunctionTable::is_subtype`'s native type fallback.
    let root = tempfile::tempdir().expect("failed to create temp dir");

    let mut table = FunctionTable::new(root.path().to_path_buf());
    let diagnostics = diagnostics_for(
        "useless-downcast",
        "ScriptName Example\n\nFunction Test(Actor dude)\n    Foo(dude as ObjectReference)\nEndFunction\n",
        &mut table,
    );

    assert_eq!(diagnostics.len(), 1);
    assert!(diagnostics[0]
        .message
        .contains("'Actor' already extends 'ObjectReference'"));
}

#[test]
fn resolves_an_armor_argument_for_a_form_parameter_through_the_argument_type_check_lint() {
    let root = tempfile::tempdir().expect("failed to create temp dir");
    write_script(root.path(), "Form", "ScriptName Form\n");
    write_script(root.path(), "Armor", "ScriptName Armor Extends Form\n");
    write_script(
        root.path(),
        "ObjectReference",
        "ScriptName ObjectReference\n\nInt Function GetItemCount(Form akItem)\nEndFunction\n",
    );

    let mut table = FunctionTable::new(root.path().to_path_buf());
    let diagnostics = papyrus_lints::check_argument_types(
        "ScriptName Example\n\nArmor Property MyArmor Auto\n\nFunction Test(ObjectReference akRef)\n    akRef.GetItemCount(MyArmor)\nEndFunction\n",
        &mut table,
    );

    assert!(diagnostics.is_empty());
}

#[test]
fn resolves_an_armor_return_value_for_a_form_return_type_through_the_return_type_check_lint() {
    let root = tempfile::tempdir().expect("failed to create temp dir");
    write_script(root.path(), "Form", "ScriptName Form\n");
    write_script(root.path(), "Armor", "ScriptName Armor Extends Form\n");

    let mut table = FunctionTable::new(root.path().to_path_buf());
    let diagnostics = diagnostics_for(
        "return-types",
        "ScriptName Example\n\nArmor Property MyArmor Auto\n\nForm Function Test()\n    Return MyArmor\nEndFunction\n",
        &mut table,
    );

    assert!(diagnostics.is_empty());
}

#[test]
fn still_flags_an_unrelated_return_type_through_the_return_type_check_lint() {
    let root = tempfile::tempdir().expect("failed to create temp dir");
    write_script(root.path(), "Form", "ScriptName Form\n");
    write_script(root.path(), "Weapon", "ScriptName Weapon Extends Form\n");
    write_script(root.path(), "Armor", "ScriptName Armor Extends Form\n");

    let mut table = FunctionTable::new(root.path().to_path_buf());
    let diagnostics = diagnostics_for(
        "return-types",
        "ScriptName Example\n\nWeapon Property MyWeapon Auto\n\nArmor Function Test()\n    Return MyWeapon\nEndFunction\n",
        &mut table,
    );

    assert_eq!(diagnostics.len(), 1);
    assert!(diagnostics[0]
        .message
        .contains("declares return type Armor"));
    assert!(diagnostics[0].message.contains("returns Weapon"));
}

#[test]
fn drives_the_function_override_lint_across_scripts() {
    let root = tempfile::tempdir().expect("failed to create temp dir");
    write_script(
        root.path(),
        "ParentScript",
        "ScriptName ParentScript\n\nFunction DoThing()\nEndFunction\n",
    );

    let mut table = FunctionTable::new(root.path().to_path_buf());
    let diagnostics = diagnostics_for(
        "function-override",
        "ScriptName Example Extends ParentScript\n\nFunction DoThing()\nEndFunction\n",
        &mut table,
    );

    assert_eq!(diagnostics.len(), 1);
    assert!(diagnostics[0].message.contains("'DoThing'"));
    assert!(diagnostics[0].message.contains("'ParentScript'"));
}

#[test]
fn finds_an_override_inherited_transitively_through_the_function_override_lint() {
    let root = tempfile::tempdir().expect("failed to create temp dir");
    write_script(
        root.path(),
        "Grandparent",
        "ScriptName Grandparent\n\nFunction DoThing()\nEndFunction\n",
    );
    write_script(
        root.path(),
        "Middle",
        "ScriptName Middle Extends Grandparent\n",
    );

    let mut table = FunctionTable::new(root.path().to_path_buf());
    let diagnostics = diagnostics_for(
        "function-override",
        "ScriptName Example Extends Middle\n\nFunction DoThing()\nEndFunction\n",
        &mut table,
    );

    assert_eq!(diagnostics.len(), 1);
    assert!(diagnostics[0].message.contains("'DoThing'"));
}

#[test]
fn drives_the_argument_naming_lint_across_scripts() {
    let root = tempfile::tempdir().expect("failed to create temp dir");
    write_script(
        root.path(),
        "ParentScript",
        "ScriptName ParentScript\n\nFunction DoThing(ObjectReference akTarget)\nEndFunction\n",
    );

    let mut table = FunctionTable::new(root.path().to_path_buf());
    let diagnostics = diagnostics_for(
        "argument-naming",
        "ScriptName Example Extends ParentScript\n\nFunction DoThing(ObjectReference akRef)\nEndFunction\n",
        &mut table,
    );

    assert_eq!(diagnostics.len(), 1);
    assert!(diagnostics[0].message.contains("Parameter 1 of 'DoThing'"));
    assert!(diagnostics[0].message.contains("named 'akRef'"));
    assert!(diagnostics[0].message.contains("names it 'akTarget'"));
}

#[test]
fn drives_the_argument_type_check_lint_across_scripts() {
    let root = tempfile::tempdir().expect("failed to create temp dir");
    write_script(
        root.path(),
        "Greeter",
        "ScriptName Greeter\n\nFunction Greet(String name)\nEndFunction\n",
    );

    let mut table = FunctionTable::new(root.path().to_path_buf());
    let diagnostics = papyrus_lints::check_argument_types(
        "ScriptName Example\n\nGreeter Property Target Auto\n\nFunction Test()\n    Target.Greet(1)\nEndFunction\n",
        &mut table,
    );

    assert_eq!(diagnostics.len(), 1);
    assert!(diagnostics[0].message.contains("Argument 1 to 'Greet'"));
    assert!(diagnostics[0].message.contains("expects String"));
    assert!(diagnostics[0].message.contains("got Int"));
}

#[test]
fn script_exists_true_for_a_script_found_under_the_project_root() {
    let root = tempfile::tempdir().expect("failed to create temp dir");
    write_script(root.path(), "Foo", "ScriptName Foo\n");

    let mut table = FunctionTable::new(root.path().to_path_buf());

    assert!(table.script_exists("Foo"));
    assert!(table.script_exists("foo"));
}

#[test]
fn script_exists_true_for_a_known_native_singleton_script() {
    let root = tempfile::tempdir().expect("failed to create temp dir");

    let mut table = FunctionTable::new(root.path().to_path_buf());

    assert!(table.script_exists("Game"));
    assert!(table.script_exists("utility"));
    assert!(table.script_exists("Debug"));
}

#[test]
fn script_exists_false_for_a_script_that_cannot_be_found() {
    let root = tempfile::tempdir().expect("failed to create temp dir");

    let mut table = FunctionTable::new(root.path().to_path_buf());

    assert!(!table.script_exists("MyMissingScript"));
}

#[test]
fn external_signature_trait_reports_builtin_native_and_project_types() {
    let root = tempfile::tempdir().expect("failed to create temp dir");
    write_script(
        root.path(),
        "Helpers",
        "ScriptName Helpers\n\nFunction Run() Global\nEndFunction\n",
    );

    let mut table = FunctionTable::new(root.path().to_path_buf());

    assert!(papyrus_lints::ExternalSignatures::type_exists(
        &mut table, "FLOAT"
    ));
    assert!(papyrus_lints::ExternalSignatures::type_exists(
        &mut table, "Actor"
    ));
    assert!(papyrus_lints::ExternalSignatures::type_exists(
        &mut table, "helpers"
    ));
    assert!(!papyrus_lints::ExternalSignatures::type_exists(
        &mut table,
        "DefinitelyMissing"
    ));
    assert_eq!(
        papyrus_lints::ExternalSignatures::is_global_function(&mut table, "Helpers", "Run"),
        Some(true)
    );
    assert_eq!(
        papyrus_lints::ExternalSignatures::is_global_function(&mut table, "Helpers", "Missing"),
        None
    );
}

#[test]
fn drives_the_unresolved_script_lint_across_scripts() {
    let root = tempfile::tempdir().expect("failed to create temp dir");
    write_script(
        root.path(),
        "Greeter",
        "ScriptName Greeter\n\nFunction Greet()\nEndFunction\n",
    );

    let mut table = FunctionTable::new(root.path().to_path_buf());
    let diagnostics = diagnostics_for(
        "unresolved-script",
        "ScriptName Example\n\nFunction Test()\n    Greeter.Greet()\n    Utility.Wait(1.0)\n    MyMissingScript.DoThing()\nEndFunction\n",
        &mut table,
    );

    assert_eq!(diagnostics.len(), 1);
    assert!(diagnostics[0]
        .message
        .contains("Script 'MyMissingScript' could not be located"));
}

#[test]
fn drives_a_named_argument_check_across_scripts_through_the_argument_type_check_lint() {
    let root = tempfile::tempdir().expect("failed to create temp dir");
    write_script(
        root.path(),
        "Greeter",
        "ScriptName Greeter\n\nFunction Greet(String name)\nEndFunction\n",
    );

    let mut table = FunctionTable::new(root.path().to_path_buf());
    let diagnostics = papyrus_lints::check_argument_types(
        "ScriptName Example\n\nGreeter Property Target Auto\n\nFunction Test()\n    Target.Greet(name = 1)\nEndFunction\n",
        &mut table,
    );

    assert_eq!(diagnostics.len(), 1);
    assert!(diagnostics[0].message.contains("Argument 1 to 'Greet'"));
    assert!(diagnostics[0].message.contains("expects String"));
    assert!(diagnostics[0].message.contains("got Int"));
}
