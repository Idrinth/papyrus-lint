use super::*;

use papyrus_lint_globals::Game;

const GAME: Game = Game::Skyrim;

fn write_file(dir: &Path, name: &str) -> PathBuf {
    let path = dir.join(name);
    fs::write(&path, "").expect("failed to write test script file");
    path
}

#[test]
fn finds_exact_match_in_scripts_source() {
    let root = tempfile::tempdir().expect("failed to create temp dir");
    let source_dir = root.path().join("scripts/source");
    fs::create_dir_all(&source_dir).expect("failed to create source dir");
    let expected = write_file(&source_dir, "Foo.psc");

    let result = find_psc_file(root.path(), "Foo.psc", &[]);

    assert_eq!(result, Some(expected));
}

#[test]
fn finds_case_insensitive_match_in_source_scripts() {
    let root = tempfile::tempdir().expect("failed to create temp dir");
    let source_dir = root.path().join("source/scripts");
    fs::create_dir_all(&source_dir).expect("failed to create source dir");
    let expected = write_file(&source_dir, "Foo.psc");

    let result = find_psc_file(root.path(), "fOO.PSC", &[]);

    assert_eq!(result, Some(expected));
}

#[test]
fn appends_extension_when_omitted() {
    let root = tempfile::tempdir().expect("failed to create temp dir");
    let source_dir = root.path().join("scripts/source");
    fs::create_dir_all(&source_dir).expect("failed to create source dir");
    let expected = write_file(&source_dir, "Foo.psc");

    let result = find_psc_file(root.path(), "foo", &[]);

    assert_eq!(result, Some(expected));
}

#[test]
fn prefers_scripts_source_over_source_scripts() {
    let root = tempfile::tempdir().expect("failed to create temp dir");
    let scripts_source = root.path().join("scripts/source");
    let source_scripts = root.path().join("source/scripts");
    fs::create_dir_all(&scripts_source).expect("failed to create scripts/source dir");
    fs::create_dir_all(&source_scripts).expect("failed to create source/scripts dir");
    let expected = write_file(&scripts_source, "Foo.psc");
    write_file(&source_scripts, "Foo.psc");

    let result = find_psc_file(root.path(), "Foo.psc", &[]);

    assert_eq!(result, Some(expected));
}

#[test]
fn returns_none_when_no_match_found() {
    let root = tempfile::tempdir().expect("failed to create temp dir");
    let source_dir = root.path().join("scripts/source");
    fs::create_dir_all(&source_dir).expect("failed to create source dir");
    write_file(&source_dir, "Bar.psc");

    let result = find_psc_file(root.path(), "Foo.psc", &[]);

    assert_eq!(result, None);
}

#[test]
fn returns_none_when_neither_directory_exists() {
    let root = tempfile::tempdir().expect("failed to create temp dir");

    let result = find_psc_file(root.path(), "Foo.psc", &[]);

    assert_eq!(result, None);
}

#[test]
fn ignores_a_directory_with_the_target_filename() {
    let root = tempfile::tempdir().expect("failed to create temp dir");
    let source_dir = root.path().join("scripts/source");
    fs::create_dir_all(source_dir.join("Foo.psc")).expect("failed to create misleading directory");

    let result = find_psc_file(root.path(), "Foo.psc", &[]);

    assert_eq!(result, None);
}

#[test]
fn skips_non_directory_search_roots() {
    let root = tempfile::tempdir().expect("failed to create temp dir");
    let additional_root = root.path().join("not-a-directory");
    fs::write(&additional_root, "content").expect("failed to create regular file");

    let result = find_psc_file(
        root.path(),
        "Foo.psc",
        &[additional_root.to_string_lossy().into_owned()],
    );

    assert_eq!(result, None);
}

#[test]
fn finds_match_in_an_additional_root_relative_to_the_project_root() {
    let root = tempfile::tempdir().expect("failed to create temp dir");
    let shared_dir = root.path().join("../SharedScripts");
    fs::create_dir_all(&shared_dir).expect("failed to create shared dir");
    write_file(&shared_dir, "Foo.psc");

    let result = find_psc_file(root.path(), "Foo.psc", &["../SharedScripts".to_string()]);

    assert!(result.is_some());
}

#[test]
fn finds_match_in_an_absolute_additional_root() {
    let root = tempfile::tempdir().expect("failed to create temp dir");
    let shared = tempfile::tempdir().expect("failed to create temp dir");
    let expected = write_file(shared.path(), "Foo.psc");

    let result = find_psc_file(
        root.path(),
        "Foo.psc",
        &[shared.path().to_string_lossy().into_owned()],
    );

    assert_eq!(result, Some(expected));
}

#[test]
fn prefers_candidate_dirs_over_additional_roots() {
    let root = tempfile::tempdir().expect("failed to create temp dir");
    let scripts_source = root.path().join("scripts/source");
    let shared = tempfile::tempdir().expect("failed to create temp dir");
    fs::create_dir_all(&scripts_source).expect("failed to create scripts/source dir");
    let expected = write_file(&scripts_source, "Foo.psc");
    write_file(shared.path(), "Foo.psc");

    let result = find_psc_file(
        root.path(),
        "Foo.psc",
        &[shared.path().to_string_lossy().into_owned()],
    );

    assert_eq!(result, Some(expected));
}

#[test]
fn searches_additional_roots_in_order() {
    let root = tempfile::tempdir().expect("failed to create temp dir");
    let first = tempfile::tempdir().expect("failed to create temp dir");
    let second = tempfile::tempdir().expect("failed to create temp dir");
    let expected = write_file(first.path(), "Foo.psc");
    write_file(second.path(), "Foo.psc");

    let result = find_psc_file(
        root.path(),
        "Foo.psc",
        &[
            first.path().to_string_lossy().into_owned(),
            second.path().to_string_lossy().into_owned(),
        ],
    );

    assert_eq!(result, Some(expected));
}

#[test]
fn searches_scripts_source_beside_an_added_source_scripts_root() {
    let root = tempfile::tempdir().expect("failed to create temp dir");
    let scripts_source = root.path().join("shared/scripts/source");
    fs::create_dir_all(&scripts_source).expect("failed to create scripts/source dir");
    let expected = write_file(&scripts_source, "Foo.psc");

    let result = find_psc_file(root.path(), "Foo.psc", &["shared/source/scripts".into()]);

    assert_eq!(result, Some(expected));
}

#[test]
fn searches_source_scripts_beside_an_added_scripts_source_root() {
    let root = tempfile::tempdir().expect("failed to create temp dir");
    let source_scripts = root.path().join("shared/source/scripts");
    fs::create_dir_all(&source_scripts).expect("failed to create source/scripts dir");
    let expected = write_file(&source_scripts, "Foo.psc");

    let result = find_psc_file(root.path(), "Foo.psc", &["shared/scripts/source".into()]);

    assert_eq!(result, Some(expected));
}

#[test]
fn resolve_additional_roots_joins_relative_and_keeps_absolute_paths() {
    let root = tempfile::tempdir().expect("failed to create temp dir");
    let relative = root.path().join("../SharedScripts");
    let absolute = tempfile::tempdir().expect("failed to create absolute root");
    fs::create_dir_all(&relative).expect("failed to create relative root");

    let resolved = resolve_additional_roots(
        root.path(),
        &[
            "../SharedScripts".to_string(),
            absolute.path().to_string_lossy().into_owned(),
        ],
    );

    assert_eq!(resolved, vec![relative, absolute.path().to_path_buf()]);
}

#[test]
fn resolve_additional_roots_adds_counterparts_without_duplicates() {
    let root = tempfile::tempdir().expect("failed to create temp dir");
    let source_scripts = root.path().join("Shared/source/scripts");
    let scripts_source = root.path().join("Shared/scripts/source");
    fs::create_dir_all(&source_scripts).expect("failed to create source/scripts dir");
    fs::create_dir_all(&scripts_source).expect("failed to create scripts/source dir");

    let resolved = resolve_additional_roots(
        root.path(),
        &[
            "Shared/source/scripts".to_string(),
            "Shared/scripts/source".to_string(),
        ],
    );

    assert_eq!(resolved, vec![source_scripts, scripts_source]);
}

#[test]
fn resolve_additional_roots_omits_nonexistent_directories() {
    let root = tempfile::tempdir().expect("failed to create temp dir");

    assert!(resolve_additional_roots(root.path(), &["missing".to_string()]).is_empty());
}

#[test]
fn detected_script_roots_returns_only_existing_search_directories() {
    let root = tempfile::tempdir().expect("failed to create temp dir");
    let conventional = root.path().join("scripts/source");
    let additional = root.path().join("shared");
    fs::create_dir_all(&conventional).expect("failed to create conventional root");
    fs::create_dir_all(&additional).expect("failed to create additional root");

    assert_eq!(
        detected_script_roots(root.path(), &["shared".to_string(), "missing".to_string()]),
        vec![conventional, additional]
    );
}

#[test]
fn detected_script_roots_excludes_regular_files() {
    let root = tempfile::tempdir().expect("failed to create temp dir");
    let regular_file = root.path().join("shared");
    fs::write(&regular_file, "content").expect("failed to create regular file");

    assert!(detected_script_roots(root.path(), &["shared".to_string()]).is_empty());
}

#[test]
fn find_psc_files_recursively_finds_files_at_every_nesting_depth() {
    let root = tempfile::tempdir().expect("failed to create temp dir");
    write_file(root.path(), "Top.psc");
    let nested = root.path().join("Requiem/Sub");
    fs::create_dir_all(&nested).expect("failed to create nested dir");
    write_file(&nested, "Nested.psc");

    let mut result = find_psc_files_recursively(root.path());
    result.sort();
    let mut expected = vec![
        root.path().join("Requiem/Sub/Nested.psc"),
        root.path().join("Top.psc"),
    ];
    expected.sort();

    assert_eq!(result, expected);
}

#[test]
fn find_psc_files_recursively_matches_the_extension_case_insensitively() {
    let root = tempfile::tempdir().expect("failed to create temp dir");
    write_file(root.path(), "Example.PSC");

    assert_eq!(
        find_psc_files_recursively(root.path()),
        vec![root.path().join("Example.PSC")]
    );
}

#[test]
fn find_psc_files_recursively_ignores_non_psc_files() {
    let root = tempfile::tempdir().expect("failed to create temp dir");
    write_file(root.path(), "Example.pex");

    assert!(find_psc_files_recursively(root.path()).is_empty());
}

#[test]
fn find_psc_files_recursively_returns_empty_for_a_missing_directory() {
    let root = tempfile::tempdir().expect("failed to create temp dir");

    assert!(find_psc_files_recursively(&root.path().join("missing")).is_empty());
}

#[test]
fn warns_about_same_named_scripts_with_different_contents() {
    let root = tempfile::tempdir().expect("failed to create temp dir");
    let primary = root.path().join("scripts/source");
    let alternate = root.path().join("source/scripts");
    fs::create_dir_all(&primary).expect("failed to create primary root");
    fs::create_dir_all(&alternate).expect("failed to create alternate root");
    let script = write_file(&primary, "Example.psc");
    fs::write(&script, "ScriptName Example\n").expect("failed to write primary script");
    fs::write(alternate.join("example.PSC"), "ScriptName ExampleV2\n")
        .expect("failed to write alternate script");

    let diagnostics = conflicting_script_versions(&script, root.path(), &[], false, GAME);

    assert_eq!(diagnostics.len(), 1);
    assert_eq!(diagnostics[0].rule, CONFLICTING_SCRIPT_VERSIONS_RULE);
    assert!(diagnostics[0].message.contains("example.PSC"));
}

#[test]
fn warns_about_same_named_scripts_with_different_contents_and_shortens_the_path() {
    let root = tempfile::tempdir().expect("failed to create temp dir");
    let primary = root.path().join("scripts/source");
    let alternate = root.path().join("source/scripts");
    fs::create_dir_all(&primary).expect("failed to create primary root");
    fs::create_dir_all(&alternate).expect("failed to create alternate root");
    let script = write_file(&primary, "Example.psc");
    fs::write(&script, "ScriptName Example\n").expect("failed to write primary script");
    fs::write(alternate.join("example.PSC"), "ScriptName ExampleV2\n")
        .expect("failed to write alternate script");

    let diagnostics = conflicting_script_versions(&script, root.path(), &[], true, GAME);

    assert_eq!(diagnostics.len(), 1);
    let shortened = Path::new("source/scripts/example.PSC");
    assert!(diagnostics[0]
        .message
        .contains(&shortened.display().to_string()));
    assert!(!diagnostics[0]
        .message
        .contains(&root.path().display().to_string()));
}

#[test]
fn ignores_identical_same_named_scripts() {
    let root = tempfile::tempdir().expect("failed to create temp dir");
    let primary = root.path().join("scripts/source");
    let alternate = root.path().join("source/scripts");
    fs::create_dir_all(&primary).expect("failed to create primary root");
    fs::create_dir_all(&alternate).expect("failed to create alternate root");
    let script = write_file(&primary, "Example.psc");
    fs::write(&script, "same").expect("failed to write primary script");
    fs::write(alternate.join("Example.psc"), "same").expect("failed to write alternate script");

    assert!(conflicting_script_versions(&script, root.path(), &[], false, GAME).is_empty());
}

#[test]
fn find_psc_file_in_lookup_roots_finds_a_script_outside_the_project() {
    let root = tempfile::tempdir().expect("failed to create temp dir");
    let vanilla = tempfile::tempdir().expect("failed to create temp dir");
    let expected = write_file(vanilla.path(), "Actor.psc");

    let result = find_psc_file_in_lookup_roots(
        root.path(),
        "Actor",
        &[vanilla.path().to_string_lossy().into_owned()],
    );

    assert_eq!(result, Some(expected));
    assert_eq!(find_psc_file(root.path(), "Actor", &[]), None);
}

#[test]
fn conflicting_script_versions_ignores_lookup_roots() {
    let root = tempfile::tempdir().expect("failed to create temp dir");
    let primary = root.path().join("scripts/source");
    fs::create_dir_all(&primary).expect("failed to create primary root");
    let script = write_file(&primary, "Actor.psc");
    fs::write(&script, "ScriptName Actor\n").expect("failed to write project script");

    let vanilla = tempfile::tempdir().expect("failed to create temp dir");
    fs::write(
        vanilla.path().join("Actor.psc"),
        "ScriptName Actor Native\n",
    )
    .expect("failed to write vanilla script");
    let lookup = vec![vanilla.path().to_string_lossy().into_owned()];

    assert!(conflicting_script_versions(&script, root.path(), &[], false, GAME).is_empty());
    assert!(find_psc_file_in_lookup_roots(root.path(), "Actor", &lookup).is_some());
    assert_eq!(
        conflicting_script_versions(&script, root.path(), &lookup, false, GAME).len(),
        1,
        "additional_script_roots still report collisions, unlike lookup roots"
    );
}

#[test]
fn cached_lookup_index_rebuilds_after_a_new_file_is_added() {
    let root = tempfile::tempdir().expect("failed to create temp dir");
    let vanilla = tempfile::tempdir().expect("failed to create temp dir");
    write_file(vanilla.path(), "Actor.psc");
    let lookup = vec![vanilla.path().to_string_lossy().into_owned()];

    let first = cached_lookup_index(root.path(), &lookup);
    assert!(first.contains_key("actor.psc"));
    assert!(!first.contains_key("form.psc"));

    write_file(vanilla.path(), "Form.psc");
    let later = std::time::SystemTime::now() + std::time::Duration::from_secs(2);
    if let Ok(dir) = fs::File::open(vanilla.path()) {
        let _ = dir.set_modified(later);
    }

    let second = cached_lookup_index(root.path(), &lookup);
    assert!(
        second.contains_key("form.psc"),
        "a newer directory mtime must invalidate the cached lookup index"
    );
    assert!(!std::sync::Arc::ptr_eq(&first, &second));
}

#[test]
fn cached_script_index_reuses_a_scan_until_a_source_directory_changes() {
    let root = tempfile::tempdir().expect("failed to create temp dir");
    let source = root.path().join("scripts/source");
    fs::create_dir_all(&source).expect("failed to create source dir");
    write_file(&source, "Example.psc");

    let first = cached_script_index(root.path(), &[]);
    assert!(first.contains_key("example.psc"));
    assert!(std::sync::Arc::ptr_eq(
        &first,
        &cached_script_index(root.path(), &[])
    ));

    write_file(&source, "Other.psc");
    let later = std::time::SystemTime::now() + std::time::Duration::from_secs(2);
    if let Ok(dir) = fs::File::open(&source) {
        let _ = dir.set_modified(later);
    }

    let second = cached_script_index(root.path(), &[]);
    assert!(
        second.contains_key("other.psc"),
        "a newer directory mtime must invalidate the cached script index"
    );
    assert!(!std::sync::Arc::ptr_eq(&first, &second));
}

#[test]
fn reports_conflicts_from_additional_roots_in_sorted_path_order() {
    let root = tempfile::tempdir().expect("failed to create temp dir");
    let primary = root.path().join("scripts/source");
    let first_alphabetically = root.path().join("a-scripts");
    let second_alphabetically = root.path().join("z-scripts");
    fs::create_dir_all(&primary).expect("failed to create primary root");
    fs::create_dir_all(&first_alphabetically).expect("failed to create first root");
    fs::create_dir_all(&second_alphabetically).expect("failed to create second root");
    let script = primary.join("Example.psc");
    fs::write(&script, "primary").expect("failed to write primary script");
    fs::write(first_alphabetically.join("example.psc"), "first")
        .expect("failed to write first alternate");
    fs::write(second_alphabetically.join("EXAMPLE.PSC"), "second")
        .expect("failed to write second alternate");

    let diagnostics = conflicting_script_versions(
        &script,
        root.path(),
        &["z-scripts".into(), "a-scripts".into()],
        false,
        GAME,
    );

    assert_eq!(diagnostics.len(), 2);
    assert!(diagnostics[0]
        .message
        .contains(&first_alphabetically.display().to_string()));
    assert!(diagnostics[1]
        .message
        .contains(&second_alphabetically.display().to_string()));
    assert!(diagnostics
        .iter()
        .all(|diagnostic| diagnostic.line == 1 && diagnostic.column == 1));
}

#[test]
fn conflict_check_returns_empty_for_an_unreadable_script_path() {
    let root = tempfile::tempdir().expect("failed to create temp dir");

    assert!(conflicting_script_versions(
        &root.path().join("Missing.psc"),
        root.path(),
        &[],
        false,
        GAME
    )
    .is_empty());
}

#[test]
fn conflict_check_ignores_same_named_directories() {
    let root = tempfile::tempdir().expect("failed to create temp dir");
    let primary = root.path().join("scripts/source");
    let alternate = root.path().join("source/scripts");
    fs::create_dir_all(&primary).expect("failed to create primary root");
    fs::create_dir_all(alternate.join("Example.psc"))
        .expect("failed to create same-named directory");
    let script = primary.join("Example.psc");
    fs::write(&script, "primary").expect("failed to write primary script");

    assert!(conflicting_script_versions(&script, root.path(), &[], false, GAME).is_empty());
}

#[test]
fn build_script_index_groups_files_by_lowercased_name_across_search_roots() {
    let root = tempfile::tempdir().expect("failed to create temp dir");
    let primary = root.path().join("scripts/source");
    let alternate = root.path().join("source/scripts");
    fs::create_dir_all(&primary).expect("failed to create primary root");
    fs::create_dir_all(&alternate).expect("failed to create alternate root");
    let script = write_file(&primary, "Example.psc");
    let other = write_file(&alternate, "example.PSC");
    write_file(&primary, "Unrelated.psc");

    let index = build_script_index(root.path(), &[]);

    let mut matches = index
        .get("example.psc")
        .expect("expected an entry for example.psc")
        .clone();
    matches.sort();
    let mut expected = vec![script, other];
    expected.sort();
    assert_eq!(matches, expected);
}

#[test]
fn indexed_lookup_is_case_insensitive_and_preserves_search_order() {
    let first = PathBuf::from("first/Example.psc");
    let second = PathBuf::from("second/example.PSC");
    let index = HashMap::from([("example.psc".to_string(), vec![first.clone(), second])]);

    assert_eq!(find_psc_file_in_index(&index, "EXAMPLE"), Some(first));
    assert_eq!(find_psc_file_in_index(&index, "missing.psc"), None);
}

#[test]
fn build_script_index_ignores_directories_and_missing_search_roots() {
    let root = tempfile::tempdir().expect("failed to create temp dir");
    let primary = root.path().join("scripts/source");
    fs::create_dir_all(primary.join("Example.psc")).expect("failed to create same-named directory");

    let index = build_script_index(root.path(), &[]);

    assert!(!index.contains_key("example.psc"));
}

#[test]
fn conflicting_script_versions_in_index_flags_a_same_named_entry_with_different_content() {
    let root = tempfile::tempdir().expect("failed to create temp dir");
    let primary = root.path().join("scripts/source");
    let alternate = root.path().join("source/scripts");
    fs::create_dir_all(&primary).expect("failed to create primary root");
    fs::create_dir_all(&alternate).expect("failed to create alternate root");
    let script = write_file(&primary, "Example.psc");
    fs::write(&script, "ScriptName Example\n").expect("failed to write primary script");
    fs::write(alternate.join("example.PSC"), "ScriptName ExampleV2\n")
        .expect("failed to write alternate script");
    let index = build_script_index(root.path(), &[]);

    let diagnostics =
        conflicting_script_versions_in_index(&script, &index, root.path(), false, GAME);

    assert_eq!(diagnostics.len(), 1);
    assert_eq!(diagnostics[0].rule, CONFLICTING_SCRIPT_VERSIONS_RULE);
    assert!(diagnostics[0].message.contains("example.PSC"));
}

#[test]
fn conflicting_script_versions_in_index_matches_conflicting_script_versions() {
    let root = tempfile::tempdir().expect("failed to create temp dir");
    let primary = root.path().join("scripts/source");
    let alternate = root.path().join("source/scripts");
    fs::create_dir_all(&primary).expect("failed to create primary root");
    fs::create_dir_all(&alternate).expect("failed to create alternate root");
    let script = write_file(&primary, "Example.psc");
    fs::write(&script, "ScriptName Example\n").expect("failed to write primary script");
    fs::write(alternate.join("Example.psc"), "ScriptName ExampleV2\n")
        .expect("failed to write alternate script");
    let index = build_script_index(root.path(), &[]);

    assert_eq!(
        conflicting_script_versions_in_index(&script, &index, root.path(), false, GAME),
        conflicting_script_versions(&script, root.path(), &[], false, GAME)
    );
}

#[test]
fn conflicting_script_versions_in_index_ignores_unknown_file_names() {
    let root = tempfile::tempdir().expect("failed to create temp dir");
    let index = build_script_index(root.path(), &[]);
    let script = root.path().join("Untracked.psc");

    assert!(
        conflicting_script_versions_in_index(&script, &index, root.path(), false, GAME).is_empty()
    );
}

#[test]
fn conflicting_script_versions_among_flags_a_same_named_known_script_with_different_content() {
    let dir_a = tempfile::tempdir().expect("failed to create temp dir");
    let dir_b = tempfile::tempdir().expect("failed to create temp dir");
    let script = write_file(dir_a.path(), "Example.psc");
    fs::write(&script, "ScriptName Example\n").expect("failed to write first script");
    let other = write_file(dir_b.path(), "example.PSC");
    fs::write(&other, "ScriptName ExampleV2\n").expect("failed to write second script");

    let diagnostics = conflicting_script_versions_among(
        &script,
        std::slice::from_ref(&other),
        Path::new("."),
        false,
        GAME,
    );

    assert_eq!(diagnostics.len(), 1);
    assert!(diagnostics[0].message.contains("example.PSC"));
    assert!(diagnostics[0]
        .message
        .contains(&other.display().to_string()));
}

#[test]
fn conflicting_script_versions_among_shortens_a_conflict_under_the_project_root() {
    let root = tempfile::tempdir().expect("failed to create temp dir");
    let primary = root.path().join("scripts/source");
    let alternate = root.path().join("source/scripts");
    fs::create_dir_all(&primary).expect("failed to create primary root");
    fs::create_dir_all(&alternate).expect("failed to create alternate root");
    let script = write_file(&primary, "Example.psc");
    fs::write(&script, "ScriptName Example\n").expect("failed to write first script");
    let other = write_file(&alternate, "example.PSC");
    fs::write(&other, "ScriptName ExampleV2\n").expect("failed to write second script");

    let diagnostics = conflicting_script_versions_among(
        &script,
        std::slice::from_ref(&other),
        root.path(),
        true,
        GAME,
    );

    assert_eq!(diagnostics.len(), 1);
    let shortened = Path::new("source/scripts/example.PSC");
    assert!(diagnostics[0]
        .message
        .contains(&shortened.display().to_string()));
    assert!(!diagnostics[0]
        .message
        .contains(&root.path().display().to_string()));
}

#[test]
fn conflicting_script_versions_among_ignores_identical_known_scripts() {
    let dir_a = tempfile::tempdir().expect("failed to create temp dir");
    let dir_b = tempfile::tempdir().expect("failed to create temp dir");
    let script = write_file(dir_a.path(), "Example.psc");
    fs::write(&script, "same").expect("failed to write first script");
    let other = write_file(dir_b.path(), "Example.psc");
    fs::write(&other, "same").expect("failed to write second script");

    assert!(
        conflicting_script_versions_among(&script, &[other], Path::new("."), false, GAME)
            .is_empty()
    );
}

#[test]
fn conflicting_script_versions_among_ignores_the_script_itself_and_unrelated_names() {
    let dir = tempfile::tempdir().expect("failed to create temp dir");
    let script = write_file(dir.path(), "Example.psc");
    fs::write(&script, "content").expect("failed to write script");
    let unrelated = write_file(dir.path(), "Other.psc");
    fs::write(&unrelated, "different").expect("failed to write unrelated script");

    assert!(conflicting_script_versions_among(
        &script,
        &[script.clone(), unrelated],
        Path::new("."),
        false,
        GAME,
    )
    .is_empty());
}

#[test]
fn conflicting_script_versions_among_returns_empty_for_an_unreadable_script_path() {
    let root = tempfile::tempdir().expect("failed to create temp dir");

    assert!(conflicting_script_versions_among(
        &root.path().join("Missing.psc"),
        &[],
        Path::new("."),
        false,
        GAME,
    )
    .is_empty());
}

#[test]
fn conflicting_script_versions_among_ignores_unreadable_candidates() {
    let dir = tempfile::tempdir().expect("failed to create temp dir");
    let script = write_file(dir.path(), "Example.psc");
    fs::write(&script, "content").expect("failed to write script");
    let missing = dir.path().join("EXAMPLE.PSC");

    assert!(
        conflicting_script_versions_among(&script, &[missing], Path::new("."), false, GAME)
            .is_empty()
    );
}

#[test]
fn conflicting_script_versions_among_deduplicates_and_sorts_conflicts() {
    let primary = tempfile::tempdir().expect("failed to create primary dir");
    let alternatives = tempfile::tempdir().expect("failed to create alternatives dir");
    let script = write_file(primary.path(), "Example.psc");
    fs::write(&script, "primary").expect("failed to write primary script");
    let first = write_file(alternatives.path(), "EXAMPLE.PSC");
    fs::write(&first, "first").expect("failed to write first alternative");

    let second_dir = tempfile::tempdir().expect("failed to create second alternative dir");
    let second = write_file(second_dir.path(), "example.psc");
    fs::write(&second, "second").expect("failed to write second alternative");

    let diagnostics = conflicting_script_versions_among(
        &script,
        &[second.clone(), first.clone(), second],
        Path::new("."),
        false,
        GAME,
    );

    let mut expected = vec![first, second_dir.path().join("example.psc")];
    expected.sort();
    assert_eq!(diagnostics.len(), 2);
    for (diagnostic, path) in diagnostics.iter().zip(expected) {
        assert!(diagnostic.message.contains(&path.display().to_string()));
    }
}
