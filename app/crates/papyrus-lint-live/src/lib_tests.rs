use super::*;
use std::fs;

#[test]
fn lint_source_reports_engine_diagnostics_on_a_buffer() {
    let analysis = lint_source("ScriptName Example   \n", &papyrus_lints::Config::default());
    assert!(analysis
        .diagnostics
        .iter()
        .any(|diagnostic| diagnostic.rule == "trailing-whitespace"));
    assert!(!analysis.parse_failed());
}

#[test]
fn lint_source_records_a_parse_failure_separately() {
    let analysis = lint_source(
        "ScriptName Example\nFunction Broken(\n",
        &papyrus_lints::Config::default(),
    );
    let failure = analysis.parser_failure.expect("parse failure");
    assert_eq!(failure.kind, ParserFailureKind::Parse);
    assert!(failure.line >= 1);
    assert!(failure.column >= 1);
    assert!(!failure.message.is_empty());
}

#[test]
fn lint_source_records_lexer_failure_details() {
    let analysis = lint_source("ScriptName Example\n@", &papyrus_lints::Config::default());

    assert!(analysis.parse_failed());
    assert_eq!(
        analysis.parser_failure,
        Some(ParserFailure {
            kind: ParserFailureKind::Lex,
            line: 2,
            column: 1,
            message: "unexpected character '@'".to_string(),
        })
    );
}

#[test]
fn lint_source_respects_disabled_rules() {
    let mut config = papyrus_lints::Config::default();
    config.rules.trailing_whitespace = false;

    let analysis = lint_source("ScriptName Example   \n", &config);

    assert!(!analysis
        .diagnostics
        .iter()
        .any(|diagnostic| diagnostic.rule == "trailing-whitespace"));
    assert!(!analysis.parse_failed());
}

#[test]
fn config_from_override_uses_the_engine_default_without_a_path() {
    let config = config_from_override(None).expect("default config");
    assert_eq!(config, papyrus_lints::Config::default());
}

#[test]
fn config_from_override_loads_an_explicit_file() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("custom.yaml");
    fs::write(&path, "rules:\n  trailing_whitespace: false\n").unwrap();

    let config = config_from_override(Some(&path)).expect("loaded config");
    assert!(!config.rules.trailing_whitespace);
}

#[test]
fn config_from_override_reports_a_missing_file() {
    let err = config_from_override(Some(std::path::Path::new("/no/such/papyrus-lint.yaml")))
        .expect_err("missing file");
    assert!(!err.is_empty());
}

#[test]
fn config_from_override_reports_an_invalid_file() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("invalid.yaml");
    fs::write(&path, "rules: [not a rules mapping]\n").unwrap();

    let err = config_from_override(Some(&path)).expect_err("invalid config");
    assert!(!err.is_empty());
}

#[test]
fn config_from_script_path_walks_parents_for_a_project_file() {
    let root = tempfile::tempdir().unwrap();
    fs::write(
        root.path().join("papyrus-lint.yaml"),
        "rules:\n  trailing_whitespace: false\n",
    )
    .unwrap();
    let nested = root.path().join("scripts").join("source");
    fs::create_dir_all(&nested).unwrap();
    let script = nested.join("Example.psc");
    fs::write(&script, "ScriptName Example   \n").unwrap();

    let config = config_from_script_path(&script);
    assert!(!config.rules.trailing_whitespace);
}

#[test]
fn config_from_script_path_falls_back_to_default_without_a_project_file() {
    let dir = tempfile::tempdir().unwrap();
    let script = dir.path().join("Example.psc");
    fs::write(&script, "ScriptName Example\n").unwrap();
    assert_eq!(
        config_from_script_path(&script),
        papyrus_lints::Config::default()
    );
}

#[test]
fn config_from_script_path_recognizes_the_yml_extension() {
    let root = tempfile::tempdir().unwrap();
    fs::write(
        root.path().join("papyrus-lint.yml"),
        "rules:\n  trailing_whitespace: false\n",
    )
    .unwrap();
    let script = root.path().join("Example.psc");

    let config = config_from_script_path(&script);

    assert!(!config.rules.trailing_whitespace);
}

#[test]
fn config_from_script_path_prefers_yaml_when_both_extensions_exist() {
    let root = tempfile::tempdir().unwrap();
    fs::write(
        root.path().join("papyrus-lint.yaml"),
        "rules:\n  trailing_whitespace: false\n",
    )
    .unwrap();
    fs::write(
        root.path().join("papyrus-lint.yml"),
        "rules:\n  trailing_whitespace: true\n",
    )
    .unwrap();
    let script = root.path().join("Example.psc");

    let config = config_from_script_path(&script);

    assert!(!config.rules.trailing_whitespace);
}

#[test]
fn config_from_script_path_falls_back_to_default_for_an_invalid_project_file() {
    let root = tempfile::tempdir().unwrap();
    fs::write(
        root.path().join("papyrus-lint.yaml"),
        "rules: [not a rules mapping]\n",
    )
    .unwrap();
    let script = root.path().join("Example.psc");

    assert_eq!(
        config_from_script_path(&script),
        papyrus_lints::Config::default()
    );
}
