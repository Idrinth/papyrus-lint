use super::*;

#[test]
fn script_imports_matches_names_case_insensitively() {
    let script = papyrus_parser::parse("ScriptName Example\nImport Utility\n").unwrap();

    assert!(script_imports(Some(&script), "utility"));
    assert!(!script_imports(Some(&script), "Game"));
}

#[test]
fn script_imports_handles_a_missing_ast() {
    assert!(!script_imports(None, "Utility"));
}

#[test]
fn tokens_import_matches_names_case_insensitively() {
    let tokens = papyrus_parser::tokenize("ScriptName Example\nImport Utility\n").unwrap();

    assert!(tokens_import(&tokens, "UTILITY"));
    assert!(!tokens_import(&tokens, "Game"));
}

#[test]
fn tokens_import_requires_an_identifier_immediately_after_import() {
    let tokens = papyrus_parser::tokenize("ScriptName Example\nImport\nUtility.Wait(1.0)\n").unwrap();

    assert!(!tokens_import(&tokens, "Utility"));
    assert!(!tokens_import(&[], "Utility"));
}
