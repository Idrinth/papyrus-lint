use super::*;
use tempfile::tempdir;

use crate::lint::{lint_psc_file, ProjectLintContext};
use crate::repair::repair_psc_file;

#[test]
fn source_commands_parse_and_lint_without_touching_disk() {
    let source = "ScriptName Example\n\nFunction Run()\n    Game.GetPlayer()\nEndFunction\n";

    let script = parse_papyrus_script(source).expect("valid Papyrus should parse");
    assert_eq!(script.name, "Example");

    let diagnostics = lint_papyrus_script(source, papyrus_lints::Config::default());
    assert!(diagnostics
        .iter()
        .any(|diagnostic| { diagnostic.rule == "forbidden-functions" && diagnostic.line == 4 }));
}

#[test]
fn lint_papyrus_script_honors_disabled_rules() {
    let source = "ScriptName Example\n\nFunction Run()\n    Game.GetPlayer()\nEndFunction\n";
    let mut config = papyrus_lints::Config::default();
    config.rules.forbidden_functions = false;

    let diagnostics = lint_papyrus_script(source, config);

    assert!(diagnostics
        .iter()
        .all(|diagnostic| diagnostic.rule != "forbidden-functions"));
}

#[test]
fn psc_file_commands_round_trip_contents_and_parse_the_written_script() {
    let dir = tempdir().unwrap();
    let path = dir.path().join("Example.psc");
    let initial = "ScriptName Initial\n";
    let replacement = "ScriptName Replacement\n";
    std::fs::write(&path, initial).unwrap();

    assert_eq!(
        read_psc_file(path.to_string_lossy().into_owned()).unwrap(),
        initial
    );
    write_psc_file(path.to_string_lossy().into_owned(), replacement.to_string()).unwrap();

    let script = parse_psc_file(
        path.to_string_lossy().into_owned(),
        papyrus_lints::Game::default(),
    )
    .unwrap();
    assert_eq!(script.name, "Replacement");
    assert_eq!(std::fs::read_to_string(path).unwrap(), replacement);
}

#[test]
fn write_psc_file_truncates_longer_existing_contents() {
    let dir = tempdir().unwrap();
    let path = dir.path().join("Example.psc");
    std::fs::write(&path, "ScriptName MuchLongerOriginalName\n").unwrap();

    write_psc_file(
        path.to_string_lossy().into_owned(),
        "ScriptName Short\n".to_string(),
    )
    .unwrap();

    assert_eq!(std::fs::read_to_string(path).unwrap(), "ScriptName Short\n");
}

#[test]
fn write_psc_file_creates_a_new_file() {
    let dir = tempdir().unwrap();
    let path = dir.path().join("New.psc");

    write_psc_file(
        path.to_string_lossy().into_owned(),
        "ScriptName New\n".to_string(),
    )
    .unwrap();

    assert_eq!(std::fs::read_to_string(path).unwrap(), "ScriptName New\n");
}

#[test]
fn hash_psc_file_md5_reports_the_md5_digest_of_the_files_current_contents() {
    let dir = tempdir().unwrap();
    let path = dir.path().join("Example.psc");
    let source = "ScriptName Example\n";
    std::fs::write(&path, source).unwrap();
    let path_string = path.to_string_lossy().into_owned();

    assert_eq!(
        hash_psc_file_md5(path_string.clone()).unwrap(),
        content_hash::md5_hex(source)
    );

    std::fs::write(&path, "ScriptName Changed\n").unwrap();
    assert_ne!(
        hash_psc_file_md5(path_string).unwrap(),
        content_hash::md5_hex(source)
    );
}

#[test]
fn hash_psc_file_md5_hashes_the_decoded_cp1252_source() {
    let dir = tempdir().unwrap();
    let path = dir.path().join("Example.psc");
    let mut bytes = b"ScriptName Example\n; caf".to_vec();
    bytes.extend_from_slice(&[0xE9, b'\n']);
    std::fs::write(&path, bytes).unwrap();

    assert_eq!(
        hash_psc_file_md5(path.to_string_lossy().into_owned()).unwrap(),
        content_hash::md5_hex("ScriptName Example\n; café\n")
    );
}

#[test]
fn psc_file_commands_decode_cp1252_source_for_the_viewer_and_parser() {
    let dir = tempdir().unwrap();
    let path = dir.path().join("Example.psc");
    let mut source = b"ScriptName Example\n\n; caf".to_vec();
    source.extend_from_slice(&[0xE9, b'\n']);
    std::fs::write(&path, source).unwrap();
    let path_string = path.to_string_lossy().into_owned();

    assert_eq!(
        read_psc_file(path_string.clone()).unwrap(),
        "ScriptName Example\n\n; café\n"
    );
    assert_eq!(
        parse_psc_file(path_string, papyrus_lints::Game::default())
            .unwrap()
            .name,
        "Example"
    );
}

#[test]
fn parse_psc_file_reflects_edits_made_between_calls_instead_of_a_stale_cache_entry() {
    let dir = tempdir().unwrap();
    let path = dir.path().join("Example.psc");
    let path_string = path.to_string_lossy().into_owned();

    std::fs::write(&path, "ScriptName Initial\n").unwrap();
    let first = parse_psc_file(path_string.clone(), papyrus_lints::Game::default()).unwrap();
    assert_eq!(first.name, "Initial");

    std::fs::write(&path, "ScriptName Changed\n").unwrap();
    let second = parse_psc_file(path_string, papyrus_lints::Game::default()).unwrap();
    assert_eq!(second.name, "Changed");
}

#[test]
fn parse_psc_file_does_not_return_a_cached_script_after_an_invalid_edit() {
    let dir = tempdir().unwrap();
    let path = dir.path().join("Example.psc");
    let path_string = path.to_string_lossy().into_owned();

    std::fs::write(&path, "ScriptName Example\n").unwrap();
    assert_eq!(
        parse_psc_file(path_string.clone(), papyrus_lints::Game::default())
            .unwrap()
            .name,
        "Example"
    );

    std::fs::write(&path, "Function MissingScriptName()\nEndFunction\n").unwrap();

    assert!(parse_psc_file(path_string, papyrus_lints::Game::default()).is_err());
}

#[test]
fn parse_psc_file_reuses_a_valid_cached_script() {
    let dir = tempdir().unwrap();
    let path = dir.path().join("Cached.psc");
    let path_string = path.to_string_lossy().into_owned();
    std::fs::write(&path, "ScriptName Cached\n").unwrap();

    assert_eq!(
        parse_psc_file(path_string.clone(), papyrus_lints::Game::default())
            .unwrap()
            .name,
        "Cached"
    );
    assert_eq!(
        parse_psc_file(path_string, papyrus_lints::Game::default())
            .unwrap()
            .name,
        "Cached"
    );
}

#[test]
fn file_commands_report_io_errors_instead_of_panicking() {
    let missing = tempdir().unwrap().path().join("missing.psc");
    let path = missing.to_string_lossy().into_owned();

    assert!(read_psc_file(path.clone()).is_err());
    assert!(hash_psc_file_md5(path.clone()).is_err());
    assert!(write_psc_file(path.clone(), "ScriptName Example\n".to_string()).is_err());
    assert!(parse_psc_file(path.clone(), papyrus_lints::Game::default()).is_err());
    assert!(lint_psc_file(
        path.clone(),
        ProjectLintContext {
            root: missing.parent().unwrap().to_string_lossy().into_owned(),
            ..Default::default()
        }
    )
    .is_err());
    assert!(repair_psc_file(
        path,
        ProjectLintContext {
            root: missing.parent().unwrap().to_string_lossy().into_owned(),
            ..Default::default()
        }
    )
    .is_err());
}

#[test]
fn achlist_command_resolves_entries_and_reports_invalid_json() {
    let dir = tempdir().unwrap();
    let path = dir.path().join("scripts.achlist");
    std::fs::write(&path, r#"["scripts/source/Example.psc"]"#).unwrap();

    assert_eq!(
        parse_achlist_file(path.to_string_lossy().into_owned()).unwrap(),
        vec![dir
            .path()
            .join("scripts/source/Example.psc")
            .to_string_lossy()
            .into_owned()]
    );

    std::fs::write(&path, "not json").unwrap();
    assert!(parse_achlist_file(path.to_string_lossy().into_owned()).is_err());

    let missing = dir.path().join("missing.achlist");
    assert!(parse_achlist_file(missing.to_string_lossy().into_owned()).is_err());
}

#[test]
fn ppj_command_resolves_scripts_and_imports_and_reports_invalid_xml() {
    let dir = tempdir().unwrap();
    std::fs::create_dir_all(dir.path().join("Source/Scripts")).unwrap();
    std::fs::write(
        dir.path().join("Source/Scripts/Example.psc"),
        "ScriptName Example\n",
    )
    .unwrap();
    let path = dir.path().join("project.ppj");
    std::fs::write(
        &path,
        r#"<PapyrusProject xmlns="PapyrusProject.xsd">
            <Imports>
                <Import>.\Source\Scripts</Import>
            </Imports>
            <Folders>
                <Folder>.\Source\Scripts</Folder>
            </Folders>
        </PapyrusProject>"#,
    )
    .unwrap();

    let result = parse_ppj_file(path.to_string_lossy().into_owned()).unwrap();
    assert_eq!(
        result.scripts,
        vec![dir
            .path()
            .join("./Source/Scripts/Example.psc")
            .to_string_lossy()
            .into_owned()]
    );
    assert_eq!(
        result.imports,
        vec![dir
            .path()
            .join("./Source/Scripts")
            .to_string_lossy()
            .into_owned()]
    );

    std::fs::write(&path, "not xml").unwrap();
    assert!(parse_ppj_file(path.to_string_lossy().into_owned()).is_err());

    let missing = dir.path().join("missing.ppj");
    assert!(parse_ppj_file(missing.to_string_lossy().into_owned()).is_err());
}

#[test]
fn achlist_command_reports_a_missing_file() {
    let dir = tempdir().unwrap();
    let missing = dir.path().join("missing.achlist");

    let error = parse_achlist_file(missing.to_string_lossy().into_owned())
        .expect_err("a missing achlist should be reported");

    assert!(
        error.contains("failed to read achlist file"),
        "unexpected error: {error}"
    );
}

#[test]
fn list_psc_files_recursively_finds_scripts_at_every_nesting_depth() {
    let dir = tempdir().unwrap();
    std::fs::create_dir_all(dir.path().join("Requiem/Sub")).unwrap();
    std::fs::write(dir.path().join("Top.psc"), "ScriptName Top\n").unwrap();
    std::fs::write(
        dir.path().join("Requiem/Sub/Nested.psc"),
        "ScriptName Nested\n",
    )
    .unwrap();

    let mut result = list_psc_files_recursively(dir.path().to_string_lossy().into_owned())
        .expect("directory should scan successfully");
    result.sort();
    let mut expected = vec![
        dir.path()
            .join("Requiem/Sub/Nested.psc")
            .to_string_lossy()
            .into_owned(),
        dir.path().join("Top.psc").to_string_lossy().into_owned(),
    ];
    expected.sort();

    assert_eq!(result, expected);
}

#[test]
fn list_psc_files_recursively_rejects_a_non_directory_path() {
    let dir = tempdir().unwrap();
    let file = dir.path().join("Example.psc");
    std::fs::write(&file, "ScriptName Example\n").unwrap();

    assert!(list_psc_files_recursively(file.to_string_lossy().into_owned()).is_err());
    assert!(
        list_psc_files_recursively(dir.path().join("missing").to_string_lossy().into_owned())
            .is_err()
    );
}

#[test]
fn list_psc_files_recursively_accepts_an_empty_directory() {
    let dir = tempdir().unwrap();

    let files = list_psc_files_recursively(dir.path().to_string_lossy().into_owned()).unwrap();

    assert!(files.is_empty());
}

#[test]
fn list_psc_files_recursively_ignores_similarly_named_non_psc_files() {
    let dir = tempdir().unwrap();
    let script = dir.path().join("Actual.psc");
    std::fs::write(&script, "ScriptName Actual\n").unwrap();
    std::fs::write(dir.path().join("Backup.psc.bak"), "ScriptName Backup\n").unwrap();
    std::fs::write(dir.path().join("Notes.txt"), "ScriptName Notes\n").unwrap();

    let files = list_psc_files_recursively(dir.path().to_string_lossy().into_owned()).unwrap();

    assert_eq!(files, vec![script.to_string_lossy().into_owned()]);
}

#[test]
fn list_psc_files_recursively_matches_uppercase_extensions() {
    let dir = tempdir().unwrap();
    let script = dir.path().join("Uppercase.PSC");
    std::fs::write(&script, "ScriptName Uppercase\n").unwrap();

    let files = list_psc_files_recursively(dir.path().to_string_lossy().into_owned()).unwrap();

    assert_eq!(files, vec![script.to_string_lossy().into_owned()]);
}

#[test]
fn parse_commands_report_invalid_papyrus() {
    let invalid = "Function MissingScriptName()\nEndFunction\n";
    assert!(parse_papyrus_script(invalid).is_err());

    let dir = tempdir().unwrap();
    let path = dir.path().join("Invalid.psc");
    std::fs::write(&path, invalid).unwrap();
    assert!(parse_psc_file(
        path.to_string_lossy().into_owned(),
        papyrus_lints::Game::default()
    )
    .is_err());
}

#[test]
fn get_psc_file_mtimes_reports_each_existing_paths_modified_time() {
    let dir = tempdir().unwrap();
    let path = dir.path().join("Example.psc");
    std::fs::write(&path, "ScriptName Example\n").unwrap();
    let path_string = path.to_string_lossy().into_owned();

    let mtimes = get_psc_file_mtimes(vec![path_string.clone()]);

    let expected = std::fs::metadata(&path)
        .unwrap()
        .modified()
        .unwrap()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_millis() as u64;
    assert_eq!(mtimes.get(&path_string), Some(&expected));
}

#[test]
fn get_psc_file_mtimes_omits_paths_that_do_not_exist() {
    let dir = tempdir().unwrap();
    let missing = dir
        .path()
        .join("missing.psc")
        .to_string_lossy()
        .into_owned();

    let mtimes = get_psc_file_mtimes(vec![missing.clone()]);

    assert!(!mtimes.contains_key(&missing));
}

#[test]
fn get_psc_file_mtimes_only_reports_the_paths_asked_for() {
    let dir = tempdir().unwrap();
    let watched = dir.path().join("Watched.psc");
    let other = dir.path().join("Other.psc");
    std::fs::write(&watched, "ScriptName Watched\n").unwrap();
    std::fs::write(&other, "ScriptName Other\n").unwrap();
    let watched_string = watched.to_string_lossy().into_owned();

    let mtimes = get_psc_file_mtimes(vec![watched_string.clone()]);

    assert_eq!(mtimes.len(), 1);
    assert!(mtimes.contains_key(&watched_string));
}

#[test]
fn achlist_command_returns_an_empty_list_for_an_empty_array() {
    let dir = tempdir().unwrap();
    let path = dir.path().join("scripts.achlist");
    std::fs::write(&path, "[]").unwrap();

    assert_eq!(
        parse_achlist_file(path.to_string_lossy().into_owned()).unwrap(),
        Vec::<String>::new()
    );
}
