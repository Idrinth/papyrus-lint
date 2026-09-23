use super::*;

fn write_achlist(dir: &Path, name: &str, contents: &str) -> PathBuf {
    let path = dir.join(name);
    fs::write(&path, contents).expect("failed to write test achlist file");
    path
}

#[test]
fn parses_relative_paths_against_achlist_directory() {
    let dir = tempfile::tempdir().expect("failed to create temp dir");
    let achlist_path = write_achlist(
        dir.path(),
        "sources.achlist",
        r#"["scripts/Foo.psc", "../shared/Bar.psc"]"#,
    );

    let result = parse_achlist(&achlist_path).expect("parsing should succeed");

    assert_eq!(
        result,
        vec![
            dir.path().join("scripts/Foo.psc"),
            dir.path().join("../shared/Bar.psc"),
        ]
    );
}

#[test]
fn strips_redundant_data_prefix_when_achlist_lives_in_data_dir() {
    let root = tempfile::tempdir().expect("failed to create temp dir");
    let data_dir = root.path().join("Data");
    fs::create_dir_all(&data_dir).expect("failed to create Data dir");
    let achlist_path = write_achlist(
        &data_dir,
        "sources.achlist",
        r#"["Data\\SCRIPTS\\SOURCE\\Foo.psc"]"#,
    );

    let result = parse_achlist(&achlist_path).expect("parsing should succeed");

    assert_eq!(result, vec![data_dir.join("SCRIPTS\\SOURCE\\Foo.psc")]);
}

#[test]
fn strips_redundant_data_prefix_case_insensitively() {
    let root = tempfile::tempdir().expect("failed to create temp dir");
    let data_dir = root.path().join("data");
    fs::create_dir_all(&data_dir).expect("failed to create data dir");
    let achlist_path = write_achlist(&data_dir, "sources.achlist", r#"["DATA/Foo.psc"]"#);

    let result = parse_achlist(&achlist_path).expect("parsing should succeed");

    assert_eq!(result, vec![data_dir.join("Foo.psc")]);
}

#[test]
fn strips_redundant_data_prefix_with_either_separator() {
    let root = tempfile::tempdir().expect("failed to create temp dir");
    let data_dir = root.path().join("Data");
    fs::create_dir_all(&data_dir).expect("failed to create Data dir");
    let achlist_path = write_achlist(
        &data_dir,
        "sources.achlist",
        r#"["Data/Scripts/Foo.psc", "Data\\Scripts\\Bar.psc"]"#,
    );

    let result = parse_achlist(&achlist_path).expect("parsing should succeed");

    assert_eq!(
        result,
        vec![
            data_dir.join("Scripts/Foo.psc"),
            data_dir.join("Scripts\\Bar.psc"),
        ]
    );
}

#[test]
fn strips_only_the_first_redundant_data_component() {
    let root = tempfile::tempdir().expect("failed to create temp dir");
    let data_dir = root.path().join("Data");
    fs::create_dir_all(&data_dir).expect("failed to create Data dir");
    let achlist_path = write_achlist(&data_dir, "sources.achlist", r#"["Data/Data/Foo.psc"]"#);

    let result = parse_achlist(&achlist_path).expect("parsing should succeed");

    assert_eq!(result, vec![data_dir.join("Data/Foo.psc")]);
}

#[test]
fn leaves_entry_unchanged_when_achlist_directory_is_not_named_data() {
    let dir = tempfile::tempdir().expect("failed to create temp dir");
    let achlist_path = write_achlist(dir.path(), "sources.achlist", r#"["Data/Foo.psc"]"#);

    let result = parse_achlist(&achlist_path).expect("parsing should succeed");

    assert_eq!(result, vec![dir.path().join("Data/Foo.psc")]);
}

#[test]
fn leaves_entry_unchanged_when_it_has_no_data_prefix() {
    let root = tempfile::tempdir().expect("failed to create temp dir");
    let data_dir = root.path().join("Data");
    fs::create_dir_all(&data_dir).expect("failed to create Data dir");
    let achlist_path = write_achlist(
        &data_dir,
        "sources.achlist",
        r#"["SCRIPTS\\SOURCE\\Foo.psc"]"#,
    );

    let result = parse_achlist(&achlist_path).expect("parsing should succeed");

    assert_eq!(result, vec![data_dir.join("SCRIPTS\\SOURCE\\Foo.psc")]);
}

#[test]
fn parses_empty_list() {
    let dir = tempfile::tempdir().expect("failed to create temp dir");
    let achlist_path = write_achlist(dir.path(), "empty.achlist", "[]");

    let result = parse_achlist(&achlist_path).expect("parsing should succeed");

    assert!(result.is_empty());
}

#[test]
fn preserves_entry_order_and_duplicates() {
    let dir = tempfile::tempdir().expect("failed to create temp dir");
    let achlist_path = write_achlist(
        dir.path(),
        "sources.achlist",
        r#"["B.psc", "A.psc", "B.psc"]"#,
    );

    let result = parse_achlist(&achlist_path).expect("parsing should succeed");

    assert_eq!(
        result,
        vec![
            dir.path().join("B.psc"),
            dir.path().join("A.psc"),
            dir.path().join("B.psc"),
        ]
    );
}

#[test]
fn parses_unicode_and_whitespace_in_entries() {
    let dir = tempfile::tempdir().expect("failed to create temp dir");
    let achlist_path = write_achlist(
        dir.path(),
        "sources.achlist",
        r#"["Scripts/Crème brûlée.psc", " spaced name.psc "]"#,
    );

    let result = parse_achlist(&achlist_path).expect("parsing should succeed");

    assert_eq!(
        result,
        vec![
            dir.path().join("Scripts/Crème brûlée.psc"),
            dir.path().join(" spaced name.psc "),
        ]
    );
}

#[test]
fn preserves_absolute_entries() {
    let dir = tempfile::tempdir().expect("failed to create temp dir");
    let absolute = dir.path().join("elsewhere/Foo.psc");
    let contents = serde_json::to_string(&vec![absolute.to_string_lossy()])
        .expect("failed to serialize achlist");
    let achlist_path = write_achlist(dir.path(), "sources.achlist", &contents);

    let result = parse_achlist(&achlist_path).expect("parsing should succeed");

    assert_eq!(result, vec![absolute]);
}

#[test]
fn does_not_strip_data_without_a_following_separator() {
    let root = tempfile::tempdir().expect("failed to create temp dir");
    let data_dir = root.path().join("Data");
    fs::create_dir_all(&data_dir).expect("failed to create Data dir");
    let achlist_path = write_achlist(&data_dir, "sources.achlist", r#"["Data"]"#);

    let result = parse_achlist(&achlist_path).expect("parsing should succeed");

    assert_eq!(result, vec![data_dir.join("Data")]);
}

#[test]
fn errors_on_missing_file() {
    let missing_path = PathBuf::from("/nonexistent/path/does-not-exist.achlist");

    let result = parse_achlist(&missing_path);

    assert!(matches!(result, Err(AchlistError::Io(_))));
}

#[test]
fn errors_on_invalid_json() {
    let dir = tempfile::tempdir().expect("failed to create temp dir");
    let achlist_path = write_achlist(dir.path(), "broken.achlist", "not valid json");

    let result = parse_achlist(&achlist_path);

    assert!(matches!(result, Err(AchlistError::Json(_))));
}

#[test]
fn errors_on_non_array_json() {
    let dir = tempfile::tempdir().expect("failed to create temp dir");
    let achlist_path = write_achlist(dir.path(), "object.achlist", r#"{"not": "a list"}"#);

    let result = parse_achlist(&achlist_path);

    assert!(matches!(result, Err(AchlistError::Json(_))));
}

#[test]
fn errors_when_an_array_entry_is_not_a_string() {
    let dir = tempfile::tempdir().expect("failed to create temp dir");
    let achlist_path = write_achlist(dir.path(), "wrong-entry-type.achlist", r#"["Foo.psc", 42]"#);

    let result = parse_achlist(&achlist_path);

    assert!(matches!(result, Err(AchlistError::Json(_))));
}

#[test]
fn io_error_display_includes_context_and_underlying_error() {
    let error = AchlistError::from(std::io::Error::new(
        std::io::ErrorKind::PermissionDenied,
        "access denied",
    ));

    assert_eq!(
        error.to_string(),
        "failed to read achlist file: access denied"
    );
    assert!(matches!(
        error,
        AchlistError::Io(inner) if inner.kind() == std::io::ErrorKind::PermissionDenied
    ));
}

#[test]
fn json_error_display_includes_context_and_underlying_error() {
    let serde_error = serde_json::from_str::<Vec<String>>("{").unwrap_err();
    let error = AchlistError::from(serde_error);

    assert!(error
        .to_string()
        .starts_with("failed to parse achlist file:"));
    assert!(matches!(error, AchlistError::Json(_)));
}
