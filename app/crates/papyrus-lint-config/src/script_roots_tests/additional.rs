use super::*;

#[test]
fn load_script_roots_returns_empty_when_unset() {
    let dir = tempfile::tempdir().expect("failed to create temp dir");

    assert_eq!(
        load_script_roots(dir.path()).expect("should succeed"),
        Vec::<String>::new()
    );
}

#[test]
fn save_and_load_script_roots_round_trips_without_disturbing_lint_settings() {
    let dir = tempfile::tempdir().expect("failed to create temp dir");
    let config = papyrus_lints::Config {
        semicolon: true,
        ..papyrus_lints::Config::default()
    };
    save_config(dir.path(), &config).expect("saving lint config should succeed");

    save_script_roots(
        dir.path(),
        &[
            "../SharedScripts".to_string(),
            "/abs/OtherScripts".to_string(),
        ],
    )
    .expect("saving script roots should succeed");

    assert_eq!(
        load_script_roots(dir.path()).expect("should succeed"),
        vec![
            "../SharedScripts".to_string(),
            "/abs/OtherScripts".to_string()
        ]
    );
    assert_eq!(load_config(dir.path()).expect("should succeed"), config);
}

#[test]
fn save_script_roots_drops_blank_entries() {
    let dir = tempfile::tempdir().expect("failed to create temp dir");

    save_script_roots(
        dir.path(),
        &[
            "  ".to_string(),
            "../SharedScripts".to_string(),
            String::new(),
        ],
    )
    .expect("saving script roots should succeed");

    assert_eq!(
        load_script_roots(dir.path()).expect("should succeed"),
        vec!["../SharedScripts".to_string()]
    );
}

#[test]
fn load_script_roots_trims_entries_from_yaml() {
    let dir = tempfile::tempdir().expect("failed to create temp dir");
    write_config(
        dir.path(),
        "papyrus-lint.yaml",
        "additional_script_roots:\n  - '  ../SharedScripts  '\n  - '   '\n",
    );

    assert_eq!(
        load_script_roots(dir.path()).expect("should succeed"),
        vec!["../SharedScripts".to_string()]
    );
}

#[test]
fn save_script_roots_empty_clears_existing_roots() {
    let dir = tempfile::tempdir().expect("failed to create temp dir");
    save_script_roots(dir.path(), &["../SharedScripts".to_string()])
        .expect("saving script roots should succeed");

    save_script_roots(dir.path(), &[]).expect("clearing script roots should succeed");

    assert_eq!(
        load_script_roots(dir.path()).expect("should succeed"),
        Vec::<String>::new()
    );
}

#[test]
fn save_config_preserves_existing_script_roots() {
    let dir = tempfile::tempdir().expect("failed to create temp dir");
    save_script_roots(dir.path(), &["../SharedScripts".to_string()])
        .expect("saving script roots should succeed");

    let config = papyrus_lints::Config {
        semicolon: true,
        ..papyrus_lints::Config::default()
    };
    save_config(dir.path(), &config).expect("saving lint config should succeed");

    assert_eq!(
        load_script_roots(dir.path()).expect("should succeed"),
        vec!["../SharedScripts".to_string()]
    );
}
