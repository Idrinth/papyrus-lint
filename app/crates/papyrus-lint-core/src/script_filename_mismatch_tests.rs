use super::*;

#[test]
fn flags_a_script_name_that_does_not_match_its_file_name() {
    let diagnostic = check(Path::new("Other.psc"), "ScriptName Example\n")
        .expect("mismatched script name should be flagged");

    assert_eq!(diagnostic.line, 1);
    assert_eq!(diagnostic.rule, RULE);
    assert!(diagnostic.message.starts_with("[error]"));
    assert!(diagnostic.message.contains("'Example'"));
    assert!(diagnostic.message.contains("'Other'"));
}

#[test]
fn does_not_flag_a_matching_script_name() {
    assert!(check(Path::new("Example.psc"), "ScriptName Example\n").is_none());
}

#[test]
fn matches_case_insensitively() {
    assert!(check(Path::new("example.psc"), "ScriptName EXAMPLE\n").is_none());
    assert!(check(Path::new("Example.psc"), "ScriptName example\n").is_none());
}

#[test]
fn ignores_a_script_with_no_scriptname_statement() {
    assert!(check(Path::new("Example.psc"), "; comment only\n").is_none());
}

#[test]
fn ignores_a_scriptname_without_a_declared_name() {
    assert!(check(Path::new("Example.psc"), "ScriptName").is_none());
}

#[test]
fn ignores_a_scriptname_followed_by_a_non_identifier() {
    assert!(check(Path::new("Example.psc"), "ScriptName 123\n").is_none());
}

#[test]
fn ignores_an_incomplete_namespace() {
    assert!(check(Path::new("Example.psc"), "ScriptName User:\n").is_none());
}

#[test]
fn ignores_a_namespace_segment_that_is_not_an_identifier() {
    assert!(check(Path::new("Example.psc"), "ScriptName User:123\n").is_none());
}

#[test]
fn ignores_a_script_that_fails_to_lex() {
    assert!(check(
        Path::new("Other.psc"),
        "ScriptName Example \"unterminated\n"
    )
    .is_none());
}

#[test]
fn ignores_a_path_with_no_file_stem() {
    assert!(check(Path::new("/"), "ScriptName Example\n").is_none());
}

#[test]
#[cfg(unix)]
fn ignores_a_path_with_a_non_utf8_file_stem() {
    use std::ffi::OsStr;
    use std::os::unix::ffi::OsStrExt;

    let path = Path::new(OsStr::from_bytes(b"Example\xFF.psc"));

    assert!(check(path, "ScriptName Example\n").is_none());
}

#[test]
fn compares_against_the_whole_stem_before_only_the_final_extension() {
    // `file_stem` only strips the last extension, so a file name with an
    // extra dot segment (e.g. a versioned file name) is compared against
    // its full stem, dot included — an identifier can't contain a dot,
    // so no `ScriptName` can ever match one anyway, but this documents
    // that `check` doesn't strip anything beyond the final extension.
    let diagnostic = check(Path::new("Quest.v2.psc"), "ScriptName Quest\n")
        .expect("mismatched script name should be flagged");

    assert!(diagnostic.message.contains("'Quest.v2'"));
}

#[test]
fn does_not_flag_a_namespaced_script_name_matching_only_its_final_segment() {
    assert!(check(
        Path::new("Scripts/Source/User/MyScript.psc"),
        "ScriptName User:MyScript\n"
    )
    .is_none());
}

#[test]
fn flags_a_namespaced_script_name_whose_final_segment_does_not_match() {
    let diagnostic = check(
        Path::new("Scripts/Source/User/Other.psc"),
        "ScriptName User:MyScript\n",
    )
    .expect("mismatched namespaced script name should be flagged");

    assert!(diagnostic.message.contains("'User:MyScript'"));
    assert!(diagnostic.message.contains("'Other'"));
}

#[test]
fn reports_the_scriptname_identifiers_own_position() {
    let diagnostic = check(Path::new("Other.psc"), "\n\nScriptName   Example\n")
        .expect("mismatched script name should be flagged");

    assert_eq!(diagnostic.line, 3);
    assert_eq!(diagnostic.column, 14);
}

#[test]
fn does_not_honor_a_disable_comment_itself_since_the_caller_filters_it_in() {
    // `check` no longer applies `; @disable`/`; @disable-file` itself —
    // see this module's own docs above. A caller merges its result into
    // `papyrus_lints::lint_with_external_arguments_and_extra_diagnostics`
    // instead, which filters it (and validates the directive as used)
    // the same way it does for every other diagnostic; that's covered by
    // integration tests in `papyrus-lint-cli` and `src-tauri`.
    assert!(check(
        Path::new("Other.psc"),
        "ScriptName Example ; @disable script-filename-mismatch\n"
    )
    .is_some());
    assert!(check(Path::new("Other.psc"), "ScriptName Example ; @disable\n").is_some());
    assert!(check(
        Path::new("Other.psc"),
        "ScriptName Example\n; @disable-file script-filename-mismatch\n"
    )
    .is_some());
}
