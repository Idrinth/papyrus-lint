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
