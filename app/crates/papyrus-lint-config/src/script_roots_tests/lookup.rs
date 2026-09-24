use super::*;
use crate::fallout4::detected_fallout4_script_lookup_dirs;

#[test]
fn load_lookup_script_roots_returns_empty_when_unset() {
    let dir = tempfile::tempdir().expect("failed to create temp dir");

    assert_eq!(
        load_lookup_script_roots(dir.path()).expect("should succeed"),
        Vec::<String>::new()
    );
}

#[test]
fn save_and_load_lookup_script_roots_round_trips_without_disturbing_other_settings() {
    let dir = tempfile::tempdir().expect("failed to create temp dir");
    save_script_roots(dir.path(), &["../SharedScripts".to_string()])
        .expect("saving script roots should succeed");

    save_lookup_script_roots(
        dir.path(),
        &[
            "  C:/Skyrim/Data/Scripts/Source  ".to_string(),
            "  ".to_string(),
            "C:/Skyrim/Data/Source/Scripts".to_string(),
        ],
    )
    .expect("saving lookup roots should succeed");

    assert_eq!(
        load_lookup_script_roots(dir.path()).expect("should succeed"),
        vec![
            "C:/Skyrim/Data/Scripts/Source".to_string(),
            "C:/Skyrim/Data/Source/Scripts".to_string()
        ]
    );
    assert_eq!(
        load_script_roots(dir.path()).expect("should succeed"),
        vec!["../SharedScripts".to_string()]
    );
}

#[test]
fn save_lookup_script_roots_empty_is_kept_explicit_and_not_refilled() {
    let dir = tempfile::tempdir().expect("failed to create temp dir");
    save_lookup_script_roots(dir.path(), &["C:/Skyrim/Data/Scripts/Source".to_string()])
        .expect("saving lookup roots should succeed");

    save_lookup_script_roots(dir.path(), &[]).expect("clearing lookup roots should succeed");

    assert_eq!(
        load_lookup_script_roots(dir.path()).expect("should succeed"),
        Vec::<String>::new()
    );
    let contents = fs::read_to_string(dir.path().join("papyrus-lint.yaml"))
        .expect("failed to read saved config");
    assert!(contents.contains("lookup_script_roots:"));
}

#[test]
fn load_lookup_script_roots_trims_entries_from_yaml() {
    let dir = tempfile::tempdir().expect("failed to create temp dir");
    write_config(
        dir.path(),
        "papyrus-lint.yaml",
        "lookup_script_roots:\n  - '  C:/Skyrim/Data/Scripts/Source  '\n  - '   '\n",
    );

    assert_eq!(
        load_lookup_script_roots(dir.path()).expect("should succeed"),
        vec!["C:/Skyrim/Data/Scripts/Source".to_string()]
    );
}

#[test]
fn save_config_preserves_existing_lookup_script_roots() {
    let dir = tempfile::tempdir().expect("failed to create temp dir");
    save_lookup_script_roots(dir.path(), &["C:/Skyrim/Data/Scripts/Source".to_string()])
        .expect("saving lookup roots should succeed");

    let config = papyrus_lints::Config {
        semicolon: true,
        ..papyrus_lints::Config::default()
    };
    save_config(dir.path(), &config).expect("saving lint config should succeed");

    assert_eq!(
        load_lookup_script_roots(dir.path()).expect("should succeed"),
        vec!["C:/Skyrim/Data/Scripts/Source".to_string()]
    );
}

#[test]
fn seed_lookup_script_roots_fills_only_when_the_key_was_missing() {
    let mut unset = ProjectFile::default();
    merge_lookup_roots(
        &mut unset.lookup_script_roots,
        &["C:/Skyrim/Data/Scripts/Source".to_string()],
    );
    assert_eq!(
        unset.lookup_script_roots,
        vec!["C:/Skyrim/Data/Scripts/Source".to_string()]
    );

    let mut explicit = ProjectFile {
        lookup_script_roots_explicit: true,
        ..ProjectFile::default()
    };
    seed_lookup_script_roots(&mut explicit);
    assert!(explicit.lookup_script_roots.is_empty());
}

#[test]
fn project_file_from_yaml_seeds_lookup_script_roots_from_the_configured_game_not_skyrim() {
    let project = project_file_from_yaml("game: fallout4\n").expect("parsing should succeed");

    assert_eq!(
        project.lookup_script_roots,
        detected_fallout4_script_lookup_dirs(),
        "a fallout4 project must be seeded from Fallout 4's own detected install, \
         never Skyrim's (see issue #1187)"
    );
}

#[test]
fn seed_lookup_script_roots_uses_the_projects_configured_game() {
    let mut fallout4 = ProjectFile {
        lint: papyrus_lints::Config {
            game: papyrus_lints::Game::Fallout4,
            ..papyrus_lints::Config::default()
        },
        ..ProjectFile::default()
    };

    seed_lookup_script_roots(&mut fallout4);

    assert_eq!(
        fallout4.lookup_script_roots,
        detected_fallout4_script_lookup_dirs()
    );
}

#[test]
fn updating_a_config_without_lookup_script_roots_writes_the_key() {
    let dir = tempfile::tempdir().expect("failed to create temp dir");
    write_config(dir.path(), "papyrus-lint.yaml", "semicolon: true\n");

    save_script_roots(dir.path(), &["../SharedScripts".to_string()])
        .expect("saving script roots should succeed");

    let contents = fs::read_to_string(dir.path().join("papyrus-lint.yaml"))
        .expect("failed to read saved config");
    assert!(contents.contains("lookup_script_roots:"));
    assert!(contents.contains("semicolon: true"));
}
