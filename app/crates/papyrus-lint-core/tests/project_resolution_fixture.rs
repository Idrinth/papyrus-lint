use std::fs;
use std::path::Path;

use papyrus_lint_core::achlist::parse_achlist;
use papyrus_lint_core::script_locator::{
    build_script_index, conflicting_script_versions, conflicting_script_versions_among,
    conflicting_script_versions_in_index, detected_script_roots, find_psc_file,
    find_psc_file_in_index, find_psc_files_recursively, CONFLICTING_SCRIPT_VERSIONS_RULE,
};
use papyrus_lint_core::source_encoding::{read_psc_source_with_encoding, PscEncoding};

fn write_file(path: &Path, contents: impl AsRef<[u8]>) {
    fs::create_dir_all(path.parent().expect("test file should have a parent"))
        .expect("failed to create test directory");
    fs::write(path, contents).expect("failed to write test file");
}

#[test]
fn achlist_paths_can_be_read_with_their_original_source_encoding() {
    let project = tempfile::tempdir().expect("failed to create project directory");
    let source_path = project.path().join("scripts/source/Encoded.psc");
    write_file(
        &source_path,
        b"ScriptName Encoded\r\n; Windows-1252 caf\xE9\r\n",
    );
    let achlist = project.path().join("project.achlist");
    write_file(&achlist, br#"["scripts/source/Encoded.psc"]"#);

    let paths = parse_achlist(&achlist).expect("achlist should parse");
    let (source, encoding) =
        read_psc_source_with_encoding(&paths[0]).expect("listed source should be readable");

    assert_eq!(paths, vec![source_path]);
    assert_eq!(encoding, PscEncoding::Windows1252);
    assert!(source.contains("café"));
}

#[test]
fn indexed_resolution_uses_search_root_precedence_and_reports_later_conflicts() {
    let project = tempfile::tempdir().expect("failed to create project directory");
    let preferred = project.path().join("scripts/source/Example.psc");
    let fallback = project.path().join("source/scripts/example.PSC");
    write_file(&preferred, "ScriptName Example\n");
    write_file(&fallback, "ScriptName Example extends Quest\n");

    let index = build_script_index(project.path(), &[]);
    let resolved = find_psc_file_in_index(&index, "EXAMPLE")
        .expect("case-insensitive indexed lookup should resolve");
    let diagnostics = conflicting_script_versions_in_index(&resolved, &index);

    assert_eq!(resolved, preferred);
    assert_eq!(diagnostics.len(), 1);
    assert_eq!(diagnostics[0].rule, CONFLICTING_SCRIPT_VERSIONS_RULE);
    assert!(diagnostics[0]
        .message
        .contains(&fallback.display().to_string()));
}

#[test]
fn direct_resolution_accepts_names_with_or_without_an_extension() {
    let project = tempfile::tempdir().expect("failed to create project directory");
    let source = project.path().join("source/scripts/MixedCase.PSC");
    write_file(&source, "ScriptName MixedCase\n");

    assert_eq!(
        find_psc_file(project.path(), "mixedcase", &[]),
        Some(source.clone())
    );
    assert_eq!(
        find_psc_file(project.path(), "MIXEDCASE.PSC", &[]),
        Some(source)
    );
    assert_eq!(find_psc_file(project.path(), "missing", &[]), None);
}

#[test]
fn direct_conflict_detection_scans_conventional_and_configured_roots() {
    let project = tempfile::tempdir().expect("failed to create project directory");
    let selected = project.path().join("scripts/source/Example.psc");
    let conventional_conflict = project.path().join("source/scripts/example.PSC");
    let configured_conflict = project.path().join("imports/EXAMPLE.psc");
    let identical_copy = project.path().join("identical/Example.psc");
    write_file(&selected, "ScriptName Example\n");
    write_file(&conventional_conflict, "ScriptName Example extends Form\n");
    write_file(&configured_conflict, "ScriptName Example extends Quest\n");
    write_file(&identical_copy, "ScriptName Example\n");

    let additional_roots = vec![
        "imports".to_string(),
        "identical".to_string(),
        "missing".to_string(),
    ];
    let diagnostics = conflicting_script_versions(&selected, project.path(), &additional_roots);
    let mut expected_conflicts = [conventional_conflict, configured_conflict];
    expected_conflicts.sort();

    assert_eq!(diagnostics.len(), 2);
    assert!(diagnostics[0]
        .message
        .contains(&expected_conflicts[0].display().to_string()));
    assert!(diagnostics[1]
        .message
        .contains(&expected_conflicts[1].display().to_string()));
    assert!(diagnostics
        .iter()
        .all(|diagnostic| diagnostic.rule == CONFLICTING_SCRIPT_VERSIONS_RULE));
    assert!(diagnostics
        .iter()
        .all(|diagnostic| !diagnostic.message.contains("identical")));
}

#[test]
fn recursive_discovery_returns_a_deterministic_achlist_ready_file_set() {
    let project = tempfile::tempdir().expect("failed to create project directory");
    let expected = vec![
        project.path().join("A/First.PSC"),
        project.path().join("B/nested/Second.psc"),
        project.path().join("Third.psc"),
    ];
    for path in &expected {
        write_file(path, "ScriptName Fixture\n");
    }
    write_file(&project.path().join("B/nested/compiled.pex"), b"not source");
    fs::create_dir_all(project.path().join("LooksLike.psc"))
        .expect("failed to create misleading directory");

    let discovered = find_psc_files_recursively(project.path());

    assert_eq!(discovered, expected);
    assert!(discovered.iter().all(|path| path.is_file()));
}

#[test]
fn additional_root_layout_pair_is_indexed_after_conventional_roots() {
    let project = tempfile::tempdir().expect("failed to create project directory");
    let conventional = project.path().join("scripts/source/Shared.psc");
    let configured = project.path().join("imports/source/scripts/shared.PSC");
    let paired = project.path().join("imports/scripts/source/Paired.psc");
    write_file(&conventional, "ScriptName Shared\n");
    write_file(&configured, "ScriptName Shared extends Quest\n");
    write_file(&paired, "ScriptName Paired\n");

    let additional_roots = vec!["imports/source/scripts".to_string()];
    let roots = detected_script_roots(project.path(), &additional_roots);
    let index = build_script_index(project.path(), &additional_roots);

    assert_eq!(
        roots,
        vec![
            project.path().join("scripts/source"),
            project.path().join("imports/source/scripts"),
            project.path().join("imports/scripts/source"),
        ]
    );
    assert_eq!(
        find_psc_file_in_index(&index, "shared.psc"),
        Some(conventional)
    );
    assert_eq!(find_psc_file_in_index(&index, "PAIRED"), Some(paired));
}

#[test]
fn identical_known_script_copies_do_not_report_a_conflict() {
    let project = tempfile::tempdir().expect("failed to create project directory");
    let first = project.path().join("one/Example.psc");
    let second = project.path().join("two/example.PSC");
    write_file(&first, "ScriptName Example\n");
    write_file(&second, "ScriptName Example\n");

    let diagnostics =
        conflicting_script_versions_among(&first, &[first.clone(), second.clone(), second]);

    assert!(diagnostics.is_empty());
}

#[test]
fn known_script_conflicts_are_sorted_and_deduplicated() {
    let project = tempfile::tempdir().expect("failed to create project directory");
    let selected = project.path().join("selected/Example.psc");
    let conflict_a = project.path().join("a/example.PSC");
    let conflict_b = project.path().join("b/EXAMPLE.psc");
    let unrelated = project.path().join("c/Other.psc");
    write_file(&selected, "ScriptName Example\n");
    write_file(&conflict_a, "ScriptName Example extends Form\n");
    write_file(&conflict_b, "ScriptName Example extends Quest\n");
    write_file(&unrelated, "ScriptName Other\n");

    let diagnostics = conflicting_script_versions_among(
        &selected,
        &[
            conflict_b.clone(),
            unrelated,
            conflict_a.clone(),
            conflict_b.clone(),
        ],
    );

    assert_eq!(diagnostics.len(), 2);
    assert!(diagnostics[0]
        .message
        .contains(&conflict_a.display().to_string()));
    assert!(diagnostics[1]
        .message
        .contains(&conflict_b.display().to_string()));
    assert!(diagnostics
        .iter()
        .all(|diagnostic| diagnostic.rule == CONFLICTING_SCRIPT_VERSIONS_RULE));
}
