use super::*;
use std::fs;
use std::path::PathBuf;

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

    let mut table = FunctionTable::new(root.path().to_path_buf())
        .with_known_scripts(&[invalid_path, valid_path]);

    assert!(table.script_exists("Example"));
}
