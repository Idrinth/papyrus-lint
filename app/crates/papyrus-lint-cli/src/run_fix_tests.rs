use super::*;

#[test]
fn fix_file_reports_a_write_error_without_claiming_the_file_was_fixed() {
    let dir = tempfile::tempdir().expect("failed to create temp dir");
    let function_table = RwLock::new(FunctionTable::new(dir.path().to_path_buf()));

    let error = fix_file(
        dir.path(),
        "Example.psc",
        "ScriptName Example   \n".to_string(),
        PscEncoding::Utf8,
        &papyrus_lints::Config::default(),
        &function_table,
        None,
        None,
        None,
        false,
        false,
    )
    .err()
    .expect("writing repaired source to a directory should fail");

    assert!(error.starts_with("error: failed to write "));
    assert!(error.contains(&dir.path().display().to_string()));
}
