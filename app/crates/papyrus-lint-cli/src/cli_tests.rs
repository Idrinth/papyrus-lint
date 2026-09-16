use super::*;

#[test]
fn unix_timestamps_are_formatted_as_utc_rfc3339() {
    assert_eq!(format_unix_timestamp(0, 0), "1970-01-01T00:00:00.000Z");
    assert_eq!(
        format_unix_timestamp(1_709_251_199, 42),
        "2024-02-29T23:59:59.042Z"
    );
}

#[test]
fn unix_timestamps_handle_calendar_boundaries_and_millisecond_padding() {
    assert_eq!(
        format_unix_timestamp(31_535_999, 7),
        "1970-12-31T23:59:59.007Z"
    );
    assert_eq!(
        format_unix_timestamp(31_536_000, 70),
        "1971-01-01T00:00:00.070Z"
    );
    assert_eq!(
        format_unix_timestamp(951_782_400, 999),
        "2000-02-29T00:00:00.999Z"
    );
}

#[test]
fn severity_counts_tallies_each_known_level_and_ignores_unknown_levels() {
    let diagnostics = [
        JsonDiagnostic {
            line: 1,
            column: 1,
            rule: "first-rule",
            level: "error",
            message: "first".to_string(),
            doc_url: None,
        },
        JsonDiagnostic {
            line: 2,
            column: 1,
            rule: "second-rule",
            level: "warning",
            message: "second".to_string(),
            doc_url: None,
        },
        JsonDiagnostic {
            line: 3,
            column: 1,
            rule: "third-rule",
            level: "info",
            message: "third".to_string(),
            doc_url: None,
        },
        JsonDiagnostic {
            line: 4,
            column: 1,
            rule: "future-rule",
            level: "notice",
            message: "future".to_string(),
            doc_url: None,
        },
    ];

    let counts = severity_counts(&diagnostics);

    assert_eq!(counts.errors, 1);
    assert_eq!(counts.warnings, 1);
    assert_eq!(counts.info, 1);
}

#[test]
fn rule_counts_aggregates_duplicates_in_sorted_rule_order() {
    let diagnostics = [
        JsonDiagnostic {
            line: 1,
            column: 1,
            rule: "z-rule",
            level: "warning",
            message: "first".to_string(),
            doc_url: None,
        },
        JsonDiagnostic {
            line: 2,
            column: 1,
            rule: "a-rule",
            level: "warning",
            message: "second".to_string(),
            doc_url: None,
        },
        JsonDiagnostic {
            line: 3,
            column: 1,
            rule: "z-rule",
            level: "warning",
            message: "third".to_string(),
            doc_url: None,
        },
    ];

    let counts = rule_counts(&diagnostics);

    assert_eq!(
        counts.keys().copied().collect::<Vec<_>>(),
        ["a-rule", "z-rule"]
    );
    assert_eq!(counts["a-rule"], 1);
    assert_eq!(counts["z-rule"], 2);
}

#[test]
fn ai_configuration_replaces_rule_flags_with_enabled_rule_ids() {
    let config = papyrus_lints::Config::default();

    let value = ai_configuration(&config);
    let object = value
        .as_object()
        .expect("configuration should be an object");
    let enabled = object["enabled_rules"]
        .as_array()
        .expect("enabled_rules should be an array");

    assert!(!object.contains_key("rules"));
    assert!(enabled.contains(&serde_json::json!("trailing-whitespace")));
    assert!(!enabled.contains(&serde_json::json!("property-sorting")));
}

#[test]
fn display_path_only_shortens_paths_when_requested() {
    let path = Path::new("project/scripts/source/Example.psc");
    let root = Path::new("project");

    assert_eq!(
        display_path(path, root, true),
        Path::new("scripts/source/Example.psc")
            .display()
            .to_string()
    );
    assert_eq!(display_path(path, root, false), path.display().to_string());
}

fn write_file(path: &Path, contents: &str) {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).expect("failed to create parent dir");
    }
    fs::write(path, contents).expect("failed to write file");
}

fn run_captured_with_terminal_stdout(
    args: &[String],
    stdout_is_terminal: bool,
) -> (u8, String, String) {
    let mut stdout = Vec::new();
    let mut stderr = Vec::new();
    let code = run(args, &mut stdout, &mut stderr, stdout_is_terminal);
    (
        code,
        String::from_utf8(stdout).expect("stdout should be utf8"),
        String::from_utf8(stderr).expect("stderr should be utf8"),
    )
}

fn run_captured(args: &[String]) -> (u8, String, String) {
    run_captured_with_terminal_stdout(args, false)
}

#[test]
fn candidate_pair_root_recognizes_every_supported_directory_order() {
    let root = Path::new("project");

    assert_eq!(
        find_candidate_pair_root(&root.join("scripts/source/Example.psc")),
        Some(root.to_path_buf())
    );
    assert_eq!(
        find_candidate_pair_root(&root.join("source/scripts/Example.psc")),
        Some(root.to_path_buf())
    );
}

#[test]
fn candidate_pair_root_is_case_insensitive_and_supports_nested_scripts() {
    let script = Path::new("project/SCRIPTS/Source/User/Example.psc");

    assert_eq!(
        find_candidate_pair_root(script),
        Some(PathBuf::from("project"))
    );
}

#[test]
fn psc_project_root_uses_the_legacy_fallback_without_a_candidate_pair() {
    assert_eq!(
        find_psc_project_root(Path::new("project/custom/source/Example.psc")),
        PathBuf::from("project")
    );
    assert_eq!(
        find_psc_project_root(Path::new("Example.psc")),
        PathBuf::from(".")
    );
}

#[test]
fn prints_usage_and_exits_2_with_no_arguments() {
    let (code, _stdout, stderr) = run_captured(&[]);

    assert_eq!(code, 2);
    assert!(stderr.contains("Usage: PapyrusLinterCLI"));
}

#[test]
fn prints_version_for_version_flag() {
    let (code, stdout, _stderr) = run_captured(&["--version".to_string()]);

    assert_eq!(code, 0);
    assert_eq!(stdout, format!("PapyrusLinterCLI {VERSION}\n"));
}

#[test]
fn prints_version_for_short_version_flag() {
    let (code, stdout, _stderr) = run_captured(&["-V".to_string()]);

    assert_eq!(code, 0);
    assert_eq!(stdout, format!("PapyrusLinterCLI {VERSION}\n"));
}

#[test]
fn prints_usage_for_help_flag() {
    let (code, _stdout, stderr) = run_captured(&["--help".to_string()]);

    assert_eq!(code, 2);
    assert!(stderr.contains("Usage: PapyrusLinterCLI"));
    assert!(stderr.contains("everything an AI needs to assist"));
}

#[test]
fn prints_usage_with_too_many_arguments() {
    let (code, _stdout, stderr) = run_captured(&["a".to_string(), "b".to_string()]);

    assert_eq!(code, 2);
    assert!(stderr.contains("Usage: PapyrusLinterCLI"));
}

#[test]
fn prints_usage_when_preset_is_given_without_add() {
    let (code, _stdout, stderr) = run_captured(&["preset".to_string()]);

    assert_eq!(code, 2);
    assert!(stderr.contains("Usage: PapyrusLinterCLI"));
}

#[test]
fn prints_usage_for_an_unrecognized_preset_subcommand() {
    let (code, _stdout, stderr) = run_captured(&["preset".to_string(), "remove".to_string()]);

    assert_eq!(code, 2);
    assert!(stderr.contains("Usage: PapyrusLinterCLI"));
}

#[test]
fn prints_usage_when_preset_add_is_missing_arguments() {
    let (code, _stdout, stderr) = run_captured(&[
        "preset".to_string(),
        "add".to_string(),
        "my-team".to_string(),
    ]);

    assert_eq!(code, 2);
    assert!(stderr.contains("Usage: PapyrusLinterCLI"));
}

#[test]
fn parse_preset_add_args_parses_the_two_positionals() {
    assert_eq!(
        parse_preset_add_args(&[
            "my-team".to_string(),
            "path/to/papyrus-lint.yaml".to_string()
        ]),
        Ok((
            "my-team".to_string(),
            PathBuf::from("path/to/papyrus-lint.yaml"),
            false
        ))
    );
}

#[test]
fn parse_preset_add_args_recognizes_yes_in_any_position() {
    assert_eq!(
        parse_preset_add_args(&[
            "--yes".to_string(),
            "my-team".to_string(),
            "path/to/papyrus-lint.yaml".to_string()
        ]),
        Ok((
            "my-team".to_string(),
            PathBuf::from("path/to/papyrus-lint.yaml"),
            true
        ))
    );
    assert_eq!(
        parse_preset_add_args(&[
            "my-team".to_string(),
            "path/to/papyrus-lint.yaml".to_string(),
            "--yes".to_string()
        ]),
        Ok((
            "my-team".to_string(),
            PathBuf::from("path/to/papyrus-lint.yaml"),
            true
        ))
    );
}

#[test]
fn parse_preset_add_args_rejects_a_missing_argument() {
    let err = parse_preset_add_args(&["my-team".to_string()])
        .expect_err("a single positional argument should be rejected");
    assert!(matches!(err, PresetAddArgsError::Usage));
}

#[test]
fn parse_preset_add_args_rejects_an_extra_argument() {
    let err = parse_preset_add_args(&[
        "my-team".to_string(),
        "path.yaml".to_string(),
        "extra".to_string(),
    ])
    .expect_err("an extra positional argument should be rejected");
    assert!(matches!(err, PresetAddArgsError::Usage));
}

#[test]
fn parse_preset_add_args_rejects_an_unrecognized_flag() {
    let err = parse_preset_add_args(&[
        "my-team".to_string(),
        "path.yaml".to_string(),
        "--force".to_string(),
    ])
    .expect_err("an unrecognized flag should be rejected");
    assert!(matches!(err, PresetAddArgsError::Usage));
}

#[test]
fn report_add_user_preset_prints_the_added_path_on_success() {
    let mut stdout = Vec::new();
    let mut stderr = Vec::new();

    let code = report_add_user_preset(
        "my-team",
        Ok(PathBuf::from("/presets/my-team.yaml")),
        &mut stdout,
        &mut stderr,
    );

    assert_eq!(code, 0);
    assert!(stderr.is_empty());
    assert_eq!(
        String::from_utf8(stdout).unwrap(),
        "Added preset 'my-team' at /presets/my-team.yaml\n"
    );
}

#[test]
fn report_add_user_preset_explains_how_to_confirm_an_overwrite() {
    let mut stdout = Vec::new();
    let mut stderr = Vec::new();

    let code = report_add_user_preset(
        "my-team",
        Err(config::AddPresetError::AlreadyExists(PathBuf::from(
            "/presets/my-team.yaml",
        ))),
        &mut stdout,
        &mut stderr,
    );

    assert_eq!(code, 2);
    assert!(stdout.is_empty());
    let error = String::from_utf8(stderr).unwrap();
    assert!(error.contains("already exists"));
    assert!(error.contains("--yes"));
}

#[test]
fn report_add_user_preset_reports_an_invalid_name() {
    let mut stdout = Vec::new();
    let mut stderr = Vec::new();

    let code = report_add_user_preset(
        "strict",
        Err(config::AddPresetError::InvalidName("strict".to_string())),
        &mut stdout,
        &mut stderr,
    );

    assert_eq!(code, 2);
    assert!(stdout.is_empty());
    assert!(String::from_utf8(stderr)
        .unwrap()
        .contains("built-in preset"));
}

#[test]
fn init_creates_a_default_config() {
    let dir = tempfile::tempdir().expect("failed to create temp dir");
    let mut stdout = Vec::new();
    let mut stderr = Vec::new();

    let code = initialize_config(
        dir.path(),
        config::Preset::default(),
        &mut stdout,
        &mut stderr,
    );

    assert_eq!(code, 0);
    assert!(stderr.is_empty());
    assert!(String::from_utf8(stdout)
        .unwrap()
        .contains("papyrus-lint.yaml"));
    let config = config::load_config(dir.path()).expect("config should load");
    assert_eq!(config, papyrus_lints::Config::default());
}

#[test]
fn init_refuses_to_overwrite_an_existing_config() {
    let dir = tempfile::tempdir().expect("failed to create temp dir");
    let path = dir.path().join("papyrus-lint.yml");
    write_file(&path, "semicolon: true\n");
    let mut stdout = Vec::new();
    let mut stderr = Vec::new();

    let code = initialize_config(
        dir.path(),
        config::Preset::default(),
        &mut stdout,
        &mut stderr,
    );

    assert_eq!(code, 2);
    assert!(stdout.is_empty());
    assert!(String::from_utf8(stderr)
        .unwrap()
        .contains("config already exists"));
    assert_eq!(fs::read_to_string(path).unwrap(), "semicolon: true\n");
}

#[test]
fn parse_init_preset_defaults_to_strict_when_no_flag_is_given() {
    assert_eq!(parse_init_preset(&[]), Ok(config::Preset::Strict));
}

#[test]
fn parse_init_preset_accepts_the_flag_and_its_equals_form() {
    assert_eq!(
        parse_init_preset(&["--preset".to_string(), "careful".to_string()]),
        Ok(config::Preset::Careful)
    );
    assert_eq!(
        parse_init_preset(&["--preset=standard".to_string()]),
        Ok(config::Preset::Standard)
    );
}

#[test]
fn parse_init_preset_matches_names_case_insensitively() {
    assert_eq!(
        parse_init_preset(&["--preset".to_string(), "STANDARD".to_string()]),
        Ok(config::Preset::Standard)
    );
}

#[test]
fn parse_init_preset_accepts_a_name_that_is_not_a_built_in_as_a_custom_preset() {
    // Whether a name actually matches a user preset file is only
    // checked once `init` runs (see `config::Preset::yaml`), not during
    // argument parsing, so an arbitrary non-blank name parses fine here.
    assert_eq!(
        parse_init_preset(&["--preset".to_string(), "lenient".to_string()]),
        Ok(config::Preset::Custom("lenient".to_string()))
    );
}

#[test]
fn parse_init_preset_rejects_a_missing_value() {
    let err = parse_init_preset(&["--preset".to_string()])
        .expect_err("a --preset with no value should be rejected");
    assert!(matches!(err, InitPresetError::Usage));
}

#[test]
fn parse_init_preset_rejects_a_blank_value() {
    let err = parse_init_preset(&["--preset=".to_string()])
        .expect_err("a blank --preset value should be rejected");
    assert!(matches!(err, InitPresetError::Usage));
}

#[test]
fn parse_init_preset_rejects_an_unrecognized_extra_argument() {
    let err = parse_init_preset(&["extra".to_string()])
        .expect_err("an argument other than --preset should be rejected");
    assert!(matches!(err, InitPresetError::Usage));
}

#[test]
fn run_init_with_an_unresolvable_preset_name_reports_an_error_at_init_time() {
    // No `presets` directory exists next to the test binary, so a name
    // that isn't a built-in preset fails once `init` actually looks for
    // a matching file, rather than during argument parsing.
    let (code, _stdout, stderr) =
        run_captured(&["init".to_string(), "--preset=lenient".to_string()]);

    assert_eq!(code, 2);
    assert!(stderr.contains("unknown preset 'lenient'"));
}

#[test]
fn run_init_with_a_preset_flag_writes_the_selected_presets_config() {
    // `run(["init", ...], ...)` writes into the process's actual
    // current directory (see `initialize_config`'s call in `run`),
    // which isn't safe to exercise from a parallel test suite. The
    // argument parsing itself (see the `parse_init_preset` tests
    // above) and `initialize_config`'s own preset handling (see
    // `init_creates_a_default_config` above and
    // `papyrus-lint-core`'s own preset tests) are covered separately,
    // so this only checks that `run` wires the two together for a
    // `--preset` other than the default.
    let dir = tempfile::tempdir().expect("failed to create temp dir");
    let mut stdout = Vec::new();
    let mut stderr = Vec::new();

    let preset =
        parse_init_preset(&["--preset=careful".to_string()]).expect("careful should parse");
    let code = initialize_config(dir.path(), preset, &mut stdout, &mut stderr);

    assert_eq!(code, 0);
    let generated = fs::read_to_string(dir.path().join("papyrus-lint.yaml"))
        .expect("failed to read generated config");
    assert!(generated.contains("cyclomatic_complexity_warning: 20\n"));
}

#[test]
fn errors_when_achlist_is_missing() {
    let dir = tempfile::tempdir().expect("failed to create temp dir");
    let achlist_path = dir.path().join("missing.achlist");

    let (code, _stdout, stderr) = run_captured(&[achlist_path.to_string_lossy().into_owned()]);

    assert_eq!(code, 2);
    assert!(stderr.starts_with("error:"));
}

#[test]
fn reports_no_problems_for_a_clean_project() {
    let dir = tempfile::tempdir().expect("failed to create temp dir");
    write_file(
        &dir.path().join("scripts/source/Example.psc"),
        "ScriptName Example\n",
    );
    write_file(
        &dir.path().join("sources.achlist"),
        r#"["scripts/source/Example.psc"]"#,
    );
    let achlist_path = dir.path().join("sources.achlist");

    let (code, stdout, _stderr) = run_captured(&[achlist_path.to_string_lossy().into_owned()]);

    assert_eq!(code, 0);
    assert!(stdout.contains("no problems found in 1 script"));
}

#[test]
fn reports_diagnostics_and_exits_1_for_a_dirty_project() {
    let dir = tempfile::tempdir().expect("failed to create temp dir");
    write_file(
        &dir.path().join("scripts/source/Example.psc"),
        "ScriptName Example\n\nFunction DoThing()\n\tGame.GetPlayer()\nEndFunction\n",
    );
    write_file(
        &dir.path().join("sources.achlist"),
        r#"["scripts/source/Example.psc"]"#,
    );
    let achlist_path = dir.path().join("sources.achlist");

    let (code, stdout, _stderr) = run_captured(&[achlist_path.to_string_lossy().into_owned()]);

    assert_eq!(code, 1);
    assert!(stdout.contains("[forbidden-functions]"));
    assert!(stdout.contains("problem(s) found in 1 of 1 script(s)"));
}

#[test]
fn does_not_fail_on_warning_level_diagnostics_by_default() {
    let dir = tempfile::tempdir().expect("failed to create temp dir");
    write_file(
        &dir.path().join("scripts/source/Example.psc"),
        "ScriptName Example\n\nInt Property MyValue = 1 Auto\n\nFunction DoThing()\nEndFunction\n",
    );
    write_file(
        &dir.path().join("sources.achlist"),
        r#"["scripts/source/Example.psc"]"#,
    );
    let achlist_path = dir.path().join("sources.achlist");

    let (code, stdout, _stderr) = run_captured(&[achlist_path.to_string_lossy().into_owned()]);

    assert_eq!(code, 0);
    assert!(stdout.contains("[unused-property]"));
    assert!(stdout.contains("1 problem(s) found in 1 of 1 script(s)"));
}

#[test]
fn fails_on_warning_level_diagnostics_when_configured() {
    let dir = tempfile::tempdir().expect("failed to create temp dir");
    write_file(
        &dir.path().join("scripts/source/Example.psc"),
        "ScriptName Example\n\nInt Property MyValue = 1 Auto\n\nFunction DoThing()\nEndFunction\n",
    );
    write_file(
        &dir.path().join("sources.achlist"),
        r#"["scripts/source/Example.psc"]"#,
    );
    write_file(
        &dir.path().join("papyrus-lint.yaml"),
        "fail_on_warning: true\n",
    );
    let achlist_path = dir.path().join("sources.achlist");

    let (code, stdout, _stderr) = run_captured(&[achlist_path.to_string_lossy().into_owned()]);

    assert_eq!(code, 1);
    assert!(stdout.contains("[unused-property]"));
}

#[test]
fn quiet_warnings_hides_warnings_without_changing_the_exit_code() {
    let dir = tempfile::tempdir().expect("failed to create temp dir");
    let script_path = dir.path().join("scripts/source/Example.psc");
    write_file(&script_path, "ScriptName Example   \n");
    write_file(
        &dir.path().join("papyrus-lint.yaml"),
        "fail_on_warning: true\n",
    );

    let (code, stdout, stderr) = run_captured(&[
        "--quiet-warnings".to_string(),
        script_path.to_string_lossy().into_owned(),
    ]);

    assert_eq!(code, 1);
    assert!(stderr.is_empty());
    assert!(!stdout.contains("[warning]"));
    assert!(stdout.contains("no problems found in 1 script"));
}

#[test]
fn quiet_info_hides_info_diagnostics_from_json_without_changing_the_exit_code() {
    let dir = tempfile::tempdir().expect("failed to create temp dir");
    let script_path = dir.path().join("scripts/source/Example.psc");
    write_file(
        &script_path,
        "ScriptName Example\n\nGlobalVariable Property Value Auto\n\nFunction Test()\n    Value.GetValueInt()\nEndFunction\n",
    );
    write_file(
        &dir.path().join("papyrus-lint.yaml"),
        "fail_on_info: true\n",
    );

    let (unfiltered_code, unfiltered_stdout, _) = run_captured(&[
        "--json".to_string(),
        script_path.to_string_lossy().into_owned(),
    ]);
    let unfiltered: serde_json::Value = serde_json::from_str(&unfiltered_stdout).unwrap();
    assert_eq!(unfiltered_code, 1);
    assert!(unfiltered["files"][0]["diagnostics"]
        .as_array()
        .unwrap()
        .iter()
        .any(|diagnostic| diagnostic["level"] == "info"));

    let (code, stdout, stderr) = run_captured(&[
        "--json".to_string(),
        "--quiet-info".to_string(),
        script_path.to_string_lossy().into_owned(),
    ]);

    assert_eq!(code, 1);
    assert!(stderr.is_empty());
    let report: serde_json::Value = serde_json::from_str(&stdout).unwrap();
    assert_eq!(report["success"], false);
    let diagnostics = report["files"][0]["diagnostics"].as_array().unwrap();
    assert_eq!(report["total_diagnostics"], diagnostics.len());
    assert!(diagnostics
        .iter()
        .all(|diagnostic| diagnostic["level"] != "info"));
}

#[test]
fn short_paths_strips_the_project_root_from_reported_paths() {
    let dir = tempfile::tempdir().expect("failed to create temp dir");
    write_file(
        &dir.path().join("scripts/source/Example.psc"),
        "ScriptName Example   \n",
    );
    write_file(
        &dir.path().join("sources.achlist"),
        r#"["scripts/source/Example.psc"]"#,
    );
    let achlist_path = dir.path().join("sources.achlist");

    let (code, stdout, _stderr) = run_captured(&[
        "--short-paths".to_string(),
        achlist_path.to_string_lossy().into_owned(),
    ]);

    assert_eq!(code, 0);
    assert!(stdout.contains("scripts/source/Example.psc:"));
    assert!(stdout.contains("[trailing-whitespace]"));
    assert!(!stdout.contains(dir.path().to_string_lossy().as_ref()));
}

#[test]
fn short_paths_strips_the_project_root_from_json_paths() {
    let dir = tempfile::tempdir().expect("failed to create temp dir");
    write_file(
        &dir.path().join("scripts/source/Example.psc"),
        "ScriptName Example   \n",
    );
    write_file(
        &dir.path().join("sources.achlist"),
        r#"["scripts/source/Example.psc"]"#,
    );
    let achlist_path = dir.path().join("sources.achlist");

    let (code, stdout, _stderr) = run_captured(&[
        "--json".to_string(),
        "--short-paths".to_string(),
        achlist_path.to_string_lossy().into_owned(),
    ]);

    assert_eq!(code, 0);
    let report: serde_json::Value = serde_json::from_str(&stdout).unwrap();
    assert_eq!(
        report["files"][0]["path"],
        "scripts/source/Example.psc".replace('/', std::path::MAIN_SEPARATOR_STR)
    );
}

#[test]
fn without_short_paths_reports_the_full_path() {
    let dir = tempfile::tempdir().expect("failed to create temp dir");
    write_file(
        &dir.path().join("scripts/source/Example.psc"),
        "ScriptName Example   \n",
    );
    write_file(
        &dir.path().join("sources.achlist"),
        r#"["scripts/source/Example.psc"]"#,
    );
    let achlist_path = dir.path().join("sources.achlist");

    let (code, stdout, _stderr) = run_captured(&[achlist_path.to_string_lossy().into_owned()]);

    assert_eq!(code, 0);
    assert!(stdout.contains(dir.path().to_string_lossy().as_ref()));
}

#[test]
fn honors_the_project_yaml_config() {
    let dir = tempfile::tempdir().expect("failed to create temp dir");
    write_file(
        &dir.path().join("scripts/source/Example.psc"),
        "ScriptName Example   \n",
    );
    write_file(
        &dir.path().join("sources.achlist"),
        r#"["scripts/source/Example.psc"]"#,
    );
    write_file(
        &dir.path().join("papyrus-lint.yaml"),
        "rules:\n  trailing_whitespace: false\n",
    );
    let achlist_path = dir.path().join("sources.achlist");

    let (code, stdout, _stderr) = run_captured(&[achlist_path.to_string_lossy().into_owned()]);

    assert_eq!(code, 0);
    assert!(stdout.contains("no problems found"));
}

#[test]
fn reports_an_invalid_project_yaml_config() {
    let dir = tempfile::tempdir().expect("failed to create temp dir");
    write_file(
        &dir.path().join("scripts/source/Example.psc"),
        "ScriptName Example\n",
    );
    write_file(
        &dir.path().join("sources.achlist"),
        r#"["scripts/source/Example.psc"]"#,
    );
    write_file(
        &dir.path().join("papyrus-lint.yaml"),
        "rules: definitely-not-a-rule-set\n",
    );
    let achlist_path = dir.path().join("sources.achlist");

    let (code, stdout, stderr) = run_captured(&[achlist_path.to_string_lossy().into_owned()]);

    assert_eq!(code, 2);
    assert!(stdout.is_empty());
    assert!(stderr.starts_with("error: failed to load lint config:"));
}

#[test]
fn ignores_non_psc_entries_in_the_achlist() {
    let dir = tempfile::tempdir().expect("failed to create temp dir");
    write_file(&dir.path().join("scripts/source/Example.pex"), "");
    write_file(
        &dir.path().join("sources.achlist"),
        r#"["scripts/source/Example.pex"]"#,
    );
    let achlist_path = dir.path().join("sources.achlist");

    let (code, stdout, _stderr) = run_captured(&[achlist_path.to_string_lossy().into_owned()]);

    assert_eq!(code, 0);
    assert!(stdout.contains("no problems found in 0 script"));
}

#[test]
fn recognizes_uppercase_psc_extensions_in_the_achlist() {
    let dir = tempfile::tempdir().expect("failed to create temp dir");
    write_file(
        &dir.path().join("scripts/source/Example.PSC"),
        "ScriptName Example   \n",
    );
    write_file(
        &dir.path().join("sources.achlist"),
        r#"["scripts/source/Example.PSC"]"#,
    );
    let achlist_path = dir.path().join("sources.achlist");

    let (code, stdout, stderr) = run_captured(&[achlist_path.to_string_lossy().into_owned()]);

    assert_eq!(code, 0);
    assert!(stderr.is_empty());
    assert!(stdout.contains("[trailing-whitespace]"));
    assert!(stdout.contains("1 problem(s) found in 1 of 1 script(s)"));
}

#[test]
fn lints_a_single_psc_file_passed_directly() {
    let dir = tempfile::tempdir().expect("failed to create temp dir");
    let script_path = dir.path().join("Example.psc");
    write_file(&script_path, "ScriptName Example   \n");

    let (code, stdout, _stderr) = run_captured(&[script_path.to_string_lossy().into_owned()]);

    assert_eq!(code, 0);
    assert!(stdout.contains("[trailing-whitespace]"));
    assert!(stdout.contains("1 problem(s) found in 1 of 1 script(s)"));
}

#[test]
fn reports_no_problems_for_a_clean_single_psc_file() {
    let dir = tempfile::tempdir().expect("failed to create temp dir");
    let script_path = dir.path().join("Example.psc");
    write_file(&script_path, "ScriptName Example\n");

    let (code, stdout, _stderr) = run_captured(&[script_path.to_string_lossy().into_owned()]);

    assert_eq!(code, 0);
    assert!(stdout.contains("no problems found in 1 script"));
}

#[test]
fn lints_every_psc_found_recursively_under_a_dropped_directory() {
    // Mirrors a mod like Requiem, whose scripts are spread across
    // arbitrarily nested subfolders rather than a flat scripts/source
    // directory, and ships no .achlist at all.
    let dir = tempfile::tempdir().expect("failed to create temp dir");
    write_file(
        &dir.path().join("scripts/source/Top.psc"),
        "ScriptName Top   \n",
    );
    write_file(
        &dir.path().join("scripts/source/Requiem/Sub/Nested.psc"),
        "ScriptName Nested   \n",
    );
    let target = dir.path().join("scripts/source");

    let (code, stdout, stderr) = run_captured(&[target.to_string_lossy().into_owned()]);

    assert_eq!(code, 0, "stderr: {stderr}");
    assert!(stdout.contains("2 script(s)"));
    assert!(stdout.contains("Top.psc"));
    assert!(stdout.contains("Nested.psc"));
}

#[test]
fn directory_scan_finds_the_project_root_from_a_nested_scripts_source_pair() {
    let dir = tempfile::tempdir().expect("failed to create temp dir");
    write_file(
        &dir.path().join("scripts/source/Requiem/Nested.psc"),
        "ScriptName Nested   \n",
    );
    write_file(
        &dir.path().join("papyrus-lint.yaml"),
        "rules:\n  trailing_whitespace: false\n",
    );
    let target = dir.path().join("scripts/source");

    let (code, stdout, _stderr) = run_captured(&[target.to_string_lossy().into_owned()]);

    assert_eq!(code, 0);
    assert!(stdout.contains("no problems found"));
}

#[test]
fn directory_scan_falls_back_to_the_scanned_directory_as_project_root() {
    // No scripts/source or source/scripts pair anywhere in the path, so
    // the scanned directory itself must be used as the project root.
    let dir = tempfile::tempdir().expect("failed to create temp dir");
    write_file(
        &dir.path().join("Nested/Example.psc"),
        "ScriptName Example   \n",
    );
    write_file(
        &dir.path().join("papyrus-lint.yaml"),
        "rules:\n  trailing_whitespace: false\n",
    );

    let (code, stdout, _stderr) = run_captured(&[dir.path().to_string_lossy().into_owned()]);

    assert_eq!(code, 0);
    assert!(stdout.contains("no problems found"));
}

#[test]
fn directory_scan_reports_no_problems_for_an_empty_directory() {
    let dir = tempfile::tempdir().expect("failed to create temp dir");

    let (code, stdout, _stderr) = run_captured(&[dir.path().to_string_lossy().into_owned()]);

    assert_eq!(code, 0);
    assert!(stdout.contains("no problems found in 0 script"));
}

#[test]
fn directory_scan_resolves_cross_script_calls_across_nested_subfolders() {
    let dir = tempfile::tempdir().expect("failed to create temp dir");
    write_file(
        &dir.path().join("scripts/source/Greeter.psc"),
        "ScriptName Greeter\n\nFunction Greet(String name)\nEndFunction\n",
    );
    write_file(
        &dir.path().join("scripts/source/Requiem/Example.psc"),
        "ScriptName Example\n\nGreeter Property Target Auto\n\nFunction Test()\n    Target.Greet(1)\nEndFunction\n",
    );
    let target = dir.path().join("scripts/source");

    let (code, stdout, _stderr) = run_captured(&[target.to_string_lossy().into_owned()]);

    assert_eq!(code, 1);
    assert!(stdout.contains("[argument-types]"));
}

#[test]
fn honors_the_project_yaml_config_two_directories_above_a_single_psc_file() {
    // Mirrors the real layout a bare .psc file is found at (e.g. an
    // editor plugin invoking the CLI on a saved file), where the
    // project root sits two directories above the script, at
    // `<root>/scripts/source/Example.psc`.
    let dir = tempfile::tempdir().expect("failed to create temp dir");
    let script_path = dir.path().join("scripts/source/Example.psc");
    write_file(&script_path, "ScriptName Example   \n");
    write_file(
        &dir.path().join("papyrus-lint.yaml"),
        "rules:\n  trailing_whitespace: false\n",
    );

    let (code, stdout, _stderr) = run_captured(&[script_path.to_string_lossy().into_owned()]);

    assert_eq!(code, 0);
    assert!(stdout.contains("no problems found"));
}

#[test]
fn finds_the_project_root_for_a_psc_nested_under_a_namespaced_subfolder() {
    // A Fallout 4-style namespaced script, e.g. `ScriptName User:MyScript`
    // stored at `Scripts/Source/User/MyScript.psc`, sits three
    // directories under the project root rather than the conventional
    // two. A naive "two directories up" rule would land on `Scripts`
    // instead of the real root, missing the project's config and
    // breaking every cross-script lookup — this must still find the
    // real root and pick up the config there.
    let dir = tempfile::tempdir().expect("failed to create temp dir");
    let script_path = dir.path().join("scripts/source/User/MyScript.psc");
    write_file(&script_path, "ScriptName User:MyScript   \n");
    write_file(
        &dir.path().join("papyrus-lint.yaml"),
        "rules:\n  trailing_whitespace: false\n",
    );

    let (code, stdout, _stderr) = run_captured(&[script_path.to_string_lossy().into_owned()]);

    assert_eq!(code, 0);
    assert!(stdout.contains("no problems found"));
}

#[test]
fn finds_the_project_root_from_script_position_when_the_achlist_lives_elsewhere() {
    // Users sometimes drop the .achlist somewhere other than the
    // project root (e.g. next to a game's Data directory) while the
    // actual project, including its papyrus-lint.yaml, lives deeper:
    //
    //   achlist
    //   somefolder/
    //     otherfolder/
    //       papyrus-lint.yaml
    //       scripts/source/AType.psc
    //       source/scripts/BType.psc
    //
    // The achlist's own parent directory (the top-level one) has no
    // config file at all, so the project root must instead be found
    // from the resolved scripts' own position under their
    // scripts/source or source/scripts pair.
    let dir = tempfile::tempdir().expect("failed to create temp dir");
    let project_root = dir.path().join("somefolder/otherfolder");
    write_file(
        &project_root.join("scripts/source/AType.psc"),
        "ScriptName AType   \n",
    );
    write_file(
        &project_root.join("source/scripts/BType.psc"),
        "ScriptName BType   \n",
    );
    write_file(
        &project_root.join("papyrus-lint.yaml"),
        "rules:\n  trailing_whitespace: false\n",
    );
    write_file(
        &dir.path().join("achlist"),
        r#"["somefolder/otherfolder/scripts/source/AType.psc", "somefolder/otherfolder/source/scripts/BType.psc"]"#,
    );
    let achlist_path = dir.path().join("achlist");

    let (code, stdout, _stderr) = run_captured(&[achlist_path.to_string_lossy().into_owned()]);

    // If the project root were (wrongly) taken as the achlist's own
    // parent directory, papyrus-lint.yaml wouldn't be found and the
    // trailing-whitespace lint (disabled by that config) would fire on
    // both scripts instead.
    assert_eq!(code, 0);
    assert!(stdout.contains("no problems found in 2 script"));
}

#[test]
fn same_script_lints_identically_via_achlist_and_directly_when_namespaced() {
    // The same nested script should produce the same diagnostics
    // whether it's resolved from an .achlist or linted directly.
    let dir = tempfile::tempdir().expect("failed to create temp dir");
    write_file(
        &dir.path().join("scripts/source/Greeter.psc"),
        "ScriptName Greeter\n\nFunction Greet(String name)\nEndFunction\n",
    );
    let script_path = dir.path().join("scripts/source/User/Example.psc");
    write_file(
        &script_path,
        "ScriptName Example\n\nGreeter Property Target Auto\n\nFunction Test()\n    Target.Greet(1)\nEndFunction\n",
    );
    write_file(
        &dir.path().join("sources.achlist"),
        r#"["scripts/source/Greeter.psc", "scripts/source/User/Example.psc"]"#,
    );
    let achlist_path = dir.path().join("sources.achlist");

    let (achlist_code, achlist_stdout, _) =
        run_captured(&[achlist_path.to_string_lossy().into_owned()]);
    let (direct_code, direct_stdout, _) =
        run_captured(&[script_path.to_string_lossy().into_owned()]);

    assert_eq!(achlist_code, 1);
    assert_eq!(direct_code, 1);
    assert!(achlist_stdout.contains("[argument-types]"));
    assert!(direct_stdout.contains("[argument-types]"));
}

#[test]
fn decodes_a_cp1252_encoded_script_instead_of_failing_the_whole_run() {
    let dir = tempfile::tempdir().expect("failed to create temp dir");
    let script_path = dir.path().join("scripts/source/Example.psc");
    fs::create_dir_all(script_path.parent().unwrap()).expect("failed to create parent dir");
    // "; caf\xE9" in Windows-1252 (0xE9 is "é"), which is not valid
    // UTF-8 on its own.
    let mut contents = b"ScriptName Example\n\n; caf".to_vec();
    contents.push(0xE9);
    contents.push(b'\n');
    fs::write(&script_path, &contents).expect("failed to write test file");
    write_file(
        &dir.path().join("sources.achlist"),
        r#"["scripts/source/Example.psc"]"#,
    );
    let achlist_path = dir.path().join("sources.achlist");

    let (code, stdout, stderr) = run_captured(&[achlist_path.to_string_lossy().into_owned()]);

    assert_eq!(code, 0, "stderr: {stderr}");
    assert!(stdout.contains("no problems found in 1 script"));
}

#[test]
fn errors_when_the_given_psc_file_is_missing() {
    let dir = tempfile::tempdir().expect("failed to create temp dir");
    let script_path = dir.path().join("Missing.psc");

    let (code, _stdout, stderr) = run_captured(&[script_path.to_string_lossy().into_owned()]);

    assert_eq!(code, 2);
    assert!(stderr.starts_with("error:"));
}

#[test]
fn fix_rewrites_fixable_issues_and_reports_the_rest() {
    let dir = tempfile::tempdir().expect("failed to create temp dir");
    write_file(
        &dir.path().join("scripts/source/Example.psc"),
        "ScriptName Example   \n\nFunction DoThing()\n\tGame.GetPlayer()\nEndFunction\n",
    );
    write_file(
        &dir.path().join("sources.achlist"),
        r#"["scripts/source/Example.psc"]"#,
    );
    let achlist_path = dir.path().join("sources.achlist");

    let (code, stdout, _stderr) = run_captured(&[
        "fix".to_string(),
        achlist_path.to_string_lossy().into_owned(),
    ]);

    assert_eq!(
        fs::read_to_string(dir.path().join("scripts/source/Example.psc")).unwrap(),
        "ScriptName Example\n\nFunction DoThing()\n\tGame.GetPlayer()\nEndFunction\n"
    );
    assert_eq!(code, 1);
    assert!(!stdout.contains("[trailing-whitespace]"));
    assert!(stdout.contains("Game.GetPlayer"));
    assert!(stdout.contains("(1 script(s) fixed.)"));
}

#[test]
fn fix_preserves_a_cp1252_encoded_files_encoding() {
    let dir = tempfile::tempdir().expect("failed to create temp dir");
    let script_path = dir.path().join("scripts/source/Example.psc");
    fs::create_dir_all(script_path.parent().unwrap()).expect("failed to create parent dir");
    // "ScriptName Example   \n\n; caf\xE9\n" with trailing whitespace on
    // the first line to fix, and 0xE9 ("é" in Windows-1252) making the
    // file as a whole invalid UTF-8.
    let mut contents = b"ScriptName Example   \n\n; caf".to_vec();
    contents.push(0xE9);
    contents.push(b'\n');
    fs::write(&script_path, &contents).expect("failed to write test file");
    write_file(
        &dir.path().join("sources.achlist"),
        r#"["scripts/source/Example.psc"]"#,
    );
    let achlist_path = dir.path().join("sources.achlist");

    let (code, stdout, stderr) = run_captured(&[
        "fix".to_string(),
        achlist_path.to_string_lossy().into_owned(),
    ]);

    assert!(stdout.contains("(1 script(s) fixed.)"), "stderr: {stderr}");
    let mut expected = b"ScriptName Example\n\n; caf".to_vec();
    expected.push(0xE9);
    expected.push(b'\n');
    assert_eq!(
        fs::read(&script_path).expect("failed to read back fixed file"),
        expected,
        "fixing must preserve the file's original Windows-1252 encoding"
    );
    assert_eq!(code, 0);
}

#[test]
fn fix_type_filter_applies_only_the_named_rule() {
    let dir = tempfile::tempdir().expect("failed to create temp dir");
    let script_path = dir.path().join("Example.psc");
    write_file(
        &script_path,
        "ScriptName Example\n\nFunction Add(Int left,Int right)\nEndFunction   \n",
    );

    let (code, _stdout, stderr) = run_captured(&[
        "fix".to_string(),
        "--type=comma_spacing".to_string(),
        script_path.to_string_lossy().into_owned(),
    ]);

    assert!(stderr.is_empty());
    assert_eq!(code, 0);
    assert_eq!(
        fs::read_to_string(&script_path).unwrap(),
        "ScriptName Example\n\nFunction Add(Int left, Int right)\nEndFunction   \n"
    );
}

#[test]
fn fix_type_filter_accepts_the_hyphenated_rule_id() {
    let dir = tempfile::tempdir().expect("failed to create temp dir");
    let script_path = dir.path().join("Example.psc");
    write_file(&script_path, "ScriptName Example   \n");

    let (code, _stdout, stderr) = run_captured(&[
        "fix".to_string(),
        "--type".to_string(),
        "trailing-whitespace".to_string(),
        script_path.to_string_lossy().into_owned(),
    ]);

    assert!(stderr.is_empty());
    assert_eq!(code, 0);
    assert_eq!(
        fs::read_to_string(&script_path).unwrap(),
        "ScriptName Example\n"
    );
}

#[test]
fn fix_line_filter_only_touches_the_named_line() {
    let dir = tempfile::tempdir().expect("failed to create temp dir");
    let script_path = dir.path().join("Example.psc");
    write_file(
        &script_path,
        "ScriptName Example   \n\nFunction DoThing()   \nEndFunction\n",
    );

    let (code, _stdout, stderr) = run_captured(&[
        "fix".to_string(),
        "--line=3".to_string(),
        script_path.to_string_lossy().into_owned(),
    ]);

    assert!(stderr.is_empty());
    assert_eq!(code, 0);
    assert_eq!(
        fs::read_to_string(&script_path).unwrap(),
        "ScriptName Example   \n\nFunction DoThing()\nEndFunction\n"
    );
}

#[test]
fn fix_line_and_type_filters_combine() {
    let dir = tempfile::tempdir().expect("failed to create temp dir");
    let script_path = dir.path().join("Example.psc");
    write_file(
        &script_path,
        "ScriptName Example   \n\nFunction Add(Int left,Int right)   \nEndFunction\n",
    );

    let (code, _stdout, stderr) = run_captured(&[
        "fix".to_string(),
        "--line=3".to_string(),
        "--type=comma_spacing".to_string(),
        script_path.to_string_lossy().into_owned(),
    ]);

    assert!(stderr.is_empty());
    assert_eq!(code, 0);
    assert_eq!(
        fs::read_to_string(&script_path).unwrap(),
        "ScriptName Example   \n\nFunction Add(Int left, Int right)   \nEndFunction\n"
    );
}

#[test]
fn type_filter_rejects_an_unknown_rule_id() {
    let dir = tempfile::tempdir().expect("failed to create temp dir");
    let script_path = dir.path().join("Example.psc");
    write_file(&script_path, "ScriptName Example\n");

    let (code, _stdout, stderr) = run_captured(&[
        "fix".to_string(),
        "--type=made-up-rule".to_string(),
        script_path.to_string_lossy().into_owned(),
    ]);

    assert_eq!(code, 2);
    assert!(stderr.contains("unknown rule 'made-up-rule'"));
}

#[test]
fn type_filter_rejects_a_rule_with_no_automatic_fix() {
    let dir = tempfile::tempdir().expect("failed to create temp dir");
    let script_path = dir.path().join("Example.psc");
    write_file(&script_path, "ScriptName Example\n");

    let (code, _stdout, stderr) = run_captured(&[
        "fix".to_string(),
        "--type=forbidden-functions".to_string(),
        script_path.to_string_lossy().into_owned(),
    ]);

    assert_eq!(code, 2);
    assert!(stderr.contains("rule 'forbidden-functions' has no automatic fix"));
}

#[test]
fn type_filter_without_fix_prints_usage() {
    let dir = tempfile::tempdir().expect("failed to create temp dir");
    let script_path = dir.path().join("Example.psc");
    write_file(&script_path, "ScriptName Example\n");

    let (code, _stdout, stderr) = run_captured(&[
        "--type=trailing-whitespace".to_string(),
        script_path.to_string_lossy().into_owned(),
    ]);

    assert_eq!(code, 2);
    assert!(stderr.contains("Usage: PapyrusLinterCLI"));
}

#[test]
fn tag_filter_restricts_reported_diagnostics_to_the_matching_kind() {
    let dir = tempfile::tempdir().expect("failed to create temp dir");
    let script_path = dir.path().join("Example.psc");
    write_file(
        &script_path,
        "ScriptName Example   \n\nFunction DoThing()\n\tGame.GetPlayer()\nEndFunction\n",
    );

    let (code, stdout, stderr) = run_captured(&[
        "--tag=style".to_string(),
        script_path.to_string_lossy().into_owned(),
    ]);

    assert!(stderr.is_empty());
    assert_eq!(code, 0);
    assert!(stdout.contains("[trailing-whitespace]"));
    assert!(!stdout.contains("[forbidden-functions]"));
}

#[test]
fn tag_filter_matches_case_insensitively() {
    let dir = tempfile::tempdir().expect("failed to create temp dir");
    let script_path = dir.path().join("Example.psc");
    write_file(&script_path, "ScriptName Example   \n");

    let (code, stdout, stderr) = run_captured(&[
        "--tag=STYLE".to_string(),
        script_path.to_string_lossy().into_owned(),
    ]);

    assert!(stderr.is_empty());
    assert_eq!(code, 0);
    assert!(stdout.contains("[trailing-whitespace]"));
}

#[test]
fn tag_filter_works_without_fix() {
    let dir = tempfile::tempdir().expect("failed to create temp dir");
    let script_path = dir.path().join("Example.psc");
    write_file(
        &script_path,
        "ScriptName Example\n\nFunction DoThing()\n\tGame.GetPlayer()\nEndFunction\n",
    );

    let (code, _stdout, stderr) = run_captured(&[
        "--tag=performance".to_string(),
        script_path.to_string_lossy().into_owned(),
    ]);

    assert!(stderr.is_empty());
    assert_eq!(code, 1);
}

#[test]
fn fix_tag_filter_applies_only_fixes_in_that_kind() {
    let dir = tempfile::tempdir().expect("failed to create temp dir");
    let script_path = dir.path().join("Example.psc");
    write_file(
        &script_path,
        "ScriptName Example\n\nFunction DoThing(GlobalVariable akGlobal)\n    akGlobal.GetValueInt()  \nEndFunction\n",
    );

    let (code, _stdout, stderr) = run_captured(&[
        "fix".to_string(),
        "--tag=performance".to_string(),
        script_path.to_string_lossy().into_owned(),
    ]);

    assert!(stderr.is_empty());
    assert_eq!(code, 0);
    let fixed = fs::read_to_string(&script_path).unwrap();
    assert!(!fixed.contains("GetValueInt"));
    // trailing-whitespace is tagged "style", not "performance", so it's
    // left in place by --tag=performance.
    assert!(fixed.contains("  \n"));
}

#[test]
fn tag_and_type_filters_cannot_be_combined() {
    let dir = tempfile::tempdir().expect("failed to create temp dir");
    let script_path = dir.path().join("Example.psc");
    write_file(&script_path, "ScriptName Example\n");

    let (code, _stdout, stderr) = run_captured(&[
        "fix".to_string(),
        "--type=trailing-whitespace".to_string(),
        "--tag=style".to_string(),
        script_path.to_string_lossy().into_owned(),
    ]);

    assert_eq!(code, 2);
    assert!(stderr.contains("--type and --tag can't be combined"));
}

#[test]
fn tag_filter_rejects_an_unknown_tag() {
    let dir = tempfile::tempdir().expect("failed to create temp dir");
    let script_path = dir.path().join("Example.psc");
    write_file(&script_path, "ScriptName Example\n");

    let (code, _stdout, stderr) = run_captured(&[
        "--tag=made-up-tag".to_string(),
        script_path.to_string_lossy().into_owned(),
    ]);

    assert_eq!(code, 2);
    assert!(stderr.contains("unknown tag 'made-up-tag'"));
}

#[test]
fn blob_lints_raw_source_text_without_a_file() {
    let (code, stdout, stderr) =
        run_captured(&["--blob".to_string(), "ScriptName Example   \n".to_string()]);

    assert!(stderr.is_empty());
    assert_eq!(code, 0);
    assert!(stdout.contains("<blob>:1:"));
    assert!(stdout.contains("[trailing-whitespace]"));
    assert!(stdout.contains("problem(s) found in the given blob"));
}

#[test]
fn blob_with_no_diagnostics_reports_success() {
    let (code, stdout, stderr) = run_captured(&["--blob=ScriptName Example\n".to_string()]);

    assert!(stderr.is_empty());
    assert_eq!(code, 0);
    assert!(stdout.contains("no problems found in the given blob"));
}

#[test]
fn blob_reports_errors_as_a_failure() {
    let (code, stdout, stderr) = run_captured(&[
        "--blob".to_string(),
        "ScriptName Example\n\nFunction DoThing()\n\tGame.GetPlayer()\nEndFunction\n".to_string(),
    ]);

    assert!(stderr.is_empty());
    assert_eq!(code, 1);
    assert!(stdout.contains("[forbidden-functions]"));
}

#[test]
fn blob_json_report_uses_the_literal_blob_path() {
    let (code, stdout, stderr) = run_captured(&[
        "--json".to_string(),
        "--blob".to_string(),
        "ScriptName Example   \n".to_string(),
    ]);

    assert!(stderr.is_empty());
    assert_eq!(code, 0);
    let report: serde_json::Value = serde_json::from_str(&stdout).unwrap();
    assert_eq!(report["scripts_checked"], 1);
    assert_eq!(report["files"][0]["path"], "<blob>");
    assert_eq!(
        report["files"][0]["diagnostics"][0]["rule"],
        "trailing-whitespace"
    );
}

#[test]
fn blob_honors_tag_filter() {
    let (code, stdout, stderr) = run_captured(&[
        "--tag=performance".to_string(),
        "--blob".to_string(),
        "ScriptName Example   \n\nFunction DoThing()\n\tGame.GetPlayer()\nEndFunction\n"
            .to_string(),
    ]);

    assert!(stderr.is_empty());
    assert_eq!(code, 1);
    assert!(stdout.contains("[forbidden-functions]"));
    assert!(!stdout.contains("[trailing-whitespace]"));
}

#[test]
fn blob_honors_an_explicit_config_file() {
    let dir = tempfile::tempdir().expect("failed to create temp dir");
    let config_path = dir.path().join("papyrus-lint.yaml");
    write_file(&config_path, "rules:\n  trailing_whitespace: false\n");

    let (code, stdout, stderr) = run_captured(&[
        "--config".to_string(),
        config_path.to_string_lossy().into_owned(),
        "--blob".to_string(),
        "ScriptName Example   \n".to_string(),
    ]);

    assert!(stderr.is_empty());
    assert_eq!(code, 0);
    assert!(!stdout.contains("[trailing-whitespace]"));
}

#[test]
fn blob_reports_a_missing_explicit_config_file() {
    let dir = tempfile::tempdir().expect("failed to create temp dir");
    let missing_config = dir.path().join("missing.yaml");

    let (code, stdout, stderr) = run_captured(&[
        "--blob".to_string(),
        "ScriptName Example\n".to_string(),
        "--config".to_string(),
        missing_config.to_string_lossy().into_owned(),
    ]);

    assert_eq!(code, 2);
    assert!(stdout.is_empty());
    assert!(stderr.contains("error: failed to load lint config:"));
    assert!(stderr.contains("No such file or directory"));
}

#[test]
fn blob_ai_format_reports_content_and_rule_metadata() {
    let source = "ScriptName Example   \n";

    let (code, stdout, stderr) = run_captured(&[
        "--format=ai".to_string(),
        "--blob".to_string(),
        source.to_string(),
    ]);

    assert_eq!(code, 0, "stderr: {stderr}");
    assert!(stderr.is_empty());
    let report: serde_json::Value =
        serde_json::from_str(&stdout).expect("AI report should be valid JSON");
    assert_eq!(report["findings"]["total_diagnostics"], 1);
    assert_eq!(report["findings"]["files"][0]["path"], "<blob>");
    assert_eq!(report["findings"]["files"][0]["source"]["content"], source);
    assert_eq!(report["findings"]["rule_counts"]["trailing-whitespace"], 1);
    assert_eq!(report["rule_details"][0]["rule"], "trailing-whitespace");
    assert!(report["configuration"]["enabled_rules"].is_array());
}

#[test]
fn blob_ai_hash_source_omits_the_source_content() {
    let source = "ScriptName Example   \n";

    let (code, stdout, stderr) = run_captured(&[
        "--blob".to_string(),
        source.to_string(),
        "--format".to_string(),
        "ai".to_string(),
        "--hash-source".to_string(),
    ]);

    assert_eq!(code, 0, "stderr: {stderr}");
    assert!(stderr.is_empty());
    let report: serde_json::Value =
        serde_json::from_str(&stdout).expect("AI report should be valid JSON");
    let source_report = &report["findings"]["files"][0]["source"];
    assert_eq!(source_report["type"], "hash");
    assert_eq!(source_report["algorithm"], "md5");
    assert_eq!(source_report["hash"], content_hash::md5_hex(source));
    assert!(source_report.get("content").is_none());
    assert!(!stdout.contains(source));
}

#[test]
fn blob_ai_format_omits_clean_files_from_findings() {
    let (code, stdout, stderr) = run_captured(&[
        "--blob".to_string(),
        "ScriptName Example\n".to_string(),
        "--format=ai".to_string(),
    ]);

    assert_eq!(code, 0, "stderr: {stderr}");
    assert!(stderr.is_empty());
    let report: serde_json::Value =
        serde_json::from_str(&stdout).expect("AI report should be valid JSON");
    assert_eq!(report["findings"]["total_diagnostics"], 0);
    assert_eq!(report["findings"]["files"], serde_json::json!([]));
    assert_eq!(report["rule_details"], serde_json::json!([]));
}

#[test]
fn blob_reports_an_error_when_output_cannot_be_written() {
    let dir = tempfile::tempdir().expect("failed to create temp dir");

    let (code, stdout, stderr) = run_captured(&[
        "--blob".to_string(),
        "ScriptName Example\n".to_string(),
        "--output".to_string(),
        dir.path().to_string_lossy().into_owned(),
    ]);

    assert_eq!(code, 2);
    assert!(stdout.is_empty());
    assert!(stderr.contains("error: failed to write"));
    assert!(stderr.contains(&dir.path().display().to_string()));
}

#[test]
fn blob_rejects_an_unknown_tag() {
    let (code, _stdout, stderr) = run_captured(&[
        "--tag=made-up-tag".to_string(),
        "--blob".to_string(),
        "ScriptName Example\n".to_string(),
    ]);

    assert_eq!(code, 2);
    assert!(stderr.contains("unknown tag 'made-up-tag'"));
}

#[test]
fn blob_cannot_be_combined_with_a_path_argument() {
    let (code, _stdout, stderr) = run_captured(&[
        "--blob".to_string(),
        "ScriptName Example\n".to_string(),
        "some/path.psc".to_string(),
    ]);

    assert_eq!(code, 2);
    assert!(stderr.contains("--blob can't be combined"));
}

#[test]
fn blob_cannot_be_combined_with_fix() {
    let (code, _stdout, stderr) = run_captured(&[
        "--blob".to_string(),
        "ScriptName Example\n".to_string(),
        "fix".to_string(),
    ]);

    assert_eq!(code, 2);
    assert!(stderr.contains("--blob can't be combined"));
}

#[test]
fn blob_cannot_be_combined_with_dry_run_type_or_line() {
    for flag in ["--dry-run", "--type=trailing-whitespace", "--line=1"] {
        let (code, _stdout, stderr) = run_captured(&[
            flag.to_string(),
            "--blob".to_string(),
            "ScriptName Example\n".to_string(),
        ]);

        assert_eq!(code, 2, "flag {flag} should have been rejected");
        assert!(stderr.contains("--blob can't be combined with fix/--type/--line/--dry-run"));
    }
}

#[test]
fn blob_cannot_be_combined_with_script_root_progress_or_threads() {
    for args in [
        vec!["--script-root".to_string(), "other".to_string()],
        vec![
            "--progress".to_string(),
            "--output".to_string(),
            "out.txt".to_string(),
        ],
        vec!["--threads=2".to_string()],
    ] {
        let mut full_args = args;
        full_args.push("--blob".to_string());
        full_args.push("ScriptName Example\n".to_string());

        let (code, _stdout, stderr) = run_captured(&full_args);

        assert_eq!(code, 2, "args {full_args:?} should have been rejected");
        assert!(stderr.contains("--blob can't be combined with --script-root/--progress/--threads"));
    }
}

#[test]
fn blob_output_flag_redirects_the_report_to_a_file() {
    let dir = tempfile::tempdir().expect("failed to create temp dir");
    let output_path = dir.path().join("report.txt");

    let (code, stdout, stderr) = run_captured(&[
        "--output".to_string(),
        output_path.to_string_lossy().into_owned(),
        "--blob".to_string(),
        "ScriptName Example   \n".to_string(),
    ]);

    assert!(stderr.is_empty());
    assert_eq!(code, 0);
    assert!(stdout.is_empty());
    let report = fs::read_to_string(&output_path).unwrap();
    assert!(report.contains("[trailing-whitespace]"));
}

#[test]
fn line_filter_without_fix_prints_usage() {
    let dir = tempfile::tempdir().expect("failed to create temp dir");
    let script_path = dir.path().join("Example.psc");
    write_file(&script_path, "ScriptName Example\n");

    let (code, _stdout, stderr) = run_captured(&[
        "--line=1".to_string(),
        script_path.to_string_lossy().into_owned(),
    ]);

    assert_eq!(code, 2);
    assert!(stderr.contains("Usage: PapyrusLinterCLI"));
}

#[test]
fn line_filter_rejects_a_non_positive_value() {
    let dir = tempfile::tempdir().expect("failed to create temp dir");
    let script_path = dir.path().join("Example.psc");
    write_file(&script_path, "ScriptName Example\n");

    let (code, _stdout, stderr) = run_captured(&[
        "fix".to_string(),
        "--line=0".to_string(),
        script_path.to_string_lossy().into_owned(),
    ]);

    assert_eq!(code, 2);
    assert!(stderr.contains("--line must be a positive integer"));
}

#[test]
fn threads_flag_rejects_a_non_positive_value() {
    let dir = tempfile::tempdir().expect("failed to create temp dir");
    let script_path = dir.path().join("Example.psc");
    write_file(&script_path, "ScriptName Example\n");

    let (code, _stdout, stderr) = run_captured(&[
        "--threads=0".to_string(),
        script_path.to_string_lossy().into_owned(),
    ]);

    assert_eq!(code, 2);
    assert!(stderr.contains("--threads must be a positive integer"));
}

#[test]
fn threads_flag_rejects_a_non_numeric_value() {
    let dir = tempfile::tempdir().expect("failed to create temp dir");
    let script_path = dir.path().join("Example.psc");
    write_file(&script_path, "ScriptName Example\n");

    let (code, _stdout, stderr) = run_captured(&[
        "--threads".to_string(),
        "many".to_string(),
        script_path.to_string_lossy().into_owned(),
    ]);

    assert_eq!(code, 2);
    assert!(stderr.contains("--threads must be a positive integer"));
}

#[test]
fn threads_flag_without_a_value_prints_usage() {
    let (code, _stdout, stderr) = run_captured(&["--threads".to_string()]);

    assert_eq!(code, 2);
    assert!(stderr.contains("Usage: PapyrusLinterCLI"));
}

// Builds an achlist with enough scripts, several of them cross-referencing
// a shared base script, that a multi-threaded run actually exercises
// more than one worker thread and more than one `SharedFunctionTable`
// lookup collision -- then checks a `--threads 1` (fully sequential) run
// and the default multi-threaded run agree byte-for-byte, since threading
// is only ever meant to change how fast a run finishes, never what it
// reports or in what order.
#[test]
fn threaded_and_sequential_runs_report_identical_results() {
    let dir = tempfile::tempdir().expect("failed to create temp dir");
    write_file(
        &dir.path().join("scripts/source/Base.psc"),
        "ScriptName Base\n\nFunction DoThing(int arg1)\nEndFunction\n",
    );
    let mut entries = vec!["\"scripts/source/Base.psc\"".to_string()];
    for i in 0..12 {
        let name = format!("Child{i}");
        write_file(
            &dir.path().join(format!("scripts/source/{name}.psc")),
            &format!(
                "ScriptName {name} Extends Base\n\nFunction UseIt()\n    DoThing(\"wrong type\")   \nEndFunction\n"
            ),
        );
        entries.push(format!("\"scripts/source/{name}.psc\""));
    }
    write_file(
        &dir.path().join("sources.achlist"),
        &format!("[{}]", entries.join(", ")),
    );
    let achlist_path = dir.path().join("sources.achlist");

    let (sequential_code, sequential_stdout, _) = run_captured(&[
        "--threads=1".to_string(),
        "--short-paths".to_string(),
        "--json".to_string(),
        achlist_path.to_string_lossy().into_owned(),
    ]);
    let (parallel_code, parallel_stdout, _) = run_captured(&[
        "--threads=8".to_string(),
        "--short-paths".to_string(),
        "--json".to_string(),
        achlist_path.to_string_lossy().into_owned(),
    ]);

    assert_eq!(sequential_code, parallel_code);
    assert_eq!(sequential_stdout, parallel_stdout);
    // Sanity check that this fixture actually triggers diagnostics
    // (the argument-type mismatch on every child script), rather than
    // both runs trivially agreeing on an empty report.
    assert!(sequential_stdout.contains("argument-types"));
}

#[test]
fn line_filter_errors_when_a_fix_changes_the_line_count() {
    let dir = tempfile::tempdir().expect("failed to create temp dir");
    let script_path = dir.path().join("scripts/source/Example.psc");
    write_file(
        &script_path,
        "ScriptName Example\n\nInt Property Zulu = 1 Auto\nActor Property Alpha Auto\n",
    );
    write_file(
        &dir.path().join("papyrus-lint.yaml"),
        "rules:\n  property_sorting: true\n",
    );

    let (code, _stdout, stderr) = run_captured(&[
        "fix".to_string(),
        "--line=3".to_string(),
        script_path.to_string_lossy().into_owned(),
    ]);

    assert_eq!(code, 2);
    assert!(stderr.contains("changes the file's line count"));
    assert_eq!(
        fs::read_to_string(&script_path).unwrap(),
        "ScriptName Example\n\nInt Property Zulu = 1 Auto\nActor Property Alpha Auto\n"
    );
}

#[test]
fn fix_does_not_rewrite_an_already_clean_file() {
    let dir = tempfile::tempdir().expect("failed to create temp dir");
    let script_path = dir.path().join("Example.psc");
    write_file(&script_path, "ScriptName Example\n");

    let (code, stdout, _stderr) = run_captured(&[
        "fix".to_string(),
        script_path.to_string_lossy().into_owned(),
    ]);

    assert_eq!(code, 0);
    assert_eq!(
        fs::read_to_string(&script_path).unwrap(),
        "ScriptName Example\n"
    );
    assert!(stdout.contains("(0 script(s) fixed.)"));
}

#[test]
fn fix_dry_run_prints_a_diff_without_writing_the_file() {
    let dir = tempfile::tempdir().expect("failed to create temp dir");
    let script_path = dir.path().join("Example.psc");
    let original = "ScriptName Example   \n\nFunction DoThing()\n\tGame.GetPlayer()\nEndFunction\n";
    write_file(&script_path, original);

    let (code, stdout, stderr) = run_captured(&[
        "fix".to_string(),
        "--dry-run".to_string(),
        script_path.to_string_lossy().into_owned(),
    ]);

    assert_eq!(code, 1, "stderr: {stderr}");
    assert_eq!(
        fs::read_to_string(&script_path).unwrap(),
        original,
        "--dry-run must never write to the file"
    );
    let expected_path = script_path.to_string_lossy();
    assert!(stdout.contains(&format!("--- {expected_path}\n")));
    assert!(stdout.contains(&format!("+++ {expected_path}\n")));
    assert!(stdout.contains("-ScriptName Example   \n"));
    assert!(stdout.contains("+ScriptName Example\n"));
    assert!(stdout.contains("(1 script(s) would be fixed.)"));
    assert!(stdout.contains("Game.GetPlayer"));
}

#[test]
fn fix_dry_run_prints_nothing_extra_for_an_already_clean_file() {
    let dir = tempfile::tempdir().expect("failed to create temp dir");
    let script_path = dir.path().join("Example.psc");
    write_file(&script_path, "ScriptName Example\n");

    let (code, stdout, stderr) = run_captured(&[
        "fix".to_string(),
        "--dry-run".to_string(),
        script_path.to_string_lossy().into_owned(),
    ]);

    assert_eq!(code, 0, "stderr: {stderr}");
    assert_eq!(
        fs::read_to_string(&script_path).unwrap(),
        "ScriptName Example\n"
    );
    assert!(!stdout.contains("---"));
    assert!(stdout.contains("(0 script(s) would be fixed.)"));
}

#[test]
fn dry_run_without_fix_is_a_usage_error() {
    let dir = tempfile::tempdir().expect("failed to create temp dir");
    let script_path = dir.path().join("Example.psc");
    write_file(&script_path, "ScriptName Example   \n");

    let (code, _stdout, stderr) = run_captured(&[
        "--dry-run".to_string(),
        script_path.to_string_lossy().into_owned(),
    ]);

    assert_eq!(code, 2);
    assert!(stderr.contains("Usage: PapyrusLinterCLI"));
    assert_eq!(
        fs::read_to_string(&script_path).unwrap(),
        "ScriptName Example   \n"
    );
}

#[test]
fn prints_usage_when_fix_is_given_without_a_path() {
    let (code, _stdout, stderr) = run_captured(&["fix".to_string()]);

    assert_eq!(code, 2);
    assert!(stderr.contains("Usage: PapyrusLinterCLI"));
}

#[test]
fn fix_errors_when_the_achlist_is_missing() {
    let dir = tempfile::tempdir().expect("failed to create temp dir");
    let achlist_path = dir.path().join("missing.achlist");

    let (code, _stdout, stderr) = run_captured(&[
        "fix".to_string(),
        achlist_path.to_string_lossy().into_owned(),
    ]);

    assert_eq!(code, 2);
    assert!(stderr.starts_with("error:"));
}

#[test]
fn fix_removes_an_unused_import_resolved_through_the_project_root() {
    let dir = tempfile::tempdir().expect("failed to create temp dir");
    write_file(
        &dir.path().join("scripts/source/Helpers.psc"),
        "ScriptName Helpers\n\nGlobal Function Assist()\nEndFunction\n",
    );
    let script_path = dir.path().join("scripts/source/Example.psc");
    write_file(
        &script_path,
        "ScriptName Example\n\nImport Helpers\n\nFunction Test()\nEndFunction\n",
    );
    write_file(
        &dir.path().join("sources.achlist"),
        r#"["scripts/source/Helpers.psc", "scripts/source/Example.psc"]"#,
    );
    let achlist_path = dir.path().join("sources.achlist");

    let (code, stdout, stderr) = run_captured(&[
        "fix".to_string(),
        achlist_path.to_string_lossy().into_owned(),
    ]);

    assert_eq!(code, 0, "stdout: {stdout}, stderr: {stderr}");
    assert_eq!(
        fs::read_to_string(&script_path).unwrap(),
        "ScriptName Example\n\n\nFunction Test()\nEndFunction\n"
    );
    assert!(stdout.contains("(1 script(s) fixed.)"));
}

#[test]
fn fix_type_filter_for_unused_import_only_touches_that_rule() {
    let dir = tempfile::tempdir().expect("failed to create temp dir");
    write_file(
        &dir.path().join("scripts/source/Helpers.psc"),
        "ScriptName Helpers\n\nGlobal Function Assist()\nEndFunction\n",
    );
    let script_path = dir.path().join("scripts/source/Example.psc");
    write_file(
        &script_path,
        "ScriptName Example\n\nImport Helpers\n\nFunction Test()\n    Call(1,2)\nEndFunction\n",
    );
    write_file(
        &dir.path().join("sources.achlist"),
        r#"["scripts/source/Helpers.psc", "scripts/source/Example.psc"]"#,
    );
    let achlist_path = dir.path().join("sources.achlist");

    let (code, _stdout, stderr) = run_captured(&[
        "fix".to_string(),
        "--type=unused-import".to_string(),
        achlist_path.to_string_lossy().into_owned(),
    ]);

    assert_eq!(code, 0, "stderr: {stderr}");
    assert_eq!(
        fs::read_to_string(&script_path).unwrap(),
        "ScriptName Example\n\n\nFunction Test()\n    Call(1,2)\nEndFunction\n"
    );
}

#[test]
fn fix_line_filter_errors_when_removing_an_unused_import_changes_the_line_count() {
    let dir = tempfile::tempdir().expect("failed to create temp dir");
    write_file(
        &dir.path().join("scripts/source/Helpers.psc"),
        "ScriptName Helpers\n\nGlobal Function Assist()\nEndFunction\n",
    );
    let script_path = dir.path().join("scripts/source/Example.psc");
    write_file(
        &script_path,
        "ScriptName Example\n\nImport Helpers\n\nFunction Test()\nEndFunction\n",
    );
    write_file(
        &dir.path().join("sources.achlist"),
        r#"["scripts/source/Helpers.psc", "scripts/source/Example.psc"]"#,
    );
    let achlist_path = dir.path().join("sources.achlist");

    let (code, _stdout, stderr) = run_captured(&[
        "fix".to_string(),
        "--line=3".to_string(),
        achlist_path.to_string_lossy().into_owned(),
    ]);

    assert_eq!(code, 2);
    assert!(stderr.contains("changes the file's line count"));
    assert_eq!(
        fs::read_to_string(&script_path).unwrap(),
        "ScriptName Example\n\nImport Helpers\n\nFunction Test()\nEndFunction\n"
    );
}

#[test]
fn resolves_cross_script_argument_types_from_the_project_root() {
    let dir = tempfile::tempdir().expect("failed to create temp dir");
    write_file(
        &dir.path().join("scripts/source/Greeter.psc"),
        "ScriptName Greeter\n\nFunction Greet(String name)\nEndFunction\n",
    );
    write_file(
        &dir.path().join("scripts/source/Example.psc"),
        "ScriptName Example\n\nGreeter Property Target Auto\n\nFunction Test()\n    Target.Greet(1)\nEndFunction\n",
    );
    write_file(
        &dir.path().join("sources.achlist"),
        r#"["scripts/source/Greeter.psc", "scripts/source/Example.psc"]"#,
    );
    let achlist_path = dir.path().join("sources.achlist");

    let (code, stdout, _stderr) = run_captured(&[achlist_path.to_string_lossy().into_owned()]);

    assert_eq!(code, 1);
    assert!(stdout.contains("[argument-types]"));
}

#[test]
fn flags_a_call_through_a_script_name_to_a_function_not_declared_global() {
    let dir = tempfile::tempdir().expect("failed to create temp dir");
    write_file(
        &dir.path().join("scripts/source/MyScriptOne.psc"),
        "ScriptName MyScriptOne Extends Form\n\nFunction IMNotStatic()\nEndFunction\n",
    );
    write_file(
        &dir.path().join("scripts/source/MyScriptTwo.psc"),
        "ScriptName MyScriptTwo Extends Form\n\nFunction Mine()\n    MyScriptOne.IMNotStatic()\nEndFunction\n",
    );
    write_file(
        &dir.path().join("sources.achlist"),
        r#"["scripts/source/MyScriptOne.psc", "scripts/source/MyScriptTwo.psc"]"#,
    );
    let achlist_path = dir.path().join("sources.achlist");

    let (code, stdout, stderr) = run_captured(&[achlist_path.to_string_lossy().into_owned()]);

    assert_eq!(code, 1, "stderr: {stderr}");
    assert!(
        stdout.contains("[non-global-function-call]"),
        "stdout: {stdout}"
    );
    assert!(stdout.contains("'IMNotStatic' is not declared Global on 'MyScriptOne'"));
}

#[test]
fn resolves_cross_script_types_from_every_directory_listed_in_the_achlist() {
    let dir = tempfile::tempdir().expect("failed to create temp dir");
    write_file(
        &dir.path().join("source/dir/one/Greeter.psc"),
        "ScriptName Greeter\n\nFunction Greet(String name)\nEndFunction\n",
    );
    write_file(
        &dir.path().join("source/dir/two/Example.psc"),
        "ScriptName Example\n\nGreeter Property Target Auto\n\nFunction Test()\n    Target.Greet(1)\nEndFunction\n",
    );
    write_file(
        &dir.path().join("sources.achlist"),
        r#"["source/dir/one/Greeter.psc", "source/dir/two/Example.psc"]"#,
    );

    let (code, stdout, stderr) = run_captured(&[dir
        .path()
        .join("sources.achlist")
        .to_string_lossy()
        .into_owned()]);

    assert_eq!(code, 1, "stderr: {stderr}");
    assert!(stdout.contains("[argument-types]"));
}

#[test]
fn goto_state_resolves_a_state_declared_on_a_parent_script_listed_in_the_achlist() {
    // Regression test for https://github.com/Idrinth/papyrus-lint/issues/259:
    // a state declared only on a script's Extends ancestor must not be
    // flagged as missing, even when (as here) the achlist's entries
    // don't sit under either conventional scripts/source layout.
    let dir = tempfile::tempdir().expect("failed to create temp dir");
    write_file(
        &dir.path().join("VahlokStateBase.psc"),
        "Scriptname VahlokStateBase extends ObjectReference\n\nauto State Idle\nEndState\n\nState Busy\nEndState\n",
    );
    write_file(
        &dir.path().join("VahlokStateChild.psc"),
        "Scriptname VahlokStateChild extends VahlokStateBase\n\nState Extra\nEndState\n\nFunction Demo()\n    GoToState(\"Extra\")\n    GoToState(\"Idle\")\n    GoToState(\"Busy\")\n    GoToState(\"NoSuchState\")\nEndFunction\n",
    );
    write_file(
        &dir.path().join("scripts.achlist"),
        r#"["VahlokStateBase.psc", "VahlokStateChild.psc"]"#,
    );

    let (code, stdout, stderr) = run_captured(&[dir
        .path()
        .join("scripts.achlist")
        .to_string_lossy()
        .into_owned()]);

    assert_eq!(code, 0, "stderr: {stderr}, stdout: {stdout}");
    assert!(!stdout.contains("'Extra'"));
    assert!(!stdout.contains("'Idle'"));
    assert!(!stdout.contains("'Busy'"));
    assert!(stdout.contains("[goto-state]"));
    assert!(stdout.contains("'NoSuchState'"));
}

#[test]
fn achlist_resolves_an_unlisted_sibling_script_by_default_for_backward_compatibility() {
    // `strict_achlist_scope` defaults to false, so an achlist-based
    // project already depending on the pre-#311-fix behavior (every
    // listed entry's directory acting as a generic search root) must
    // see no change: `Unlisted.psc` sits right beside the listed
    // `Example.psc`, is never itself mentioned in the achlist, and
    // still resolves.
    let dir = tempfile::tempdir().expect("failed to create temp dir");
    write_file(
        &dir.path().join("mods/one/Example.psc"),
        "ScriptName Example\n\nFunction Test()\n    Unlisted.DoThing()\nEndFunction\n",
    );
    write_file(
        &dir.path().join("mods/one/Unlisted.psc"),
        "ScriptName Unlisted\n\nFunction DoThing() Global\nEndFunction\n",
    );
    write_file(
        &dir.path().join("scripts.achlist"),
        r#"["mods/one/Example.psc"]"#,
    );

    let (code, stdout, stderr) = run_captured(&[dir
        .path()
        .join("scripts.achlist")
        .to_string_lossy()
        .into_owned()]);

    assert_eq!(code, 0, "stderr: {stderr}");
    assert!(!stdout.contains("[unresolved-script]"), "stdout: {stdout}");
}

#[test]
fn strict_achlist_scope_does_not_leak_into_an_unlisted_sibling_script() {
    // Regression test for https://github.com/Idrinth/papyrus-lint/issues/311:
    // with `strict_achlist_scope: true`, an achlist entry's directory
    // must not become a generic search root, since that would silently
    // make every *other* file in that directory resolvable too, even
    // though it was never listed. Here `Unlisted.psc` sits right beside
    // the listed `Example.psc` but is itself never mentioned in the
    // achlist, so a call against its type must be reported as
    // unresolved once strict scoping is turned on.
    let dir = tempfile::tempdir().expect("failed to create temp dir");
    write_file(
        &dir.path().join("mods/one/Example.psc"),
        "ScriptName Example\n\nFunction Test()\n    Unlisted.DoThing()\nEndFunction\n",
    );
    write_file(
        &dir.path().join("mods/one/Unlisted.psc"),
        "ScriptName Unlisted\n\nFunction DoThing() Global\nEndFunction\n",
    );
    write_file(
        &dir.path().join("scripts.achlist"),
        r#"["mods/one/Example.psc"]"#,
    );
    write_file(
        &dir.path().join("papyrus-lint.yaml"),
        "strict_achlist_scope: true\n",
    );

    let (code, stdout, stderr) = run_captured(&[dir
        .path()
        .join("scripts.achlist")
        .to_string_lossy()
        .into_owned()]);

    assert_eq!(code, 0, "stderr: {stderr}");
    assert!(stdout.contains("[unresolved-script]"), "stdout: {stdout}");
    assert!(stdout.contains("'Unlisted'"), "stdout: {stdout}");
}

#[test]
fn strict_achlist_scope_is_honored_from_an_explicit_config_path() {
    // Regression test for https://github.com/Idrinth/papyrus-lint/issues/362:
    // `--config <path>` must still pick up `strict_achlist_scope` from
    // the file it names, rather than always resolving as if it were
    // off (which made the flag appear entirely inert whenever
    // `--config` was used, and left an achlist entry's directory
    // reachable as a search root when it should not have been).
    let dir = tempfile::tempdir().expect("failed to create temp dir");
    write_file(
        &dir.path().join("mods/one/Example.psc"),
        "ScriptName Example\n\nFunction Test()\n    Unlisted.DoThing()\nEndFunction\n",
    );
    write_file(
        &dir.path().join("mods/one/Unlisted.psc"),
        "ScriptName Unlisted\n\nFunction DoThing() Global\nEndFunction\n",
    );
    write_file(
        &dir.path().join("scripts.achlist"),
        r#"["mods/one/Example.psc"]"#,
    );
    let config_path = dir.path().join("custom-config.yaml");
    write_file(&config_path, "strict_achlist_scope: true\n");

    let (code, stdout, stderr) = run_captured(&[
        "--config".to_string(),
        config_path.to_string_lossy().into_owned(),
        dir.path()
            .join("scripts.achlist")
            .to_string_lossy()
            .into_owned(),
    ]);

    assert_eq!(code, 0, "stderr: {stderr}");
    assert!(stdout.contains("[unresolved-script]"), "stdout: {stdout}");
    assert!(stdout.contains("'Unlisted'"), "stdout: {stdout}");
}

#[test]
fn strict_achlist_scope_still_flags_conflicting_versions_between_two_achlist_entries_sharing_a_file_name(
) {
    // Regression test for https://github.com/Idrinth/papyrus-lint/issues/311:
    // with `strict_achlist_scope: true`, two achlist entries can share a
    // file name while living in directories that are no longer scanned
    // as search roots for one another (see the previous test), so the
    // conflicting-script-versions check has to compare the achlist's
    // own listed entries directly rather than relying on a directory
    // scan to notice the collision.
    let dir = tempfile::tempdir().expect("failed to create temp dir");
    write_file(
        &dir.path().join("mods/one/Example.psc"),
        "ScriptName Example\n",
    );
    write_file(
        &dir.path().join("mods/two/Example.psc"),
        "ScriptName Example\n; a different version\n",
    );
    write_file(
        &dir.path().join("scripts.achlist"),
        r#"["mods/one/Example.psc", "mods/two/Example.psc"]"#,
    );
    write_file(
        &dir.path().join("papyrus-lint.yaml"),
        "strict_achlist_scope: true\n",
    );

    let (code, stdout, stderr) = run_captured(&[dir
        .path()
        .join("scripts.achlist")
        .to_string_lossy()
        .into_owned()]);

    assert_eq!(code, 0, "stderr: {stderr}");
    assert_eq!(
        stdout.matches("[conflicting-script-versions]").count(),
        2,
        "stdout: {stdout}"
    );
}

#[test]
fn strict_achlist_scope_does_not_double_report_a_conflict_also_visible_via_a_conventional_directory(
) {
    // Regression test: when two conflicting achlist entries also happen
    // to sit under the project's conventional scripts/source and
    // source/scripts directories, strict mode must report the
    // collision once per file (via conflicting_script_versions_among),
    // not twice (once more via the directory-based
    // conflicting_script_versions, which strict mode must skip
    // entirely to avoid duplicating what it already reports).
    let dir = tempfile::tempdir().expect("failed to create temp dir");
    write_file(
        &dir.path().join("scripts/source/Example.psc"),
        "ScriptName Example\n",
    );
    write_file(
        &dir.path().join("source/scripts/Example.psc"),
        "ScriptName Example\n; a different version\n",
    );
    write_file(
        &dir.path().join("scripts.achlist"),
        r#"["scripts/source/Example.psc", "source/scripts/Example.psc"]"#,
    );
    write_file(
        &dir.path().join("papyrus-lint.yaml"),
        "strict_achlist_scope: true\n",
    );

    let (code, stdout, stderr) = run_captured(&[dir
        .path()
        .join("scripts.achlist")
        .to_string_lossy()
        .into_owned()]);

    assert_eq!(code, 0, "stderr: {stderr}");
    assert_eq!(
        stdout.matches("[conflicting-script-versions]").count(),
        2,
        "stdout: {stdout}"
    );
}

#[test]
fn flags_a_script_newer_than_its_compiled_pex() {
    let dir = tempfile::tempdir().expect("failed to create temp dir");
    let script_path = dir.path().join("scripts/source/Example.psc");
    let pex_path = dir.path().join("scripts/Example.pex");
    write_file(&script_path, "ScriptName Example\n");
    write_file(&pex_path, "");

    let now = std::time::SystemTime::now();
    let pex_file = fs::File::open(&pex_path).expect("failed to open pex file");
    pex_file
        .set_modified(now - std::time::Duration::from_secs(60))
        .expect("failed to set pex mtime");
    let script_file = fs::File::open(&script_path).expect("failed to open script file");
    script_file
        .set_modified(now)
        .expect("failed to set script mtime");

    let (code, stdout, stderr) = run_captured(&[script_path.to_string_lossy().into_owned()]);

    assert_eq!(code, 0, "stderr: {stderr}");
    assert!(
        stdout.contains("[stale-compiled-output]"),
        "stdout: {stdout}"
    );
    assert!(stdout.contains("[info]"), "stdout: {stdout}");
}

#[test]
fn does_not_flag_a_script_older_than_its_compiled_pex() {
    let dir = tempfile::tempdir().expect("failed to create temp dir");
    let script_path = dir.path().join("scripts/source/Example.psc");
    let pex_path = dir.path().join("scripts/Example.pex");
    write_file(&script_path, "ScriptName Example\n");
    write_file(&pex_path, "");

    let now = std::time::SystemTime::now();
    let script_file = fs::File::open(&script_path).expect("failed to open script file");
    script_file
        .set_modified(now - std::time::Duration::from_secs(60))
        .expect("failed to set script mtime");
    let pex_file = fs::File::open(&pex_path).expect("failed to open pex file");
    pex_file.set_modified(now).expect("failed to set pex mtime");

    let (code, stdout, stderr) = run_captured(&[script_path.to_string_lossy().into_owned()]);

    assert_eq!(code, 0, "stderr: {stderr}");
    assert!(
        !stdout.contains("[stale-compiled-output]"),
        "stdout: {stdout}"
    );
}

#[test]
fn stale_compiled_output_can_be_disabled() {
    let dir = tempfile::tempdir().expect("failed to create temp dir");
    let script_path = dir.path().join("scripts/source/Example.psc");
    let pex_path = dir.path().join("scripts/Example.pex");
    write_file(&script_path, "ScriptName Example\n");
    write_file(&pex_path, "");
    write_file(
        &dir.path().join("papyrus-lint.yaml"),
        "rules:\n  stale_compiled_output: false\n",
    );

    let now = std::time::SystemTime::now();
    let pex_file = fs::File::open(&pex_path).expect("failed to open pex file");
    pex_file
        .set_modified(now - std::time::Duration::from_secs(60))
        .expect("failed to set pex mtime");
    let script_file = fs::File::open(&script_path).expect("failed to open script file");
    script_file
        .set_modified(now)
        .expect("failed to set script mtime");

    let (code, stdout, stderr) = run_captured(&[script_path.to_string_lossy().into_owned()]);

    assert_eq!(code, 0, "stderr: {stderr}");
    assert!(
        !stdout.contains("[stale-compiled-output]"),
        "stdout: {stdout}"
    );
}

#[test]
fn flags_a_script_name_that_does_not_match_its_file_name() {
    let dir = tempfile::tempdir().expect("failed to create temp dir");
    let script_path = dir.path().join("scripts/source/Other.psc");
    write_file(&script_path, "ScriptName Example\n");

    let (code, stdout, stderr) = run_captured(&[script_path.to_string_lossy().into_owned()]);

    assert_eq!(code, 1, "stderr: {stderr}");
    assert!(
        stdout.contains("[script-filename-mismatch]"),
        "stdout: {stdout}"
    );
    assert!(stdout.contains("[error]"), "stdout: {stdout}");
}

#[test]
fn does_not_flag_a_script_name_matching_its_file_name() {
    let dir = tempfile::tempdir().expect("failed to create temp dir");
    let script_path = dir.path().join("scripts/source/Example.psc");
    write_file(&script_path, "ScriptName Example\n");

    let (code, stdout, stderr) = run_captured(&[script_path.to_string_lossy().into_owned()]);

    assert_eq!(code, 0, "stderr: {stderr}");
    assert!(
        !stdout.contains("[script-filename-mismatch]"),
        "stdout: {stdout}"
    );
}

#[test]
fn script_filename_mismatch_can_be_disabled() {
    let dir = tempfile::tempdir().expect("failed to create temp dir");
    let script_path = dir.path().join("scripts/source/Other.psc");
    write_file(&script_path, "ScriptName Example\n");
    write_file(
        &dir.path().join("papyrus-lint.yaml"),
        "rules:\n  script_filename_mismatch: false\n",
    );

    let (code, stdout, stderr) = run_captured(&[script_path.to_string_lossy().into_owned()]);

    assert_eq!(code, 0, "stderr: {stderr}");
    assert!(
        !stdout.contains("[script-filename-mismatch]"),
        "stdout: {stdout}"
    );
}

#[test]
fn script_filename_mismatch_can_be_suppressed_with_a_disable_comment() {
    let dir = tempfile::tempdir().expect("failed to create temp dir");
    let script_path = dir.path().join("scripts/source/Other.psc");
    write_file(
        &script_path,
        "ScriptName Example ; @disable script-filename-mismatch\n",
    );

    let (code, stdout, stderr) = run_captured(&[script_path.to_string_lossy().into_owned()]);

    assert_eq!(code, 0, "stderr: {stderr}");
    assert!(
        !stdout.contains("[script-filename-mismatch]"),
        "stdout: {stdout}"
    );
}

#[test]
fn script_filename_mismatch_can_be_suppressed_with_a_disable_file_comment() {
    let dir = tempfile::tempdir().expect("failed to create temp dir");
    let script_path = dir.path().join("scripts/source/Other.psc");
    write_file(
        &script_path,
        "ScriptName Example\n; @disable-file script-filename-mismatch\n",
    );

    let (code, stdout, stderr) = run_captured(&[script_path.to_string_lossy().into_owned()]);

    assert_eq!(code, 0, "stderr: {stderr}");
    assert!(
        !stdout.contains("[script-filename-mismatch]"),
        "stdout: {stdout}"
    );
}

// Regression tests for
// https://github.com/Idrinth/papyrus-lint/issues/772: `stale-compiled-output`,
// `conflicting-script-versions`, and `script-filename-mismatch` are
// computed after `papyrus_lints::lint_with_external_arguments` used to
// run its own unused-directive validation, so an `@disable`/
// `@disable-file` directive that correctly suppressed one of them was
// still reported as an `unused-disable`, even though the diagnostic it
// named was in fact suppressed.

#[test]
fn stale_compiled_output_disable_comment_is_not_reported_as_unused() {
    let dir = tempfile::tempdir().expect("failed to create temp dir");
    let script_path = dir.path().join("scripts/source/Example.psc");
    let pex_path = dir.path().join("scripts/Example.pex");
    write_file(
        &script_path,
        "ScriptName Example ; @disable stale-compiled-output\n",
    );
    write_file(&pex_path, "");
    write_file(
        &dir.path().join("papyrus-lint.yaml"),
        "rules:\n  unused_disable: true\n",
    );

    let now = std::time::SystemTime::now();
    let pex_file = fs::File::open(&pex_path).expect("failed to open pex file");
    pex_file
        .set_modified(now - std::time::Duration::from_secs(60))
        .expect("failed to set pex mtime");
    let script_file = fs::File::open(&script_path).expect("failed to open script file");
    script_file
        .set_modified(now)
        .expect("failed to set script mtime");

    let (code, stdout, stderr) = run_captured(&[script_path.to_string_lossy().into_owned()]);

    assert_eq!(code, 0, "stderr: {stderr}");
    assert!(
        !stdout.contains("[stale-compiled-output]"),
        "stdout: {stdout}"
    );
    assert!(!stdout.contains("[unused-disable]"), "stdout: {stdout}");
}

#[test]
fn stale_compiled_output_disable_file_comment_is_not_reported_as_unused() {
    let dir = tempfile::tempdir().expect("failed to create temp dir");
    let script_path = dir.path().join("scripts/source/Example.psc");
    let pex_path = dir.path().join("scripts/Example.pex");
    write_file(
        &script_path,
        "ScriptName Example\n; @disable-file stale-compiled-output\n",
    );
    write_file(&pex_path, "");
    write_file(
        &dir.path().join("papyrus-lint.yaml"),
        "rules:\n  unused_disable: true\n",
    );

    let now = std::time::SystemTime::now();
    let pex_file = fs::File::open(&pex_path).expect("failed to open pex file");
    pex_file
        .set_modified(now - std::time::Duration::from_secs(60))
        .expect("failed to set pex mtime");
    let script_file = fs::File::open(&script_path).expect("failed to open script file");
    script_file
        .set_modified(now)
        .expect("failed to set script mtime");

    let (code, stdout, stderr) = run_captured(&[script_path.to_string_lossy().into_owned()]);

    assert_eq!(code, 0, "stderr: {stderr}");
    assert!(
        !stdout.contains("[stale-compiled-output]"),
        "stdout: {stdout}"
    );
    assert!(!stdout.contains("[unused-disable]"), "stdout: {stdout}");
}

#[test]
fn conflicting_script_versions_disable_comment_is_not_reported_as_unused() {
    let dir = tempfile::tempdir().expect("failed to create temp dir");
    let script_path = dir.path().join("scripts/source/Example.psc");
    write_file(
        &script_path,
        "ScriptName Example ; @disable conflicting-script-versions\n",
    );
    write_file(
        &dir.path().join("source/scripts/Example.psc"),
        "ScriptName Example\n; a different version\n",
    );
    write_file(
        &dir.path().join("papyrus-lint.yaml"),
        "rules:\n  unused_disable: true\n",
    );

    let (code, stdout, stderr) = run_captured(&[script_path.to_string_lossy().into_owned()]);

    assert_eq!(code, 0, "stderr: {stderr}");
    assert!(
        !stdout.contains("[conflicting-script-versions]"),
        "stdout: {stdout}"
    );
    assert!(!stdout.contains("[unused-disable]"), "stdout: {stdout}");
}

#[test]
fn conflicting_script_versions_disable_file_comment_is_not_reported_as_unused() {
    let dir = tempfile::tempdir().expect("failed to create temp dir");
    let script_path = dir.path().join("scripts/source/Example.psc");
    write_file(
        &script_path,
        "ScriptName Example\n; @disable-file conflicting-script-versions\n",
    );
    write_file(
        &dir.path().join("source/scripts/Example.psc"),
        "ScriptName Example\n; a different version\n",
    );
    write_file(
        &dir.path().join("papyrus-lint.yaml"),
        "rules:\n  unused_disable: true\n",
    );

    let (code, stdout, stderr) = run_captured(&[script_path.to_string_lossy().into_owned()]);

    assert_eq!(code, 0, "stderr: {stderr}");
    assert!(
        !stdout.contains("[conflicting-script-versions]"),
        "stdout: {stdout}"
    );
    assert!(!stdout.contains("[unused-disable]"), "stdout: {stdout}");
}

#[test]
fn script_filename_mismatch_disable_comment_is_not_reported_as_unused() {
    let dir = tempfile::tempdir().expect("failed to create temp dir");
    let script_path = dir.path().join("scripts/source/Other.psc");
    write_file(
        &script_path,
        "ScriptName Example ; @disable script-filename-mismatch\n",
    );
    write_file(
        &dir.path().join("papyrus-lint.yaml"),
        "rules:\n  unused_disable: true\n",
    );

    let (code, stdout, stderr) = run_captured(&[script_path.to_string_lossy().into_owned()]);

    assert_eq!(code, 0, "stderr: {stderr}");
    assert!(
        !stdout.contains("[script-filename-mismatch]"),
        "stdout: {stdout}"
    );
    assert!(!stdout.contains("[unused-disable]"), "stdout: {stdout}");
}

#[test]
fn script_filename_mismatch_disable_file_comment_is_not_reported_as_unused() {
    let dir = tempfile::tempdir().expect("failed to create temp dir");
    let script_path = dir.path().join("scripts/source/Other.psc");
    write_file(
        &script_path,
        "ScriptName Example\n; @disable-file script-filename-mismatch\n",
    );
    write_file(
        &dir.path().join("papyrus-lint.yaml"),
        "rules:\n  unused_disable: true\n",
    );

    let (code, stdout, stderr) = run_captured(&[script_path.to_string_lossy().into_owned()]);

    assert_eq!(code, 0, "stderr: {stderr}");
    assert!(
        !stdout.contains("[script-filename-mismatch]"),
        "stdout: {stdout}"
    );
    assert!(!stdout.contains("[unused-disable]"), "stdout: {stdout}");
}

#[test]
fn resolves_cross_script_argument_types_from_the_projects_configured_script_root() {
    let dir = tempfile::tempdir().expect("failed to create temp dir");
    let shared_dir = tempfile::tempdir().expect("failed to create temp dir");
    write_file(
        &shared_dir.path().join("Greeter.psc"),
        "ScriptName Greeter\n\nFunction Greet(String name)\nEndFunction\n",
    );
    write_file(
        &dir.path().join("scripts/source/Example.psc"),
        "ScriptName Example\n\nGreeter Property Target Auto\n\nFunction Test()\n    Target.Greet(1)\nEndFunction\n",
    );
    write_file(
        &dir.path().join("sources.achlist"),
        r#"["scripts/source/Example.psc"]"#,
    );
    write_file(
        &dir.path().join("papyrus-lint.yaml"),
        &format!(
            "additional_script_roots:\n  - {}\n",
            shared_dir.path().display()
        ),
    );
    let achlist_path = dir.path().join("sources.achlist");

    let (code, stdout, _stderr) = run_captured(&[achlist_path.to_string_lossy().into_owned()]);

    assert_eq!(code, 1);
    assert!(stdout.contains("[argument-types]"));
}

#[test]
fn resolves_cross_script_argument_types_from_a_script_root_flag() {
    let dir = tempfile::tempdir().expect("failed to create temp dir");
    let shared_dir = tempfile::tempdir().expect("failed to create temp dir");
    write_file(
        &shared_dir.path().join("Greeter.psc"),
        "ScriptName Greeter\n\nFunction Greet(String name)\nEndFunction\n",
    );
    write_file(
        &dir.path().join("scripts/source/Example.psc"),
        "ScriptName Example\n\nGreeter Property Target Auto\n\nFunction Test()\n    Target.Greet(1)\nEndFunction\n",
    );
    write_file(
        &dir.path().join("sources.achlist"),
        r#"["scripts/source/Example.psc"]"#,
    );
    let achlist_path = dir.path().join("sources.achlist");

    let (code, stdout, _stderr) = run_captured(&[
        "--script-root".to_string(),
        shared_dir.path().to_string_lossy().into_owned(),
        achlist_path.to_string_lossy().into_owned(),
    ]);

    assert_eq!(code, 1);
    assert!(stdout.contains("[argument-types]"));
}

#[test]
fn script_root_flag_without_a_value_prints_usage() {
    let (code, _stdout, stderr) = run_captured(&["--script-root".to_string()]);

    assert_eq!(code, 2);
    assert!(stderr.contains("Usage: PapyrusLinterCLI"));
}

#[test]
fn config_flag_skips_the_project_roots_additional_script_roots() {
    let dir = tempfile::tempdir().expect("failed to create temp dir");
    let shared_dir = tempfile::tempdir().expect("failed to create temp dir");
    write_file(
        &shared_dir.path().join("Greeter.psc"),
        "ScriptName Greeter\n\nFunction Greet(String name)\nEndFunction\n",
    );
    write_file(
        &dir.path().join("scripts/source/Example.psc"),
        "ScriptName Example\n\nGreeter Property Target Auto\n\nFunction Test()\n    Target.Greet(1)\nEndFunction\n",
    );
    write_file(
        &dir.path().join("sources.achlist"),
        r#"["scripts/source/Example.psc"]"#,
    );
    write_file(
        &dir.path().join("papyrus-lint.yaml"),
        &format!(
            "additional_script_roots:\n  - {}\n",
            shared_dir.path().display()
        ),
    );
    let override_path = dir.path().join("overrides/custom.yaml");
    write_file(&override_path, "");
    let achlist_path = dir.path().join("sources.achlist");

    let (code, stdout, _stderr) = run_captured(&[
        "--config".to_string(),
        override_path.to_string_lossy().into_owned(),
        achlist_path.to_string_lossy().into_owned(),
    ]);

    // Greeter can't be resolved (the project root's own config, which
    // declares the shared_dir root, is bypassed by --config), so the
    // "Argument type check" lint has nothing to flag.
    assert_eq!(code, 0);
    assert!(!stdout.contains("[argument-types]"));
}

#[test]
fn json_flag_prints_a_single_json_report_instead_of_plain_text() {
    let dir = tempfile::tempdir().expect("failed to create temp dir");
    write_file(
        &dir.path().join("scripts/source/Example.psc"),
        "ScriptName Example   \n",
    );
    write_file(
        &dir.path().join("sources.achlist"),
        r#"["scripts/source/Example.psc"]"#,
    );
    let achlist_path = dir.path().join("sources.achlist");

    let (code, stdout, stderr) = run_captured(&[
        "--json".to_string(),
        achlist_path.to_string_lossy().into_owned(),
    ]);

    assert_eq!(code, 0);
    assert_eq!(stderr, "");
    assert!(!stdout.contains("PapyrusLinterCLI:"));

    let report: serde_json::Value =
        serde_json::from_str(&stdout).expect("stdout should be a single JSON document");
    assert_eq!(report["success"], true);
    assert_eq!(report["scripts_checked"], 1);
    assert_eq!(report["files_with_diagnostics"], 1);
    assert_eq!(report["total_diagnostics"], 1);
    assert!(report["files_fixed"].is_null());
    let files = report["files"]
        .as_array()
        .expect("files should be an array");
    assert_eq!(files.len(), 1);
    let diagnostics = files[0]["diagnostics"]
        .as_array()
        .expect("diagnostics should be an array");
    assert_eq!(diagnostics.len(), 1);
    assert_eq!(diagnostics[0]["rule"], "trailing-whitespace");
    assert_eq!(diagnostics[0]["level"], "warning");
    assert_eq!(diagnostics[0]["line"], 1);
}

#[test]
fn json_flag_lists_every_resolved_script_including_clean_ones() {
    let dir = tempfile::tempdir().expect("failed to create temp dir");
    write_file(
        &dir.path().join("scripts/source/Example.psc"),
        "ScriptName Example\n",
    );
    write_file(
        &dir.path().join("sources.achlist"),
        r#"["scripts/source/Example.psc"]"#,
    );
    let achlist_path = dir.path().join("sources.achlist");

    let (code, stdout, _stderr) = run_captured(&[
        "--json".to_string(),
        achlist_path.to_string_lossy().into_owned(),
    ]);

    assert_eq!(code, 0);
    let report: serde_json::Value = serde_json::from_str(&stdout).unwrap();
    assert_eq!(report["success"], true);
    let files = report["files"].as_array().unwrap();
    assert_eq!(files.len(), 1);
    assert_eq!(files[0]["diagnostics"].as_array().unwrap().len(), 0);
}

#[test]
fn json_flag_combines_with_the_fix_subcommand() {
    let dir = tempfile::tempdir().expect("failed to create temp dir");
    write_file(
        &dir.path().join("scripts/source/Example.psc"),
        "ScriptName Example   \n\nFunction DoThing()\n\tGame.GetPlayer()\nEndFunction\n",
    );
    write_file(
        &dir.path().join("sources.achlist"),
        r#"["scripts/source/Example.psc"]"#,
    );
    let achlist_path = dir.path().join("sources.achlist");

    let (code, stdout, _stderr) = run_captured(&[
        "fix".to_string(),
        "--json".to_string(),
        achlist_path.to_string_lossy().into_owned(),
    ]);

    assert_eq!(code, 1);
    assert_eq!(
        fs::read_to_string(dir.path().join("scripts/source/Example.psc")).unwrap(),
        "ScriptName Example\n\nFunction DoThing()\n\tGame.GetPlayer()\nEndFunction\n"
    );
    let report: serde_json::Value = serde_json::from_str(&stdout).unwrap();
    assert_eq!(report["files_fixed"], 1);
    let files = report["files"].as_array().unwrap();
    let diagnostics = files[0]["diagnostics"].as_array().unwrap();
    assert!(diagnostics
        .iter()
        .any(|d| d["message"].as_str().unwrap().contains("Game.GetPlayer")));
}

#[test]
fn json_flag_combines_with_fix_dry_run() {
    let dir = tempfile::tempdir().expect("failed to create temp dir");
    write_file(
        &dir.path().join("scripts/source/Example.psc"),
        "ScriptName Example   \n\nFunction DoThing()\n\tGame.GetPlayer()\nEndFunction\n",
    );
    write_file(
        &dir.path().join("sources.achlist"),
        r#"["scripts/source/Example.psc"]"#,
    );
    let achlist_path = dir.path().join("sources.achlist");

    let (code, stdout, stderr) = run_captured(&[
        "fix".to_string(),
        "--dry-run".to_string(),
        "--json".to_string(),
        achlist_path.to_string_lossy().into_owned(),
    ]);

    assert_eq!(code, 1, "stderr: {stderr}");
    assert_eq!(
        fs::read_to_string(dir.path().join("scripts/source/Example.psc")).unwrap(),
        "ScriptName Example   \n\nFunction DoThing()\n\tGame.GetPlayer()\nEndFunction\n",
        "--dry-run must never write to the file, even combined with --json"
    );
    let report: serde_json::Value = serde_json::from_str(&stdout).unwrap();
    assert_eq!(report["dry_run"], true);
    assert_eq!(report["files_fixed"], 1);
    let files = report["files"].as_array().unwrap();
    let diff = files[0]["diff"].as_str().expect("diff should be a string");
    assert!(diff.contains("-ScriptName Example   \n"));
    assert!(diff.contains("+ScriptName Example\n"));
}

#[test]
fn config_flag_overrides_project_root_discovery() {
    let dir = tempfile::tempdir().expect("failed to create temp dir");
    write_file(
        &dir.path().join("scripts/source/Example.psc"),
        "ScriptName Example   \n",
    );
    write_file(
        &dir.path().join("sources.achlist"),
        r#"["scripts/source/Example.psc"]"#,
    );
    // A project-root config that would otherwise apply, and an
    // unrelated override file elsewhere that should win instead.
    write_file(
        &dir.path().join("papyrus-lint.yaml"),
        "rules:\n  trailing_whitespace: true\n",
    );
    let override_path = dir.path().join("overrides/custom.yaml");
    write_file(&override_path, "rules:\n  trailing_whitespace: false\n");
    let achlist_path = dir.path().join("sources.achlist");

    let (code, stdout, _stderr) = run_captured(&[
        "--config".to_string(),
        override_path.to_string_lossy().into_owned(),
        achlist_path.to_string_lossy().into_owned(),
    ]);

    assert_eq!(code, 0);
    assert!(stdout.contains("no problems found"));
}

#[test]
fn config_flag_combines_with_fix_and_json_in_any_order() {
    let dir = tempfile::tempdir().expect("failed to create temp dir");
    write_file(
        &dir.path().join("scripts/source/Example.psc"),
        "ScriptName Example   \n",
    );
    write_file(
        &dir.path().join("sources.achlist"),
        r#"["scripts/source/Example.psc"]"#,
    );
    let override_path = dir.path().join("overrides/custom.yaml");
    write_file(&override_path, "rules:\n  trailing_whitespace: false\n");
    let achlist_path = dir.path().join("sources.achlist");

    let (code, stdout, _stderr) = run_captured(&[
        "--json".to_string(),
        "fix".to_string(),
        "--config".to_string(),
        override_path.to_string_lossy().into_owned(),
        achlist_path.to_string_lossy().into_owned(),
    ]);

    assert_eq!(code, 0);
    // trailing_whitespace was disabled by the overriding config, so
    // fix should leave the trailing whitespace in place untouched.
    assert_eq!(
        fs::read_to_string(dir.path().join("scripts/source/Example.psc")).unwrap(),
        "ScriptName Example   \n"
    );
    let report: serde_json::Value = serde_json::from_str(&stdout).unwrap();
    assert_eq!(report["success"], true);
    assert_eq!(report["total_diagnostics"], 0);
}

#[test]
fn config_flag_errors_when_the_override_file_is_missing() {
    let dir = tempfile::tempdir().expect("failed to create temp dir");
    write_file(
        &dir.path().join("scripts/source/Example.psc"),
        "ScriptName Example\n",
    );
    write_file(
        &dir.path().join("sources.achlist"),
        r#"["scripts/source/Example.psc"]"#,
    );
    let achlist_path = dir.path().join("sources.achlist");
    let missing_config = dir.path().join("missing.yaml");

    let (code, _stdout, stderr) = run_captured(&[
        "--config".to_string(),
        missing_config.to_string_lossy().into_owned(),
        achlist_path.to_string_lossy().into_owned(),
    ]);

    assert_eq!(code, 2);
    assert!(stderr.starts_with("error: failed to load lint config:"));
}

#[test]
fn config_flag_without_a_value_prints_usage() {
    let (code, _stdout, stderr) = run_captured(&["--config".to_string()]);

    assert_eq!(code, 2);
    assert!(stderr.contains("Usage: PapyrusLinterCLI"));
}

#[test]
fn json_flag_can_precede_the_fix_subcommand() {
    let dir = tempfile::tempdir().expect("failed to create temp dir");
    let script_path = dir.path().join("Example.psc");
    write_file(&script_path, "ScriptName Example   \n");

    let (code, stdout, stderr) = run_captured(&[
        "--json".to_string(),
        "fix".to_string(),
        script_path.to_string_lossy().into_owned(),
    ]);

    assert_eq!(code, 0);
    assert!(stderr.is_empty());
    assert_eq!(
        fs::read_to_string(&script_path).unwrap(),
        "ScriptName Example\n"
    );
    let report: serde_json::Value = serde_json::from_str(&stdout).unwrap();
    assert_eq!(report["files_fixed"], 1);
    assert_eq!(report["total_diagnostics"], 0);
    assert_eq!(report["success"], true);
}

#[test]
fn output_flag_writes_the_plain_text_report_to_a_file_instead_of_stdout() {
    let dir = tempfile::tempdir().expect("failed to create temp dir");
    write_file(
        &dir.path().join("scripts/source/Example.psc"),
        "ScriptName Example   \n",
    );
    write_file(
        &dir.path().join("sources.achlist"),
        r#"["scripts/source/Example.psc"]"#,
    );
    let achlist_path = dir.path().join("sources.achlist");
    let output_path = dir.path().join("report.txt");

    let (code, stdout, stderr) = run_captured(&[
        "--output".to_string(),
        output_path.to_string_lossy().into_owned(),
        achlist_path.to_string_lossy().into_owned(),
    ]);

    assert_eq!(code, 0);
    assert!(stderr.is_empty());
    assert!(stdout.is_empty());
    let contents = fs::read_to_string(&output_path).expect("output file should exist");
    assert!(contents.contains("[trailing-whitespace]"));
    assert!(contents.contains("1 problem(s) found in 1 of 1 script(s)"));
}

#[test]
fn output_flag_writes_the_json_report_to_a_file_instead_of_stdout() {
    let dir = tempfile::tempdir().expect("failed to create temp dir");
    write_file(
        &dir.path().join("scripts/source/Example.psc"),
        "ScriptName Example   \n",
    );
    write_file(
        &dir.path().join("sources.achlist"),
        r#"["scripts/source/Example.psc"]"#,
    );
    let achlist_path = dir.path().join("sources.achlist");
    let output_path = dir.path().join("report.json");

    let (code, stdout, stderr) = run_captured(&[
        "--json".to_string(),
        "--output".to_string(),
        output_path.to_string_lossy().into_owned(),
        achlist_path.to_string_lossy().into_owned(),
    ]);

    assert_eq!(code, 0);
    assert!(stderr.is_empty());
    assert!(stdout.is_empty());
    let contents = fs::read_to_string(&output_path).expect("output file should exist");
    let report: serde_json::Value =
        serde_json::from_str(&contents).expect("output file should contain a JSON document");
    assert_eq!(report["success"], true);
    assert_eq!(report["total_diagnostics"], 1);
}

#[test]
fn output_flag_combines_with_fix() {
    let dir = tempfile::tempdir().expect("failed to create temp dir");
    write_file(
        &dir.path().join("scripts/source/Example.psc"),
        "ScriptName Example   \n",
    );
    write_file(
        &dir.path().join("sources.achlist"),
        r#"["scripts/source/Example.psc"]"#,
    );
    let achlist_path = dir.path().join("sources.achlist");
    let output_path = dir.path().join("report.txt");

    let (code, stdout, _stderr) = run_captured(&[
        "fix".to_string(),
        "--output".to_string(),
        output_path.to_string_lossy().into_owned(),
        achlist_path.to_string_lossy().into_owned(),
    ]);

    assert_eq!(code, 0);
    assert!(stdout.is_empty());
    assert_eq!(
        fs::read_to_string(dir.path().join("scripts/source/Example.psc")).unwrap(),
        "ScriptName Example\n"
    );
    let contents = fs::read_to_string(&output_path).expect("output file should exist");
    assert!(contents.contains("no problems found in 1 script"));
    assert!(contents.contains("(1 script(s) fixed.)"));
}

#[test]
fn output_flag_errors_when_the_directory_does_not_exist() {
    let dir = tempfile::tempdir().expect("failed to create temp dir");
    write_file(
        &dir.path().join("scripts/source/Example.psc"),
        "ScriptName Example\n",
    );
    write_file(
        &dir.path().join("sources.achlist"),
        r#"["scripts/source/Example.psc"]"#,
    );
    let achlist_path = dir.path().join("sources.achlist");
    let output_path = dir.path().join("missing-dir/report.txt");

    let (code, _stdout, stderr) = run_captured(&[
        "--output".to_string(),
        output_path.to_string_lossy().into_owned(),
        achlist_path.to_string_lossy().into_owned(),
    ]);

    assert_eq!(code, 2);
    assert!(stderr.starts_with("error: failed to write"));
}

#[test]
fn output_flag_without_a_value_prints_usage() {
    let (code, _stdout, stderr) = run_captured(&["--output".to_string()]);

    assert_eq!(code, 2);
    assert!(stderr.contains("Usage: PapyrusLinterCLI"));
}

#[test]
fn progress_flag_requires_output_in_plain_text_mode() {
    let dir = tempfile::tempdir().expect("failed to create temp dir");
    write_file(
        &dir.path().join("scripts/source/Example.psc"),
        "ScriptName Example\n",
    );
    write_file(
        &dir.path().join("sources.achlist"),
        r#"["scripts/source/Example.psc"]"#,
    );
    let achlist_path = dir.path().join("sources.achlist");

    let (code, stdout, stderr) = run_captured(&[
        "--progress".to_string(),
        achlist_path.to_string_lossy().into_owned(),
    ]);

    assert_eq!(code, 2);
    assert!(stdout.is_empty());
    assert!(stderr.contains("--progress requires --output"));
}

#[test]
fn progress_flag_requires_output_in_json_mode() {
    let dir = tempfile::tempdir().expect("failed to create temp dir");
    write_file(
        &dir.path().join("scripts/source/Example.psc"),
        "ScriptName Example\n",
    );
    write_file(
        &dir.path().join("sources.achlist"),
        r#"["scripts/source/Example.psc"]"#,
    );
    let achlist_path = dir.path().join("sources.achlist");

    let (code, stdout, stderr) = run_captured(&[
        "--json".to_string(),
        "--progress".to_string(),
        achlist_path.to_string_lossy().into_owned(),
    ]);

    assert_eq!(code, 2);
    assert!(stdout.is_empty());
    assert!(stderr.contains("--progress requires --output"));
}

#[test]
fn progress_flag_prints_a_progress_bar_to_stdout_when_output_is_set() {
    let dir = tempfile::tempdir().expect("failed to create temp dir");
    write_file(
        &dir.path().join("scripts/source/One.psc"),
        "ScriptName One\n",
    );
    write_file(
        &dir.path().join("scripts/source/Two.psc"),
        "ScriptName Two\n",
    );
    write_file(
        &dir.path().join("sources.achlist"),
        r#"["scripts/source/One.psc", "scripts/source/Two.psc"]"#,
    );
    let achlist_path = dir.path().join("sources.achlist");
    let output_path = dir.path().join("report.txt");

    let (code, stdout, stderr) = run_captured(&[
        "--progress".to_string(),
        "--output".to_string(),
        output_path.to_string_lossy().into_owned(),
        achlist_path.to_string_lossy().into_owned(),
    ]);

    assert_eq!(code, 0);
    assert!(stderr.is_empty());
    assert!(stdout.contains("\rLinting: 1/2 files"));
    assert!(stdout.contains("\rLinting: 2/2 files"));
    assert!(stdout.ends_with('\n'));
    let contents = fs::read_to_string(&output_path).expect("output file should exist");
    assert!(contents.contains("no problems found in 2 script"));
}

#[test]
fn plain_text_report_is_uncolored_when_stdout_is_not_a_terminal() {
    let dir = tempfile::tempdir().expect("failed to create temp dir");
    let script_path = dir.path().join("Example.psc");
    write_file(&script_path, "ScriptName Example   \n");

    let (code, stdout, _stderr) =
        run_captured_with_terminal_stdout(&[script_path.to_string_lossy().into_owned()], false);

    assert_eq!(code, 0);
    assert!(stdout.contains("[trailing-whitespace]"));
    assert!(!stdout.contains('\x1b'));
}

#[test]
fn color_auto_colorizes_when_stdout_is_a_terminal() {
    let dir = tempfile::tempdir().expect("failed to create temp dir");
    let script_path = dir.path().join("Example.psc");
    write_file(&script_path, "ScriptName Example   \n");

    let (code, stdout, _stderr) =
        run_captured_with_terminal_stdout(&[script_path.to_string_lossy().into_owned()], true);

    assert_eq!(code, 0);
    assert!(stdout.contains('\x1b'));
    // The rule id and level tag both still appear verbatim inside the
    // colorized escapes, so consumers scraping for them (and the other
    // tests here) still find them.
    assert!(stdout.contains("[trailing-whitespace]"));
    assert!(stdout.contains("[warning]"));
}

#[test]
fn color_never_disables_color_even_on_a_terminal() {
    let dir = tempfile::tempdir().expect("failed to create temp dir");
    let script_path = dir.path().join("Example.psc");
    write_file(&script_path, "ScriptName Example   \n");

    let (code, stdout, _stderr) = run_captured_with_terminal_stdout(
        &[
            "--color".to_string(),
            "never".to_string(),
            script_path.to_string_lossy().into_owned(),
        ],
        true,
    );

    assert_eq!(code, 0);
    assert!(!stdout.contains('\x1b'));
}

#[test]
fn color_always_enables_color_even_without_a_terminal() {
    let dir = tempfile::tempdir().expect("failed to create temp dir");
    let script_path = dir.path().join("Example.psc");
    write_file(&script_path, "ScriptName Example   \n");

    let (code, stdout, _stderr) = run_captured(&[
        "--color=always".to_string(),
        script_path.to_string_lossy().into_owned(),
    ]);

    assert_eq!(code, 0);
    assert!(stdout.contains('\x1b'));
}

#[test]
fn color_auto_does_not_colorize_a_file_written_via_output() {
    let dir = tempfile::tempdir().expect("failed to create temp dir");
    let script_path = dir.path().join("Example.psc");
    write_file(&script_path, "ScriptName Example   \n");
    let output_path = dir.path().join("report.txt");

    let (code, _stdout, _stderr) = run_captured_with_terminal_stdout(
        &[
            "--output".to_string(),
            output_path.to_string_lossy().into_owned(),
            script_path.to_string_lossy().into_owned(),
        ],
        true,
    );

    assert_eq!(code, 0);
    let contents = fs::read_to_string(&output_path).expect("output file should exist");
    assert!(!contents.contains('\x1b'));
}

#[test]
fn color_flag_rejects_an_unknown_value() {
    let dir = tempfile::tempdir().expect("failed to create temp dir");
    let script_path = dir.path().join("Example.psc");
    write_file(&script_path, "ScriptName Example\n");

    let (code, _stdout, stderr) = run_captured(&[
        "--color".to_string(),
        "rainbow".to_string(),
        script_path.to_string_lossy().into_owned(),
    ]);

    assert_eq!(code, 2);
    assert!(stderr.contains("--color must be"));
}

#[test]
fn color_flag_without_a_value_prints_usage() {
    let (code, _stdout, stderr) = run_captured(&["--color".to_string()]);

    assert_eq!(code, 2);
    assert!(stderr.contains("Usage: PapyrusLinterCLI"));
}

#[test]
fn json_output_is_never_colorized_even_when_color_is_always() {
    let dir = tempfile::tempdir().expect("failed to create temp dir");
    let script_path = dir.path().join("Example.psc");
    write_file(&script_path, "ScriptName Example   \n");

    let (code, stdout, stderr) = run_captured_with_terminal_stdout(
        &[
            "--json".to_string(),
            "--color=always".to_string(),
            script_path.to_string_lossy().into_owned(),
        ],
        true,
    );

    assert_eq!(code, 0);
    assert!(stderr.is_empty());
    assert!(!stdout.contains('\x1b'));
    let report: serde_json::Value =
        serde_json::from_str(&stdout).expect("colored JSON would not parse");
    assert_eq!(report["total_diagnostics"], 1);
}

#[test]
fn diagnostic_formatter_colorizes_each_structural_part() {
    let diagnostic = papyrus_lints::Diagnostic {
        line: 4,
        column: 7,
        rule: "example-rule",
        message: "[warning] example message".to_string(),
    };

    let formatted = format_diagnostic_line("Example.psc", &diagnostic, true);

    assert!(formatted.contains("\x1b[1mExample.psc:4:7\x1b[0m"));
    assert!(formatted.contains("\x1b[2m[example-rule]\x1b[0m"));
    assert!(formatted.contains("\x1b[33m[warning]\x1b[0m example message"));
}

#[test]
fn diagnostic_formatter_preserves_an_untagged_message() {
    let diagnostic = papyrus_lints::Diagnostic {
        line: 1,
        column: 2,
        rule: "example-rule",
        message: "example message without a level tag".to_string(),
    };

    let formatted = format_diagnostic_line("Example.psc", &diagnostic, true);

    assert!(formatted.ends_with("example message without a level tag"));
    assert!(!formatted.contains("\x1b[31m[error]"));
}

#[test]
fn display_path_leaves_paths_outside_the_project_root_unchanged() {
    let path = Path::new("other-project/scripts/source/Example.psc");

    assert_eq!(
        display_path(path, Path::new("project"), true),
        path.display().to_string()
    );
}

#[test]
fn a_psc_path_that_is_a_directory_reports_a_read_error() {
    let dir = tempfile::tempdir().expect("failed to create temp dir");
    let misleading_path = dir.path().join("NotAFile.psc");
    fs::create_dir(&misleading_path).expect("failed to create directory");

    let (code, stdout, stderr) = run_captured(&[misleading_path.to_string_lossy().into_owned()]);

    assert_eq!(code, 2);
    assert!(stdout.is_empty());
    assert!(stderr.contains("error: failed to read"));
    assert!(stderr.contains("NotAFile.psc"));
}

#[test]
fn doctor_reports_usage_error_without_a_path() {
    let (code, stdout, stderr) = run_captured(&["doctor".to_string()]);

    assert_eq!(code, 2);
    assert!(stdout.is_empty());
    assert!(stderr.contains("Usage: PapyrusLinterCLI"));
}

#[test]
fn doctor_reports_ok_for_an_existing_psc_with_no_config() {
    let dir = tempfile::tempdir().expect("failed to create temp dir");
    let source_dir = dir.path().join("scripts/source");
    let script = source_dir.join("Example.psc");
    write_file(&script, "ScriptName Example\n");

    let (code, stdout, stderr) =
        run_captured(&["doctor".to_string(), script.to_string_lossy().into_owned()]);

    assert_eq!(code, 0);
    assert!(stderr.is_empty());
    assert!(stdout.contains(&format!("[ok] script {} exists", script.display())));
    assert!(stdout.contains("scripts/source"));
    assert!(stdout.contains("no problems found"));
}

#[test]
fn doctor_reports_error_for_a_missing_psc_file() {
    let dir = tempfile::tempdir().expect("failed to create temp dir");
    let missing = dir.path().join("scripts/source/Missing.psc");

    let (code, stdout, _stderr) =
        run_captured(&["doctor".to_string(), missing.to_string_lossy().into_owned()]);

    assert_eq!(code, 1);
    assert!(stdout.contains(&format!(
        "[error] script {} does not exist",
        missing.display()
    )));
    assert!(stdout.contains("problem(s) found"));
}

#[test]
fn doctor_reports_each_missing_achlist_entry() {
    let dir = tempfile::tempdir().expect("failed to create temp dir");
    let existing = dir.path().join("scripts/source/Present.psc");
    write_file(&existing, "ScriptName Present\n");
    let achlist_path = dir.path().join("project.achlist");
    write_file(
        &achlist_path,
        r#"["scripts/source/Present.psc", "scripts/source/Missing.psc"]"#,
    );

    let (code, stdout, _stderr) = run_captured(&[
        "doctor".to_string(),
        achlist_path.to_string_lossy().into_owned(),
    ]);

    assert_eq!(code, 1);
    assert!(stdout.contains("achlist entry"));
    assert!(stdout.contains("Missing.psc"));
    assert!(stdout.contains("does not exist"));
    assert!(!stdout.contains("every entry"));
}

#[test]
fn doctor_reports_ok_when_every_achlist_entry_exists() {
    let dir = tempfile::tempdir().expect("failed to create temp dir");
    let existing = dir.path().join("scripts/source/Present.psc");
    write_file(&existing, "ScriptName Present\n");
    let achlist_path = dir.path().join("project.achlist");
    write_file(&achlist_path, r#"["scripts/source/Present.psc"]"#);

    let (code, stdout, _stderr) = run_captured(&[
        "doctor".to_string(),
        achlist_path.to_string_lossy().into_owned(),
    ]);

    assert_eq!(code, 0);
    assert!(stdout.contains("every entry"));
    assert!(stdout.contains("exists on disk (1 total)"));
}

#[test]
fn doctor_reports_error_when_the_achlist_itself_is_missing() {
    let dir = tempfile::tempdir().expect("failed to create temp dir");
    let missing_achlist = dir.path().join("missing.achlist");

    let (code, stdout, _stderr) = run_captured(&[
        "doctor".to_string(),
        missing_achlist.to_string_lossy().into_owned(),
    ]);

    assert_eq!(code, 1);
    assert!(stdout.contains("does not exist"));
}

#[test]
fn doctor_warns_when_no_conventional_script_directory_exists() {
    let dir = tempfile::tempdir().expect("failed to create temp dir");
    let achlist_path = dir.path().join("project.achlist");
    write_file(&achlist_path, "[]");

    let (code, stdout, _stderr) = run_captured(&[
        "doctor".to_string(),
        achlist_path.to_string_lossy().into_owned(),
    ]);

    assert_eq!(code, 1);
    assert!(stdout.contains("[warning] neither scripts/source nor source/scripts exists"));
}

#[test]
fn doctor_warns_about_a_missing_configured_additional_script_root() {
    let dir = tempfile::tempdir().expect("failed to create temp dir");
    let source_dir = dir.path().join("scripts/source");
    let script = source_dir.join("Example.psc");
    write_file(&script, "ScriptName Example\n");
    write_file(
        &dir.path().join("papyrus-lint.yaml"),
        "additional_script_roots:\n  - Shared\n",
    );

    let (code, stdout, _stderr) =
        run_captured(&["doctor".to_string(), script.to_string_lossy().into_owned()]);

    assert_eq!(code, 1);
    assert!(stdout.contains("configured additional script root"));
    assert!(stdout.contains("Shared"));
    assert!(stdout.contains("does not exist"));
}

#[test]
fn doctor_reports_ok_for_an_existing_configured_additional_script_root() {
    let dir = tempfile::tempdir().expect("failed to create temp dir");
    let source_dir = dir.path().join("scripts/source");
    let script = source_dir.join("Example.psc");
    write_file(&script, "ScriptName Example\n");
    fs::create_dir_all(dir.path().join("Shared")).expect("failed to create shared dir");
    write_file(
        &dir.path().join("papyrus-lint.yaml"),
        "additional_script_roots:\n  - Shared\n",
    );

    let (code, stdout, _stderr) =
        run_captured(&["doctor".to_string(), script.to_string_lossy().into_owned()]);

    assert_eq!(code, 0);
    assert!(stdout.contains("additional script root"));
    assert!(stdout.contains("Shared"));
    assert!(stdout.contains("exists"));
}

#[test]
fn doctor_via_script_root_flag_is_checked_even_without_a_config_file() {
    let dir = tempfile::tempdir().expect("failed to create temp dir");
    let source_dir = dir.path().join("scripts/source");
    let script = source_dir.join("Example.psc");
    write_file(&script, "ScriptName Example\n");

    let (code, stdout, _stderr) = run_captured(&[
        "doctor".to_string(),
        "--script-root".to_string(),
        "NotThere".to_string(),
        script.to_string_lossy().into_owned(),
    ]);

    assert_eq!(code, 1);
    assert!(stdout.contains("NotThere"));
    assert!(stdout.contains("does not exist"));
}

#[test]
fn doctor_reports_error_for_a_malformed_config_file() {
    let dir = tempfile::tempdir().expect("failed to create temp dir");
    let source_dir = dir.path().join("scripts/source");
    let script = source_dir.join("Example.psc");
    write_file(&script, "ScriptName Example\n");
    write_file(
        &dir.path().join("papyrus-lint.yaml"),
        "semicolon: [not, a, bool]\n",
    );

    let (code, stdout, _stderr) =
        run_captured(&["doctor".to_string(), script.to_string_lossy().into_owned()]);

    assert_eq!(code, 1);
    assert!(stdout.contains("[error] failed to load lint config"));
}

#[test]
fn doctor_reports_error_for_a_missing_explicit_config_path() {
    let dir = tempfile::tempdir().expect("failed to create temp dir");
    let source_dir = dir.path().join("scripts/source");
    let script = source_dir.join("Example.psc");
    write_file(&script, "ScriptName Example\n");
    let missing_config = dir.path().join("missing-config.yaml");

    let (code, stdout, _stderr) = run_captured(&[
        "doctor".to_string(),
        "--config".to_string(),
        missing_config.to_string_lossy().into_owned(),
        script.to_string_lossy().into_owned(),
    ]);

    assert_eq!(code, 1);
    assert!(stdout.contains("[error] failed to load lint config"));
    assert!(stdout.contains(&missing_config.display().to_string()));
}

#[test]
fn doctor_reports_error_for_a_configured_compiler_path_that_does_not_exist() {
    let dir = tempfile::tempdir().expect("failed to create temp dir");
    let source_dir = dir.path().join("scripts/source");
    let script = source_dir.join("Example.psc");
    write_file(&script, "ScriptName Example\n");
    write_file(
        &dir.path().join("papyrus-lint.yaml"),
        "compiler_path: /nonexistent/PapyrusCompiler.exe\n",
    );

    let (code, stdout, _stderr) =
        run_captured(&["doctor".to_string(), script.to_string_lossy().into_owned()]);

    assert_eq!(code, 1);
    assert!(stdout.contains("configured compiler_path"));
    assert!(stdout.contains("does not exist"));
}

#[test]
fn doctor_warns_when_compile_check_is_enabled_without_a_resolvable_compiler() {
    let dir = tempfile::tempdir().expect("failed to create temp dir");
    let source_dir = dir.path().join("scripts/source");
    let script = source_dir.join("Example.psc");
    write_file(&script, "ScriptName Example\n");
    write_file(
        &dir.path().join("papyrus-lint.yaml"),
        "compile_check: true\n",
    );

    let (code, stdout, _stderr) =
        run_captured(&["doctor".to_string(), script.to_string_lossy().into_owned()]);

    assert_eq!(code, 1);
    assert!(stdout.contains(
        "[warning] compile_check is enabled but no PapyrusCompiler.exe could be resolved"
    ));
}

#[cfg(unix)]
fn write_stub_compiler(dir: &Path, script: &str) -> PathBuf {
    use std::os::unix::fs::PermissionsExt;

    let path = dir.join("stub-compiler.sh");
    write_file(&path, script);
    fs::set_permissions(&path, fs::Permissions::from_mode(0o755))
        .expect("failed to make stub compiler executable");
    path
}

#[test]
#[cfg(unix)]
fn compile_check_merges_compiler_reported_errors_into_the_lint_report() {
    let dir = tempfile::tempdir().expect("failed to create temp dir");
    let source_dir = dir.path().join("scripts/source");
    let script = source_dir.join("Example.psc");
    write_file(&script, "ScriptName Example\n");
    let compiler_path = write_stub_compiler(
        dir.path(),
        "#!/bin/sh\necho \"Example.psc(3,4): custom compiler error\" >&2\nexit 1\n",
    );
    write_file(
        &dir.path().join("papyrus-lint.yaml"),
        &format!(
            "compile_check: true\ncompiler_path: {}\n",
            compiler_path.display()
        ),
    );

    let (code, stdout, _stderr) =
        run_captured(&["--json".to_string(), script.to_string_lossy().into_owned()]);

    assert_eq!(code, 1);
    assert!(stdout.contains("\"rule\": \"compiler-error\""));
    assert!(stdout.contains("custom compiler error"));
}

#[test]
#[cfg(unix)]
fn compile_check_disabled_by_default_ignores_a_failing_compiler() {
    let dir = tempfile::tempdir().expect("failed to create temp dir");
    let source_dir = dir.path().join("scripts/source");
    let script = source_dir.join("Example.psc");
    write_file(&script, "ScriptName Example\n");
    let compiler_path = write_stub_compiler(
        dir.path(),
        "#!/bin/sh\necho \"Example.psc(3,4): custom compiler error\" >&2\nexit 1\n",
    );
    // No `compile_check: true`, only a configured `compiler_path` — the
    // compiler must never be invoked at all when the setting is off.
    write_file(
        &dir.path().join("papyrus-lint.yaml"),
        &format!("compiler_path: {}\n", compiler_path.display()),
    );

    let (code, stdout, _stderr) =
        run_captured(&["--json".to_string(), script.to_string_lossy().into_owned()]);

    assert_eq!(code, 0);
    assert!(!stdout.contains("compiler-error"));
}

#[test]
#[cfg(unix)]
fn compile_check_is_ignored_when_no_compiler_path_can_be_resolved() {
    let dir = tempfile::tempdir().expect("failed to create temp dir");
    let source_dir = dir.path().join("scripts/source");
    let script = source_dir.join("Example.psc");
    write_file(&script, "ScriptName Example\n");
    write_file(
        &dir.path().join("papyrus-lint.yaml"),
        "compile_check: true\n",
    );

    let (code, stdout, _stderr) =
        run_captured(&["--json".to_string(), script.to_string_lossy().into_owned()]);

    assert_eq!(code, 0);
    assert!(!stdout.contains("compiler-error"));
}

#[test]
fn doctor_warns_when_a_scanned_directory_has_no_psc_files() {
    let dir = tempfile::tempdir().expect("failed to create temp dir");
    fs::create_dir_all(dir.path().join("empty")).expect("failed to create empty dir");

    let (code, stdout, _stderr) = run_captured(&[
        "doctor".to_string(),
        dir.path().join("empty").to_string_lossy().into_owned(),
    ]);

    assert_eq!(code, 1);
    assert!(stdout.contains("[warning] no .psc files found under"));
}

#[test]
fn doctor_json_reports_the_full_check_list_and_success_flag() {
    let dir = tempfile::tempdir().expect("failed to create temp dir");
    let source_dir = dir.path().join("scripts/source");
    let script = source_dir.join("Example.psc");
    write_file(&script, "ScriptName Example\n");

    let (code, stdout, stderr) = run_captured(&[
        "doctor".to_string(),
        "--json".to_string(),
        script.to_string_lossy().into_owned(),
    ]);

    assert_eq!(code, 0);
    assert!(stderr.is_empty());
    let report: serde_json::Value =
        serde_json::from_str(&stdout).expect("doctor --json output should be valid JSON");
    assert_eq!(report["success"], serde_json::Value::Bool(true));
    assert!(report["checks"]
        .as_array()
        .is_some_and(|checks| !checks.is_empty()));
    assert!(report["checks"]
        .as_array()
        .unwrap()
        .iter()
        .all(|check| check["status"] == "ok"));
}

#[test]
fn direct_psc_detection_is_case_insensitive() {
    let dir = tempfile::tempdir().expect("failed to create temp dir");
    let script = dir.path().join("Example.PSC");
    write_file(&script, "ScriptName Example   \n");

    let (code, stdout, stderr) = run_captured(&[script.to_string_lossy().into_owned()]);

    assert_eq!(code, 0, "stderr: {stderr}");
    assert!(stderr.is_empty());
    assert!(stdout.contains("[trailing-whitespace]"));
    assert!(stdout.contains("1 problem(s) found in 1 of 1 script(s)"));
}

#[test]
fn doctor_reports_a_malformed_achlist_and_continues_other_checks() {
    let dir = tempfile::tempdir().expect("failed to create temp dir");
    let achlist = dir.path().join("broken.achlist");
    write_file(&achlist, "not json");

    let (code, stdout, stderr) =
        run_captured(&["doctor".to_string(), achlist.to_string_lossy().into_owned()]);

    assert_eq!(code, 1);
    assert!(stderr.is_empty());
    assert!(stdout.contains("[error] failed to parse achlist"));
    assert!(stdout.contains("[ok] project root resolved to"));
    assert!(stdout.contains("PapyrusLinterCLI doctor:"));
}

#[test]
fn doctor_rejects_missing_flag_values() {
    for flag in ["--config", "--script-root"] {
        let (code, stdout, stderr) = run_captured(&["doctor".to_string(), flag.to_string()]);

        assert_eq!(code, 2, "flag: {flag}");
        assert!(stdout.is_empty(), "flag: {flag}");
        assert!(stderr.contains("Usage: PapyrusLinterCLI"), "flag: {flag}");
    }
}

#[test]
fn doctor_rejects_extra_positional_arguments() {
    let (code, stdout, stderr) = run_captured(&[
        "doctor".to_string(),
        "first.psc".to_string(),
        "second.psc".to_string(),
    ]);

    assert_eq!(code, 2);
    assert!(stdout.is_empty());
    assert!(stderr.contains("Usage: PapyrusLinterCLI"));
}

#[test]
fn output_replaces_an_existing_report_instead_of_appending() {
    let dir = tempfile::tempdir().expect("failed to create temp dir");
    let script = dir.path().join("Example.psc");
    let report = dir.path().join("report.txt");
    write_file(&script, "ScriptName Example\n");
    write_file(&report, "stale report contents that must disappear\n");

    let (code, stdout, stderr) = run_captured(&[
        "--output".to_string(),
        report.to_string_lossy().into_owned(),
        script.to_string_lossy().into_owned(),
    ]);

    assert_eq!(code, 0, "stderr: {stderr}");
    assert!(stdout.is_empty());
    assert!(stderr.is_empty());
    assert_eq!(
        fs::read_to_string(report).expect("failed to read report"),
        "PapyrusLinterCLI: no problems found in 1 script(s).\n"
    );
}

#[test]
fn ai_format_reports_source_metadata_counts_and_rule_details() {
    let dir = tempfile::tempdir().expect("failed to create temp dir");
    let dirty = dir.path().join("Dirty.psc");
    let clean = dir.path().join("Clean.psc");
    let dirty_source = "ScriptName Dirty   \n";
    write_file(&dirty, dirty_source);
    write_file(&clean, "ScriptName Clean\n");

    let (code, stdout, stderr) = run_captured(&[
        "--format=ai".to_string(),
        dir.path().to_string_lossy().into_owned(),
    ]);

    assert_eq!(code, 0, "stderr: {stderr}");
    assert!(stderr.is_empty());
    let report: serde_json::Value =
        serde_json::from_str(&stdout).expect("AI report should be valid JSON");
    assert_eq!(
        report["$schema"],
        "https://papyrus-lint.idrinth.de/schema/papyrus-lint-ai-export.v3.schema.json"
    );
    assert_eq!(report["header"]["tool"], "Papyrus Lint");
    assert_eq!(report["header"]["target_game"], "Skyrim SE/AE");
    assert_eq!(report["findings"]["total_diagnostics"], 1);
    assert_eq!(report["findings"]["severity_counts"]["warnings"], 1);
    assert_eq!(report["findings"]["rule_counts"]["trailing-whitespace"], 1);
    assert_eq!(report["findings"]["files"].as_array().unwrap().len(), 1);
    assert_eq!(report["findings"]["files"][0]["source"]["type"], "content");
    assert_eq!(
        report["findings"]["files"][0]["source"]["content"],
        dirty_source
    );
    assert_eq!(report["rule_details"][0]["rule"], "trailing-whitespace");
    assert_eq!(report["rule_details"][0]["auto_fixable"], true);
    assert!(report["configuration"]["enabled_rules"].is_array());
    assert!(report["configuration"].get("rules").is_none());
}

#[test]
fn ai_hash_source_replaces_script_contents_with_an_md5_digest() {
    let dir = tempfile::tempdir().expect("failed to create temp dir");
    let script = dir.path().join("Example.psc");
    let source = "ScriptName Example   \n";
    write_file(&script, source);

    let (code, stdout, stderr) = run_captured(&[
        "--format".to_string(),
        "ai".to_string(),
        "--hash-source".to_string(),
        script.to_string_lossy().into_owned(),
    ]);

    assert_eq!(code, 0, "stderr: {stderr}");
    let report: serde_json::Value =
        serde_json::from_str(&stdout).expect("AI report should be valid JSON");
    let source_report = &report["findings"]["files"][0]["source"];
    assert_eq!(source_report["type"], "hash");
    assert_eq!(source_report["algorithm"], "md5");
    assert_eq!(source_report["hash"], content_hash::md5_hex(source));
    assert!(source_report.get("content").is_none());
    assert!(!stdout.contains(source));
}

#[test]
fn hash_source_without_ai_format_is_a_usage_error() {
    let (code, stdout, stderr) =
        run_captured(&["--hash-source".to_string(), "Example.psc".to_string()]);

    assert_eq!(code, 2);
    assert!(stdout.is_empty());
    assert_eq!(stderr, "error: --hash-source requires --format ai\n");
}

#[test]
fn format_flag_rejects_unknown_values_and_conflicts_with_json() {
    let (unknown_code, unknown_stdout, unknown_stderr) =
        run_captured(&["--format=yaml".to_string(), "Example.psc".to_string()]);
    assert_eq!(unknown_code, 2);
    assert!(unknown_stdout.is_empty());
    assert_eq!(
        unknown_stderr,
        "error: --format must be 'plain', 'json', or 'ai', got 'yaml'\n"
    );

    let (conflict_code, conflict_stdout, conflict_stderr) = run_captured(&[
        "--json".to_string(),
        "--format=json".to_string(),
        "Example.psc".to_string(),
    ]);
    assert_eq!(conflict_code, 2);
    assert!(conflict_stdout.is_empty());
    assert_eq!(
        conflict_stderr,
        "error: --json and --format can't be combined\n"
    );
}

#[test]
fn value_flags_report_usage_when_their_separate_value_is_missing() {
    for flag in ["--line", "--tag", "--format", "--threads"] {
        let (code, stdout, stderr) = run_captured(&[flag.to_string()]);

        assert_eq!(code, 2, "unexpected exit code for {flag}");
        assert!(stdout.is_empty(), "unexpected stdout for {flag}");
        assert_eq!(stderr, USAGE, "unexpected stderr for {flag}");
    }
}

#[test]
fn level_colors_cover_info_and_unknown_diagnostic_levels() {
    assert_eq!(level_color("info"), ANSI_CYAN);
    assert_eq!(level_color("notice"), ANSI_RESET);

    let info = papyrus_lints::Diagnostic {
        line: 2,
        column: 3,
        rule: "example-rule",
        message: "[info] informational diagnostic".to_string(),
    };
    let formatted = format_diagnostic_line("Example.psc", &info, true);

    assert!(formatted.contains(&format!("{ANSI_CYAN}[info]{ANSI_RESET}")));
    assert!(formatted.contains(" informational diagnostic"));
}

#[test]
fn doctor_reports_positive_directory_and_explicit_path_checks() {
    let dir = tempfile::tempdir().expect("failed to create temp dir");
    let project = dir.path().join("project");
    let scripts = project.join("scripts/source");
    let external = dir.path().join("shared-scripts");
    let compiler = dir.path().join("PapyrusCompiler.exe");
    let config_path = dir.path().join("doctor-config.yaml");
    write_file(&scripts.join("Example.psc"), "ScriptName Example\n");
    fs::create_dir_all(&external).expect("failed to create external script root");
    write_file(&compiler, "compiler fixture");
    write_file(
        &project.join("papyrus-lint.yaml"),
        &format!("compiler_path: {}\n", compiler.display()),
    );
    write_file(&config_path, "trailing_whitespace: true\n");

    let (code, stdout, stderr) = run_captured(&[
        "doctor".to_string(),
        "--config".to_string(),
        config_path.to_string_lossy().into_owned(),
        "--script-root".to_string(),
        external.to_string_lossy().into_owned(),
        project.to_string_lossy().into_owned(),
    ]);

    assert_eq!(code, 0);
    assert!(stderr.is_empty());
    assert!(stdout.contains("found 1 .psc file(s) under"));
    assert!(stdout.contains("explicit config"));
    assert!(stdout.contains("additional script root"));
    assert!(stdout.contains("configured compiler_path"));
    assert!(stdout.contains("no problems found"));
}
