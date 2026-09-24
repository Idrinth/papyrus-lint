use std::path::{Path, PathBuf};

use super::{check, ProjectFile, RULE};

fn file(path: &str, contents: &str) -> ProjectFile {
    ProjectFile {
        path: PathBuf::from(path),
        display_path: path.to_string(),
        contents: contents.as_bytes().to_vec(),
    }
}

#[test]
fn reports_different_same_named_files_from_complete_list() {
    let files = vec![
        file("first/Test.psc", "one"),
        file("unrelated/Other.psc", "other"),
        file("second/test.PSC", "two"),
    ];
    let diagnostics = check(Path::new("first/Test.psc"), b"one", &files);

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
    assert!(check(Path::new("first/Test.psc"), b"same", &files).is_empty());
}
