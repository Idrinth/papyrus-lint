use std::path::{Path, PathBuf};

use super::{check, ProjectFile, RULE};

fn file(path: &str, content_hash: &str) -> ProjectFile {
    ProjectFile {
        path: PathBuf::from(path),
        display_path: path.to_string(),
        content_hash: content_hash.to_string(),
    }
}

#[test]
fn reports_different_same_named_files_from_complete_list() {
    let files = vec![
        file("first/Test.psc", "aaa"),
        file("unrelated/Other.psc", "ccc"),
        file("second/test.PSC", "bbb"),
    ];
    let diagnostics = check(Path::new("first/Test.psc"), "aaa", &files);

    assert_eq!(diagnostics.len(), 1);
    assert_eq!(diagnostics[0].rule, RULE);
    assert!(diagnostics[0].message.contains("second/test.PSC"));
}

#[test]
fn ignores_identical_copies() {
    let files = vec![
        file("first/Test.psc", "same"),
        file("second/Test.psc", "same"),
    ];
    assert!(check(Path::new("first/Test.psc"), "same", &files).is_empty());
}

#[test]
fn ignores_the_current_file_and_differently_named_scripts() {
    let files = vec![
        file("first/Test.psc", "changed"),
        file("second/Other.psc", "changed"),
    ];

    assert!(check(Path::new("first/Test.psc"), "original", &files).is_empty());
}

#[test]
fn sorts_conflicts_by_path_and_removes_duplicate_entries() {
    let files = vec![
        file("z/Test.psc", "z"),
        file("a/Test.psc", "a"),
        file("z/Test.psc", "z"),
    ];
    let diagnostics = check(Path::new("current/Test.psc"), "current", &files);

    assert_eq!(diagnostics.len(), 2);
    assert!(diagnostics[0].message.contains("a/Test.psc"));
    assert!(diagnostics[1].message.contains("z/Test.psc"));
    assert!(diagnostics
        .iter()
        .all(|diagnostic| diagnostic.line == 1 && diagnostic.column == 1));
}

#[test]
fn uses_the_display_path_in_the_diagnostic() {
    let files = [ProjectFile {
        path: PathBuf::from("internal/Test.psc"),
        display_path: "Data/Scripts/Source/Test.psc".to_string(),
        content_hash: "different".to_string(),
    }];
    let diagnostics = check(Path::new("current/Test.psc"), "current", &files);

    assert_eq!(diagnostics.len(), 1);
    assert!(diagnostics[0]
        .message
        .contains("Data/Scripts/Source/Test.psc"));
    assert!(!diagnostics[0].message.contains("internal/Test.psc"));
}

#[cfg(unix)]
#[test]
fn ignores_a_current_path_without_a_utf8_file_name() {
    use std::ffi::OsStr;
    use std::os::unix::ffi::OsStrExt;

    let path = Path::new(OsStr::from_bytes(b"invalid-\xff.psc"));

    assert!(check(path, "current", &[file("Test.psc", "different")]).is_empty());
}
