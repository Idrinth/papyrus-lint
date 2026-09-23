use super::*;
use std::fs;
use std::path::PathBuf;

#[test]
fn cache_probe_map_transforms_hits_and_preserves_misses() {
    let hit = CacheProbe::Hit(2).map(|value| value * 3);
    assert!(matches!(hit, CacheProbe::Hit(6)));

    let miss: CacheProbe<i32> = CacheProbe::Miss;
    assert!(matches!(miss.map(|value| value * 3), CacheProbe::Miss));
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

    let table = FunctionTable::new(root.path().to_path_buf())
        .with_known_scripts(&[invalid_path, valid_path]);

    assert!(table.script_exists("Example"));
}

#[test]
fn preload_merges_a_script_whose_path_matches_the_tables_own_resolution() {
    let dir = tempfile::tempdir().expect("failed to create temp dir");
    let source_dir = dir.path().join("scripts/source");
    fs::create_dir_all(&source_dir).expect("failed to create scripts/source");
    let path = source_dir.join("Example.psc");
    let source = "ScriptName Example\n\nFunction DoIt()\nEndFunction\n";
    fs::write(&path, source).expect("failed to write script");
    let ast = papyrus_parser::parse(source).expect("fixture should parse");

    let mut table = FunctionTable::new(dir.path().to_path_buf());
    table.preload(vec![PreloadedScript {
        path: &path,
        name_lower: "example".to_string(),
        ast: Some(&ast),
        source,
    }]);

    // Populated directly from the preloaded AST, without ever calling
    // `ensure_loaded` (which would also resolve/parse it, but through the
    // write-locked on-demand path this exists to avoid).
    assert!(matches!(
        table.lookup_function_cached("example", "doit"),
        CacheProbe::Hit(Some(_))
    ));
}

#[test]
fn preload_ignores_a_caller_supplied_path_that_disagrees_with_the_tables_own_resolution() {
    let dir = tempfile::tempdir().expect("failed to create temp dir");
    let source_dir = dir.path().join("scripts/source");
    fs::create_dir_all(&source_dir).expect("failed to create scripts/source");
    let real_path = source_dir.join("Example.psc");
    let real_source = "ScriptName Example\n\nFunction DoIt()\nEndFunction\n";
    fs::write(&real_path, real_source).expect("failed to write script");

    // A caller-supplied path that does not match where this table would
    // actually resolve "Example" to (e.g. a stale copy from elsewhere) --
    // `preload` must not trust it, and must leave the name unresolved for
    // `ensure_loaded` to pick up correctly later instead.
    let wrong_path = dir.path().join("Elsewhere.psc");
    let wrong_source = "ScriptName Example\n";
    let wrong_ast = papyrus_parser::parse(wrong_source).expect("fixture should parse");

    let mut table = FunctionTable::new(dir.path().to_path_buf());
    table.preload(vec![PreloadedScript {
        path: &wrong_path,
        name_lower: "example".to_string(),
        ast: Some(&wrong_ast),
        source: wrong_source,
    }]);

    assert!(table.get_cached("example").is_none());

    table.ensure_loaded("example");
    assert!(matches!(
        table.lookup_function_cached("example", "doit"),
        CacheProbe::Hit(Some(_))
    ));
}

#[test]
fn preload_caches_an_unparseable_script_as_unresolved() {
    let dir = tempfile::tempdir().expect("failed to create temp dir");
    let source_dir = dir.path().join("scripts/source");
    fs::create_dir_all(&source_dir).expect("failed to create scripts/source");
    let path = source_dir.join("Broken.psc");
    fs::write(&path, "not valid Papyrus").expect("failed to write script");

    let mut table = FunctionTable::new(dir.path().to_path_buf());
    table.preload(vec![PreloadedScript {
        path: &path,
        name_lower: "broken".to_string(),
        ast: None,
        source: "not valid Papyrus",
    }]);

    assert!(matches!(table.get_cached("broken"), Some(None)));
}

#[test]
fn preload_keeps_the_first_entry_for_a_duplicate_name() {
    let dir = tempfile::tempdir().expect("failed to create temp dir");
    let source_dir = dir.path().join("scripts/source");
    fs::create_dir_all(&source_dir).expect("failed to create scripts/source");
    let path = source_dir.join("Example.psc");
    let first_source = "ScriptName Example\n\nFunction First()\nEndFunction\n";
    let second_source = "ScriptName Example\n\nFunction Second()\nEndFunction\n";
    fs::write(&path, first_source).expect("failed to write script");
    let first_ast = papyrus_parser::parse(first_source).expect("first fixture should parse");
    let second_ast = papyrus_parser::parse(second_source).expect("second fixture should parse");

    let mut table = FunctionTable::new(dir.path().to_path_buf());
    table.preload(vec![
        PreloadedScript {
            path: &path,
            name_lower: "example".to_string(),
            ast: Some(&first_ast),
            source: first_source,
        },
        PreloadedScript {
            path: &path,
            name_lower: "example".to_string(),
            ast: Some(&second_ast),
            source: second_source,
        },
    ]);

    assert!(matches!(
        table.lookup_function_cached("example", "first"),
        CacheProbe::Hit(Some(_))
    ));
    assert!(matches!(
        table.lookup_function_cached("example", "second"),
        CacheProbe::Hit(None)
    ));
}
