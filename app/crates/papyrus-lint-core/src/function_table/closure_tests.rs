use super::super::test_support::write_script;
use super::super::*;
use super::*;
use papyrus_lints::ExternalSignatures;
use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::RwLock;

struct SeedParse {
    source: String,
    ast: Option<papyrus_parser::ast::Script>,
}

fn parse_seed(path: &std::path::Path) -> SeedParse {
    let source = std::fs::read_to_string(path).unwrap_or_default();
    let ast = papyrus_parser::parse(&source).ok();
    SeedParse { source, ast }
}

fn close<'a>(
    table: &'a FunctionTable,
    seeds: &[PathBuf],
    threads: usize,
    total_files: Option<&'a AtomicUsize>,
) -> ClosedScripts<SeedParse> {
    table.parse_type_closure(
        seeds,
        TypeClosureOptions {
            threads,
            total_files,
        },
        parse_seed,
        |parsed| parsed.ast.as_ref(),
        || {},
    )
}

fn preload_closed(
    table: &mut FunctionTable,
    seeds: &[(PathBuf, SeedParse)],
    closed: &ClosedScripts<SeedParse>,
) {
    let entries = seeds
        .iter()
        .filter_map(|(path, parsed)| {
            let name_lower = path.file_stem()?.to_str()?.to_ascii_lowercase();
            Some(PreloadedScript {
                path,
                name_lower,
                ast: parsed.ast.as_ref(),
                source: &parsed.source,
            })
        })
        .collect();
    table.preload(entries);
    table.preload_dependencies(&closed.dependencies);
    table.preload_name_slots(&closed.bundled, &closed.unresolved);
}

#[test]
fn closure_parses_an_unlisted_parent_once_and_preloads_it() {
    let root = tempfile::tempdir().expect("failed to create temp dir");
    write_script(
        root.path(),
        "BaseScript",
        "ScriptName BaseScript\n\nFunction FromBaseScript()\nEndFunction\n",
    );
    write_script(
        root.path(),
        "Child",
        "ScriptName Child Extends BaseScript\n\nFunction Run()\n    BaseScript.FromBaseScript()\nEndFunction\n",
    );
    let child = root.path().join("scripts/source/Child.psc");
    let mut table = FunctionTable::new(root.path().to_path_buf());
    let total = AtomicUsize::new(1);
    let finished = AtomicUsize::new(0);
    let closed = table.parse_type_closure(
        std::slice::from_ref(&child),
        TypeClosureOptions {
            threads: 4,
            total_files: Some(&total),
        },
        parse_seed,
        |parsed| parsed.ast.as_ref(),
        || {
            finished.fetch_add(1, Ordering::SeqCst);
        },
    );

    assert_eq!(closed.seeds.len(), 1);
    assert_eq!(closed.dependencies.len(), 1);
    assert_eq!(closed.dependencies[0].name_lower, "basescript");
    assert!(closed.dependencies[0].ast.is_some());
    assert_eq!(total.load(Ordering::SeqCst), 2);
    assert_eq!(finished.load(Ordering::SeqCst), 2);

    let seed = (child, closed.seeds.into_iter().next().unwrap());
    preload_closed(
        &mut table,
        std::slice::from_ref(&seed),
        &ClosedScripts {
            seeds: Vec::new(),
            dependencies: closed.dependencies,
            bundled: closed.bundled,
            unresolved: closed.unresolved,
        },
    );

    assert!(matches!(
        table.lookup_function_cached("basescript", "frombasescript"),
        CacheProbe::Hit(Some(_))
    ));
    assert!(matches!(
        table.lookup_function_cached("child", "run"),
        CacheProbe::Hit(Some(_))
    ));
}

#[test]
fn closure_stops_on_a_cycle_and_a_missing_parent() {
    let root = tempfile::tempdir().expect("failed to create temp dir");
    write_script(root.path(), "A", "ScriptName A Extends B\n");
    write_script(root.path(), "B", "ScriptName B Extends A\n");
    write_script(
        root.path(),
        "Child",
        "ScriptName Child Extends MissingParent\n",
    );
    let mut table = FunctionTable::new(root.path().to_path_buf());
    let seeds = vec![
        root.path().join("scripts/source/A.psc"),
        root.path().join("scripts/source/Child.psc"),
    ];
    let closed = close(&table, &seeds, 1, None);
    assert_eq!(closed.dependencies.len(), 1);
    assert_eq!(closed.dependencies[0].name_lower, "b");
    assert!(closed.unresolved.iter().any(|name| name == "missingparent"));

    let parsed_seeds: Vec<_> = seeds.iter().cloned().zip(closed.seeds).collect();
    preload_closed(
        &mut table,
        &parsed_seeds,
        &ClosedScripts {
            seeds: Vec::new(),
            dependencies: closed.dependencies,
            bundled: closed.bundled.clone(),
            unresolved: closed.unresolved.clone(),
        },
    );

    assert!(matches!(
        table.ancestry_fully_known_cached("child"),
        CacheProbe::Hit(false)
    ));
    assert!(matches!(table.get_cached("missingparent"), Some(None)));
    // Already cached: a later write-path load must not be required to answer.
    assert!(!table.ancestry_fully_known("child"));
}

#[test]
fn closure_prefers_a_project_script_over_a_lookup_root_of_the_same_name() {
    let root = tempfile::tempdir().expect("failed to create temp dir");
    write_script(
        root.path(),
        "BaseScript",
        "ScriptName BaseScript\n\nFunction FromProject()\nEndFunction\n",
    );
    let lookup = tempfile::tempdir().expect("failed to create temp dir");
    std::fs::write(
        lookup.path().join("BaseScript.psc"),
        "ScriptName BaseScript\n\nFunction FromLookup()\nEndFunction\n",
    )
    .unwrap();
    write_script(
        root.path(),
        "Child",
        "ScriptName Child Extends BaseScript\n",
    );
    let mut table = FunctionTable::new(root.path().to_path_buf())
        .with_lookup_roots(vec![lookup.path().to_string_lossy().into_owned()]);
    let child = root.path().join("scripts/source/Child.psc");
    let closed = close(&table, std::slice::from_ref(&child), 2, None);
    assert_eq!(closed.dependencies.len(), 1);
    assert_eq!(
        closed.dependencies[0].path,
        root.path().join("scripts/source/BaseScript.psc")
    );
    let seed = (child, closed.seeds.into_iter().next().unwrap());
    preload_closed(
        &mut table,
        std::slice::from_ref(&seed),
        &ClosedScripts {
            seeds: Vec::new(),
            dependencies: closed.dependencies,
            bundled: closed.bundled,
            unresolved: closed.unresolved,
        },
    );
    assert!(matches!(
        table.lookup_function_cached("basescript", "fromproject"),
        CacheProbe::Hit(Some(_))
    ));
    assert!(matches!(
        table.lookup_function_cached("basescript", "fromlookup"),
        CacheProbe::Hit(None)
    ));
}

#[test]
fn closure_parses_a_lookup_root_parent_without_making_it_a_seed() {
    let root = tempfile::tempdir().expect("failed to create temp dir");
    let lookup = tempfile::tempdir().expect("failed to create temp dir");
    std::fs::write(
        lookup.path().join("BaseScript.psc"),
        "ScriptName BaseScript\n\nFunction FromLookup()\nEndFunction\n",
    )
    .unwrap();
    write_script(
        root.path(),
        "Child",
        "ScriptName Child Extends BaseScript\n",
    );
    let mut table = FunctionTable::new(root.path().to_path_buf())
        .with_lookup_roots(vec![lookup.path().to_string_lossy().into_owned()]);
    let child = root.path().join("scripts/source/Child.psc");
    let closed = close(&table, std::slice::from_ref(&child), 2, None);
    assert_eq!(closed.seeds.len(), 1);
    assert_eq!(closed.dependencies.len(), 1);
    assert_eq!(
        closed.dependencies[0].path,
        lookup.path().join("BaseScript.psc")
    );
    let seed = (child, closed.seeds.into_iter().next().unwrap());
    preload_closed(
        &mut table,
        std::slice::from_ref(&seed),
        &ClosedScripts {
            seeds: Vec::new(),
            dependencies: closed.dependencies,
            bundled: closed.bundled,
            unresolved: closed.unresolved,
        },
    );
    assert!(matches!(
        table.lookup_function_cached("basescript", "fromlookup"),
        CacheProbe::Hit(Some(_))
    ));
}

#[test]
fn closure_loads_a_bundled_extends_chain_without_ensure_loaded() {
    let root = tempfile::tempdir().expect("failed to create temp dir");
    write_script(
        root.path(),
        "MyQuestScript",
        "ScriptName MyQuestScript Extends Quest\n",
    );
    let mut table = FunctionTable::new(root.path().to_path_buf());
    let seed_path = root.path().join("scripts/source/MyQuestScript.psc");
    let closed = close(&table, std::slice::from_ref(&seed_path), 2, None);
    assert!(closed.dependencies.is_empty());
    assert!(closed.bundled.iter().any(|name| name == "quest"));
    assert!(closed.bundled.iter().any(|name| name == "form"));
    let seed = (seed_path, closed.seeds.into_iter().next().unwrap());
    preload_closed(
        &mut table,
        std::slice::from_ref(&seed),
        &ClosedScripts {
            seeds: Vec::new(),
            dependencies: closed.dependencies,
            bundled: closed.bundled,
            unresolved: closed.unresolved,
        },
    );
    assert!(matches!(
        table.is_subtype_cached("myquestscript", "form"),
        CacheProbe::Hit(true)
    ));
    assert!(matches!(
        table.ancestry_fully_known_cached("myquestscript"),
        CacheProbe::Hit(true)
    ));
}

#[test]
fn closure_preloaded_parent_answers_under_a_read_lock() {
    let root = tempfile::tempdir().expect("failed to create temp dir");
    write_script(
        root.path(),
        "BaseScript",
        "ScriptName BaseScript\n\nFunction FromBaseScript()\nEndFunction\n",
    );
    write_script(
        root.path(),
        "Child",
        "ScriptName Child Extends BaseScript\n",
    );
    let child = root.path().join("scripts/source/Child.psc");
    let mut table = FunctionTable::new(root.path().to_path_buf());
    let closed = close(&table, std::slice::from_ref(&child), 2, None);
    let seed = (child, closed.seeds.into_iter().next().unwrap());
    preload_closed(
        &mut table,
        std::slice::from_ref(&seed),
        &ClosedScripts {
            seeds: Vec::new(),
            dependencies: closed.dependencies,
            bundled: closed.bundled,
            unresolved: closed.unresolved,
        },
    );
    let table = RwLock::new(table);
    let mut shared = SharedFunctionTable(&table);
    let _held = table
        .read()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    assert!(shared.is_subtype("Child", "BaseScript"));
    assert!(shared.lookup("BaseScript", "FromBaseScript").is_some());
    assert!(shared
        .function_access("BaseScript", "FromBaseScript")
        .is_some());
    assert!(shared.ancestry_fully_known("Child"));
}

#[test]
fn preload_name_slots_does_not_hide_a_higher_priority_file_or_a_bundled_script() {
    let root = tempfile::tempdir().expect("failed to create temp dir");
    write_script(
        root.path(),
        "Example",
        "ScriptName Example\n\nFunction FromProject()\nEndFunction\n",
    );
    let mut table = FunctionTable::new(root.path().to_path_buf());
    table.preload_name_slots(&["example".to_string()], &["actor".to_string()]);
    assert!(table.get_cached("example").is_none());
    assert!(table.get_cached("actor").is_none());
    assert!(table.lookup_function("Example", "FromProject").is_some());
    assert!(table.lookup_function("Actor", "GetActorValue").is_some());
}

#[test]
fn closure_does_not_let_a_losing_seed_hide_the_resolved_script() {
    let root = tempfile::tempdir().expect("failed to create temp dir");
    write_script(
        root.path(),
        "BaseScript",
        "ScriptName BaseScript\n\nFunction FromProject()\nEndFunction\n",
    );
    write_script(
        root.path(),
        "Child",
        "ScriptName Child Extends BaseScript\n",
    );
    let loose_dir = root.path().join("loose");
    std::fs::create_dir_all(&loose_dir).unwrap();
    let loose = loose_dir.join("BaseScript.psc");
    std::fs::write(
        &loose,
        "ScriptName BaseScript\n\nFunction FromLoose()\nEndFunction\n",
    )
    .unwrap();

    let mut table = FunctionTable::new(root.path().to_path_buf());
    let child = root.path().join("scripts/source/Child.psc");
    let closed = close(&table, &[loose.clone(), child.clone()], 2, None);
    let project_parent = root.path().join("scripts/source/BaseScript.psc");
    assert!(
        closed
            .dependencies
            .iter()
            .any(|dependency| dependency.path == project_parent),
        "the higher-priority BaseScript.psc must still be parsed"
    );
    assert!(closed
        .dependencies
        .iter()
        .all(|dependency| dependency.path != loose));

    let parsed_seeds: Vec<_> = [loose, child].into_iter().zip(closed.seeds).collect();
    preload_closed(
        &mut table,
        &parsed_seeds,
        &ClosedScripts {
            seeds: Vec::new(),
            dependencies: closed.dependencies,
            bundled: closed.bundled,
            unresolved: closed.unresolved,
        },
    );
    assert!(matches!(
        table.lookup_function_cached("basescript", "fromproject"),
        CacheProbe::Hit(Some(_))
    ));
    assert!(matches!(
        table.lookup_function_cached("basescript", "fromloose"),
        CacheProbe::Hit(None)
    ));
}

#[test]
fn preloading_a_lookup_root_script_fills_the_process_wide_cache() {
    let root = tempfile::tempdir().expect("failed to create temp dir");
    let lookup = tempfile::tempdir().expect("failed to create temp dir");
    let path = lookup.path().join("BaseScript.psc");
    std::fs::write(
        &path,
        "ScriptName BaseScript\n\nFunction FromFirst()\nEndFunction\n",
    )
    .unwrap();
    let mtime = std::fs::metadata(&path).unwrap().modified().expect("mtime");
    write_script(
        root.path(),
        "Child",
        "ScriptName Child Extends BaseScript\n",
    );

    let mut first = FunctionTable::new(root.path().to_path_buf())
        .with_lookup_roots(vec![lookup.path().to_string_lossy().into_owned()]);
    let child = root.path().join("scripts/source/Child.psc");
    let closed = close(&first, std::slice::from_ref(&child), 1, None);
    let seed = (child, closed.seeds.into_iter().next().unwrap());
    preload_closed(
        &mut first,
        std::slice::from_ref(&seed),
        &ClosedScripts {
            seeds: Vec::new(),
            dependencies: closed.dependencies,
            bundled: closed.bundled,
            unresolved: closed.unresolved,
        },
    );
    assert!(matches!(
        first.lookup_function_cached("basescript", "fromfirst"),
        CacheProbe::Hit(Some(_))
    ));

    std::fs::write(
        &path,
        "ScriptName BaseScript\n\nFunction FromSecond()\nEndFunction\n",
    )
    .unwrap();
    let file = std::fs::File::open(&path).unwrap();
    if file.set_modified(mtime).is_err() {
        return;
    }
    drop(file);

    let mut second = FunctionTable::new(root.path().to_path_buf())
        .with_lookup_roots(vec![lookup.path().to_string_lossy().into_owned()]);
    assert!(
        second.lookup_function("BaseScript", "FromFirst").is_some(),
        "unchanged mtime should reuse the process-wide lookup-script cache"
    );
    assert!(second.lookup_function("BaseScript", "FromSecond").is_none());
}
