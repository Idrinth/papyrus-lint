use std::fs;
use std::path::Path;

use papyrus_lint_core::achlist::parse_achlist;
use papyrus_lint_core::script_locator::{
    build_script_index, conflicting_script_versions_in_index, find_psc_file_in_index,
    find_psc_files_recursively, CONFLICTING_SCRIPT_VERSIONS_RULE,
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
