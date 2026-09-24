use super::*;
use crate::presets::{initialize_default_config, Preset};
use crate::skyrim::detected_skyrim_script_lookup_dirs;

#[test]
fn parse_lint_yaml_returns_defaults_for_empty_and_whitespace_only_documents() {
    assert_eq!(
        parse_lint_yaml("").expect("empty yaml should parse"),
        papyrus_lints::Config::default()
    );
    assert_eq!(
        parse_lint_yaml(" \t\n\r\n").expect("whitespace-only yaml should parse"),
        papyrus_lints::Config::default()
    );
}

#[test]
fn parse_lint_yaml_applies_omitted_keys_as_defaults() {
    let config = parse_lint_yaml("semicolon: true\nindentation: space\n")
        .expect("partial yaml should parse");

    assert!(config.semicolon);
    assert_eq!(config.indentation, Indentation::Space);
    assert_eq!(config.indentation_width, 4);
    assert_eq!(config.max_line_length, 120);
    assert!(!config.fail_on_warning);
    assert!(config.rules.trailing_whitespace);
}

#[test]
fn parse_lint_yaml_ignores_app_level_keys() {
    let config = parse_lint_yaml(
        "compiler_path: C:\\Tools\\PapyrusCompiler.exe\ncompile_check: true\nsemicolon: true\n",
    )
    .expect("project yaml should parse as lint config");

    assert!(config.semicolon);
    assert_eq!(config.indentation, Indentation::default());
}

#[test]
fn parse_lint_yaml_rejects_invalid_yaml() {
    assert!(parse_lint_yaml("semicolon: [not a bool\n").is_err());
    assert!(parse_lint_yaml("indentation: eight-spaces\n").is_err());
}

#[test]
fn lint_config_to_yaml_round_trips_through_parse_lint_yaml() {
    let config = papyrus_lints::Config {
        semicolon: true,
        indentation: Indentation::Space,
        indentation_width: 2,
        ..papyrus_lints::Config::default()
    };

    let yaml = lint_config_to_yaml(&config).expect("config should serialize");
    assert_eq!(
        parse_lint_yaml(&yaml).expect("serialized config should parse"),
        config
    );
}

#[test]
fn save_config_at_path_rejects_invalid_existing_yaml_without_overwriting_it() {
    let dir = tempfile::tempdir().expect("failed to create temp dir");
    let path = dir.path().join("custom-config.yaml");
    let invalid = "semicolon: [not a bool\n";
    fs::write(&path, invalid).expect("failed to write test config file");

    assert!(save_config_at_path(&path, &papyrus_lints::Config::default()).is_err());
    assert_eq!(
        fs::read_to_string(path).expect("failed to read test config file"),
        invalid
    );
}

#[test]
fn default_config_matches_the_checked_in_configuration_copy() {
    let dir = tempfile::tempdir().expect("failed to create temp dir");

    let path =
        initialize_default_config(dir.path(), Preset::default()).expect("init should succeed");
    let generated = fs::read_to_string(&path).expect("failed to read generated config");

    let checked_in_path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../../configuration/papyrus-lint.default.yaml");
    let checked_in_copy = fs::read_to_string(&checked_in_path)
        .expect("failed to read configuration/papyrus-lint.default.yaml");

    let detected = detected_skyrim_script_lookup_dirs();
    if detected.is_empty() {
        assert_eq!(
            generated, checked_in_copy,
            "configuration/papyrus-lint.default.yaml is out of date; regenerate it with `PapyrusLinterCLI init`"
        );
    } else {
        for dir in &detected {
            assert!(
                generated.contains(dir),
                "init should fill lookup_script_roots with {dir}"
            );
        }
    }
}

#[test]
fn errors_on_invalid_yaml() {
    let dir = tempfile::tempdir().expect("failed to create temp dir");
    write_config(dir.path(), "papyrus-lint.yaml", "semicolon: [not a bool\n");

    let result = load_config(dir.path());

    assert!(result.is_err());
}

#[test]
fn save_creates_yaml_file_when_none_exists() {
    let dir = tempfile::tempdir().expect("failed to create temp dir");
    let config = papyrus_lints::Config {
        semicolon: true,
        indentation: Indentation::Space,
        indentation_width: 2,
        ..papyrus_lints::Config::default()
    };

    save_config(dir.path(), &config).expect("saving should succeed");

    assert!(dir.path().join("papyrus-lint.yaml").is_file());
    assert!(!dir.path().join("papyrus-lint.yml").exists());
    let loaded = load_config(dir.path()).expect("loading should succeed");
    assert_eq!(loaded, config);
}

#[test]
fn save_annotates_top_level_keys_with_explanatory_comments() {
    let dir = tempfile::tempdir().expect("failed to create temp dir");
    let config = papyrus_lints::Config {
        semicolon: true,
        ..papyrus_lints::Config::default()
    };

    save_config(dir.path(), &config).expect("saving should succeed");

    let contents = fs::read_to_string(dir.path().join("papyrus-lint.yaml"))
        .expect("failed to read saved config file");
    assert!(contents
        .starts_with("# Target game. Currently supported: skyrim, fallout4\ngame: skyrim\n"));
    assert!(contents.contains("# true, false\nsemicolon: true\n"));
    assert!(contents.contains("# tab, space\nindentation: tab\n"));
    assert!(contents.contains("# Each rule accepts true or false\nrules:\n"));
    // Nested rule keys aren't individually commented, matching the
    // README's example, which only comments the `rules:` block itself.
    assert!(!contents.contains("trailing_whitespace:\n  #"));
}

#[test]
fn save_overwrites_existing_yaml_file() {
    let dir = tempfile::tempdir().expect("failed to create temp dir");
    write_config(dir.path(), "papyrus-lint.yaml", "semicolon: false\n");
    let config = papyrus_lints::Config {
        semicolon: true,
        ..papyrus_lints::Config::default()
    };

    save_config(dir.path(), &config).expect("saving should succeed");

    let loaded = load_config(dir.path()).expect("loading should succeed");
    assert_eq!(loaded, config);
}

#[test]
fn save_prefers_existing_yml_file_over_creating_yaml() {
    let dir = tempfile::tempdir().expect("failed to create temp dir");
    write_config(dir.path(), "papyrus-lint.yml", "semicolon: false\n");
    let config = papyrus_lints::Config {
        semicolon: true,
        ..papyrus_lints::Config::default()
    };

    save_config(dir.path(), &config).expect("saving should succeed");

    assert!(!dir.path().join("papyrus-lint.yaml").exists());
    let loaded = load_config(dir.path()).expect("loading should succeed");
    assert_eq!(loaded, config);
}
