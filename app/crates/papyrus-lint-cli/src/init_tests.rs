use super::*;
use crate::args::{parse_init_preset, parse_preset_add_args, InitPresetError, PresetAddArgsError};
use crate::test_support::*;
use papyrus_lint_config::{self as config, presets};
use std::fs;
use std::path::PathBuf;

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
        Err(presets::AddPresetError::AlreadyExists(PathBuf::from(
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
        Err(presets::AddPresetError::InvalidName("strict".to_string())),
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
fn run_preset_list_prints_the_built_in_names_when_no_user_presets_exist() {
    // No `presets` directory exists next to the test binary, so this only
    // lists the three built-ins (see `presets::user_presets_dir`).
    let mut stdout = Vec::new();

    let code = run_preset_list(&mut stdout);

    assert_eq!(code, 0);
    assert_eq!(
        String::from_utf8(stdout).unwrap(),
        "strict\nstandard\ncareful\n"
    );
}

#[test]
fn preset_list_prints_the_built_in_names_through_run() {
    let (code, stdout, stderr) = run_captured(&["preset".to_string(), "list".to_string()]);

    assert_eq!(code, 0);
    assert!(stderr.is_empty());
    assert_eq!(stdout, "strict\nstandard\ncareful\n");
}

#[test]
fn preset_list_rejects_extra_arguments() {
    let (code, stdout, stderr) = run_captured(&[
        "preset".to_string(),
        "list".to_string(),
        "extra".to_string(),
    ]);

    assert_eq!(code, 2);
    assert!(stdout.is_empty());
    assert!(stderr.contains("Usage: PapyrusLinterCLI"));
}

#[test]
fn init_creates_a_default_config() {
    let dir = tempfile::tempdir().expect("failed to create temp dir");
    let mut stdout = Vec::new();
    let mut stderr = Vec::new();

    let code = initialize_config(
        dir.path(),
        presets::Preset::default(),
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
        presets::Preset::default(),
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
fn init_seeds_additional_script_roots_from_a_ppj_in_the_same_directory() {
    let dir = tempfile::tempdir().expect("failed to create temp dir");
    write_file(
        &dir.path().join("Project.ppj"),
        r#"<PapyrusProject>
    <Imports>
        <Import>Source/Scripts</Import>
        <Import>C:\Games\Skyrim\Data\Source\Scripts</Import>
    </Imports>
</PapyrusProject>"#,
    );
    let mut stdout = Vec::new();
    let mut stderr = Vec::new();

    let code = initialize_config(
        dir.path(),
        presets::Preset::default(),
        &mut stdout,
        &mut stderr,
    );

    assert_eq!(code, 0);
    assert!(String::from_utf8(stderr).unwrap().is_empty());
    assert!(String::from_utf8(stdout)
        .unwrap()
        .contains("Seeded additional_script_roots from"));
    let roots = config::load_script_roots(dir.path()).expect("roots should load");
    assert_eq!(
        roots,
        vec![
            "Source/Scripts".to_string(),
            "C:/Games/Skyrim/Data/Source/Scripts".to_string(),
        ]
    );
}

#[test]
fn init_does_not_seed_script_roots_without_a_ppj_file() {
    let dir = tempfile::tempdir().expect("failed to create temp dir");
    let mut stdout = Vec::new();
    let mut stderr = Vec::new();

    let code = initialize_config(
        dir.path(),
        presets::Preset::default(),
        &mut stdout,
        &mut stderr,
    );

    assert_eq!(code, 0);
    assert!(!String::from_utf8(stdout)
        .unwrap()
        .contains("Seeded additional_script_roots"));
    let roots = config::load_script_roots(dir.path()).expect("roots should load");
    assert!(roots.is_empty());
}

#[test]
fn find_ppj_returns_none_when_the_directory_cannot_be_read() {
    let dir = tempfile::tempdir().expect("failed to create temp dir");
    let missing = dir.path().join("missing");

    assert_eq!(find_ppj_in_dir(&missing), None);
}

#[test]
fn init_uses_the_first_ppj_alphabetically_and_ignores_nested_projects() {
    let dir = tempfile::tempdir().expect("failed to create temp dir");
    let nested = dir.path().join("nested");
    fs::create_dir(&nested).expect("failed to create nested directory");
    write_file(
        &dir.path().join("z-last.ppj"),
        "<PapyrusProject><Imports><Import>Last</Import></Imports></PapyrusProject>",
    );
    write_file(
        &dir.path().join("A-first.PPJ"),
        "<PapyrusProject><Imports><Import>First</Import></Imports></PapyrusProject>",
    );
    write_file(
        &nested.join("0-nested.ppj"),
        "<PapyrusProject><Imports><Import>Nested</Import></Imports></PapyrusProject>",
    );
    let mut stdout = Vec::new();
    let mut stderr = Vec::new();

    let code = initialize_config(
        dir.path(),
        presets::Preset::default(),
        &mut stdout,
        &mut stderr,
    );

    assert_eq!(code, 0);
    assert!(stderr.is_empty());
    assert_eq!(
        config::load_script_roots(dir.path()).expect("roots should load"),
        vec!["First".to_string()]
    );
}

#[test]
fn seed_from_ppj_preserves_existing_script_roots() {
    let dir = tempfile::tempdir().expect("failed to create temp dir");
    write_file(
        &dir.path().join("papyrus-lint.yaml"),
        "additional_script_roots:\n  - Existing\n",
    );
    write_file(
        &dir.path().join("Project.ppj"),
        "<PapyrusProject><Imports><Import>Imported</Import></Imports></PapyrusProject>",
    );
    let mut stdout = Vec::new();
    let mut stderr = Vec::new();

    seed_additional_script_roots_from_ppj(dir.path(), &mut stdout, &mut stderr);

    assert!(stdout.is_empty());
    assert!(stderr.is_empty());
    assert_eq!(
        config::load_script_roots(dir.path()).expect("roots should load"),
        vec!["Existing".to_string()]
    );
}

#[test]
fn seed_from_ppj_warns_when_the_existing_config_cannot_be_read() {
    let dir = tempfile::tempdir().expect("failed to create temp dir");
    write_file(&dir.path().join("papyrus-lint.yaml"), "rules: [\n");
    write_file(
        &dir.path().join("Project.ppj"),
        "<PapyrusProject><Imports><Import>Imported</Import></Imports></PapyrusProject>",
    );
    let mut stdout = Vec::new();
    let mut stderr = Vec::new();

    seed_additional_script_roots_from_ppj(dir.path(), &mut stdout, &mut stderr);

    assert!(stdout.is_empty());
    assert!(String::from_utf8(stderr)
        .unwrap()
        .contains("failed to read additional_script_roots"));
}

#[test]
fn init_does_not_report_seeding_for_a_ppj_without_imports() {
    let dir = tempfile::tempdir().expect("failed to create temp dir");
    write_file(
        &dir.path().join("Project.ppj"),
        "<PapyrusProject><Imports /></PapyrusProject>",
    );
    let mut stdout = Vec::new();
    let mut stderr = Vec::new();

    let code = initialize_config(
        dir.path(),
        presets::Preset::default(),
        &mut stdout,
        &mut stderr,
    );

    assert_eq!(code, 0);
    assert!(stderr.is_empty());
    assert!(!String::from_utf8(stdout)
        .unwrap()
        .contains("Seeded additional_script_roots"));
    assert!(config::load_script_roots(dir.path())
        .expect("roots should load")
        .is_empty());
}

#[test]
fn init_warns_but_still_succeeds_on_an_unparseable_ppj() {
    let dir = tempfile::tempdir().expect("failed to create temp dir");
    write_file(&dir.path().join("broken.ppj"), "not xml at all <<<");
    let mut stdout = Vec::new();
    let mut stderr = Vec::new();

    let code = initialize_config(
        dir.path(),
        presets::Preset::default(),
        &mut stdout,
        &mut stderr,
    );

    assert_eq!(code, 0);
    assert!(String::from_utf8(stderr)
        .unwrap()
        .contains("failed to parse it"));
    let roots = config::load_script_roots(dir.path()).expect("roots should load");
    assert!(roots.is_empty());
}

#[test]
fn parse_init_preset_defaults_to_strict_when_no_flag_is_given() {
    assert_eq!(parse_init_preset(&["--game".to_string(), "skyrim".to_string()]), Ok((presets::Preset::Strict, papyrus_lints::Game::Skyrim)));
}

#[test]
fn parse_init_preset_accepts_the_flag_and_its_equals_form() {
    assert_eq!(
        parse_init_preset(&["--game".to_string(), "skyrim".to_string(), "--preset".to_string(), "careful".to_string()]),
        Ok((presets::Preset::Careful, papyrus_lints::Game::Skyrim))
    );
    assert_eq!(
        parse_init_preset(&["--game=skyrim".to_string(), "--preset=standard".to_string()]),
        Ok((presets::Preset::Standard, papyrus_lints::Game::Skyrim))
    );
}

#[test]
fn parse_init_preset_matches_names_case_insensitively() {
    assert_eq!(
        parse_init_preset(&["--game=skyrim".to_string(), "--preset".to_string(), "STANDARD".to_string()]),
        Ok((presets::Preset::Standard, papyrus_lints::Game::Skyrim))
    );
}

#[test]
fn parse_init_preset_accepts_a_name_that_is_not_a_built_in_as_a_custom_preset() {
    // Whether a name actually matches a user preset file is only
    // checked once `init` runs (see `presets::Preset::yaml`), not during
    // argument parsing, so an arbitrary non-blank name parses fine here.
    assert_eq!(
        parse_init_preset(&["--game=skyrim".to_string(), "--preset".to_string(), "lenient".to_string()]),
        Ok((presets::Preset::Custom("lenient".to_string()), papyrus_lints::Game::Skyrim))
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
        run_captured(&["init".to_string(), "--game=skyrim".to_string(), "--preset=lenient".to_string()]);

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

    let (preset, _game) =
        parse_init_preset(&["--game=skyrim".to_string(), "--preset=careful".to_string()]).expect("careful should parse");
    let code = initialize_config(dir.path(), preset, &mut stdout, &mut stderr);

    assert_eq!(code, 0);
    let generated = fs::read_to_string(dir.path().join("papyrus-lint.yaml"))
        .expect("failed to read generated config");
    assert!(generated.contains("cyclomatic_complexity_warning: 20\n"));
}
