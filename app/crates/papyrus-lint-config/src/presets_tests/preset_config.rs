use super::*;

fn initialized_preset(preset: Preset) -> papyrus_lints::Config {
    let dir = tempfile::tempdir().expect("failed to create temp dir");
    let path = initialize_config_with_base(dir.path(), None, preset).expect("init should succeed");
    load_config_from_path(&path).expect("generated config should parse")
}

fn reports(config: &papyrus_lints::Config, source: &str, rule: &str) -> bool {
    papyrus_lints::lint(source, config)
        .iter()
        .any(|diagnostic| diagnostic.rule == rule)
}

#[test]
fn standard_preset_turns_off_purely_stylistic_rules_but_keeps_formatting() {
    let config = initialized_preset(Preset::Standard);

    assert!(!reports(
        &config,
        "ScriptName Example\n\nFunction bad_name()\nEndFunction\n",
        "identifier-casing"
    ));
    assert!(!reports(&config, "ScriptName Example", "final-newline"));
    assert!(reports(
        &config,
        "ScriptName Example   \n",
        "trailing-whitespace"
    ));
    assert!(reports(
        &config,
        &format!("ScriptName Example ; {}\n", "x".repeat(120)),
        "line-length"
    ));
}

#[test]
fn careful_preset_relaxes_complexity_thresholds_and_disables_formatting() {
    let config = initialized_preset(Preset::Careful);
    let moderately_complex = "ScriptName Example\n\nFunction Test()\n\
        If true\n    EndIf\n    If true\n    EndIf\n    If true\n    EndIf\n\
        If true\n    EndIf\n    If true\n    EndIf\n    If true\n    EndIf\n\
        If true\n    EndIf\n    If true\n    EndIf\n    If true\n    EndIf\n\
        If true\n    EndIf\nEndFunction\n";

    assert!(reports(
        &initialized_preset(Preset::Strict),
        moderately_complex,
        "cyclomatic-complexity"
    ));
    assert!(!reports(
        &config,
        moderately_complex,
        "cyclomatic-complexity"
    ));
    assert!(!reports(&config, "ScriptName Example", "final-newline"));
    assert!(!reports(
        &config,
        "ScriptName Example   \n",
        "trailing-whitespace"
    ));
}

#[test]
fn strict_preset_matches_the_built_in_default() {
    let config = initialized_preset(Preset::Strict);
    let source = format!(
        "ScriptName Example\n\nFunction bad_name()\nEndFunction ; {}",
        "x".repeat(120)
    );

    assert_eq!(
        papyrus_lints::lint(&source, &config),
        papyrus_lints::lint(&source, &papyrus_lints::Config::default())
    );
    assert!(reports(&config, &source, "identifier-casing"));
    assert!(reports(&config, &source, "line-length"));
    assert!(reports(&config, &source, "final-newline"));
}

#[test]
fn executable_adjacent_base_config_overrides_a_selected_preset() {
    // A synthetic custom preset, not a real built-in one: this test only
    // exercises the base-config-over-preset merge mechanism itself, so it
    // shouldn't depend on which rules a built-in preset happens to change.
    let dir = tempfile::tempdir().expect("failed to create temp dir");
    let base_dir = tempfile::tempdir().expect("failed to create temp dir");
    let presets_dir = base_dir.path().join(USER_PRESETS_DIR_NAME);
    fs::create_dir(&presets_dir).expect("failed to create presets dir");
    write_config(
        &presets_dir,
        "custom.yaml",
        "cyclomatic_complexity_warning: 99\nrules:\n  trailing_whitespace: false\n",
    );
    write_config(
        base_dir.path(),
        "papyrus-lint.yaml",
        "semicolon: true\nrules:\n  property_sorting: true\n",
    );

    let path = initialize_config_with_base(
        dir.path(),
        Some(base_dir.path()),
        Preset::Custom("custom".to_string()),
    )
    .expect("init should succeed");
    let generated = fs::read_to_string(&path).expect("failed to read generated config");

    // The base's own settings win, even over the preset's own values...
    assert!(generated.contains("semicolon: true\n"));
    assert!(generated.contains("  property_sorting: true\n"));
    // ...while every other rule/setting still falls back to the
    // selected preset rather than the hardcoded built-in default.
    assert!(generated.contains("cyclomatic_complexity_warning: 99\n"));
    assert!(generated.contains("  trailing_whitespace: false\n"));
}

#[test]
fn preset_lint_config_matches_the_lint_settings_a_fresh_init_would_write() {
    let config = preset_lint_config(None, Preset::Careful).expect("should resolve");

    let dir = tempfile::tempdir().expect("failed to create temp dir");
    let path = initialize_config_with_base(dir.path(), None, Preset::Careful)
        .expect("init should succeed");
    let initialized = load_config_from_path(&path).expect("failed to load generated config");

    assert_eq!(config, initialized);
}

#[test]
fn preset_lint_config_ignores_a_pre_existing_project_config() {
    // Unlike initialize_config_with_base, preset_lint_config never looks
    // at (or requires the absence of) a project's own config file: it
    // only resolves the preset's own settings, for resetting an
    // existing project's already-edited settings back to it in place.
    let dir = tempfile::tempdir().expect("failed to create temp dir");
    write_config(dir.path(), "papyrus-lint.yaml", "semicolon: true\n");

    let config = preset_lint_config(None, Preset::Strict).expect("should resolve");

    assert_eq!(config, papyrus_lints::Config::default());
}

#[test]
fn preset_lint_config_is_still_layered_over_an_executable_adjacent_base_config() {
    // Same synthetic-preset approach as
    // `executable_adjacent_base_config_overrides_a_selected_preset`: only
    // the merge mechanism is under test here, not a real preset's content.
    let base_dir = tempfile::tempdir().expect("failed to create temp dir");
    let presets_dir = base_dir.path().join(USER_PRESETS_DIR_NAME);
    fs::create_dir(&presets_dir).expect("failed to create presets dir");
    write_config(
        &presets_dir,
        "custom.yaml",
        "cyclomatic_complexity_warning: 99\nrules:\n  trailing_whitespace: false\n",
    );
    write_config(
        base_dir.path(),
        "papyrus-lint.yaml",
        "semicolon: true\nrules:\n  property_sorting: true\n",
    );

    let config = preset_lint_config(Some(base_dir.path()), Preset::Custom("custom".to_string()))
        .expect("should resolve");

    assert!(config.semicolon);
    assert!(config.rules.property_sorting);
    // Every other rule/setting still falls back to the selected preset
    // rather than the hardcoded built-in default.
    assert_eq!(config.cyclomatic_complexity_warning, 99);
    assert!(!config.rules.trailing_whitespace);
}

#[test]
fn preset_lint_config_default_resolves_a_built_in_preset() {
    assert_eq!(
        preset_lint_config_default(Preset::Strict).expect("should resolve"),
        papyrus_lints::Config::default()
    );
}

#[test]
fn preset_lint_config_errors_on_an_unknown_custom_preset() {
    let dir = tempfile::tempdir().expect("failed to create temp dir");

    let error = preset_lint_config(Some(dir.path()), Preset::Custom("missing".to_string()))
        .expect_err("should error");

    assert!(error.contains("unknown preset 'missing'"));
}
