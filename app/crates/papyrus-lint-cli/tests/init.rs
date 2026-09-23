//! End-to-end tests for `init` / `preset add` through the standalone
//! `PapyrusLinterCLI` binary. Unit tests live in `src/init.rs`.

mod common;

use common::*;
use std::fs;

#[test]
fn init_creates_a_config_in_the_process_working_directory() {
    let dir = tempfile::tempdir().expect("failed to create temp directory");

    let output = run_cli_in(&["init", "--game", "skyrim"], dir.path());

    assert!(output.status.success());
    assert!(output.stderr.is_empty());
    assert_eq!(
        String::from_utf8(output.stdout).expect("stdout should be UTF-8"),
        format!(
            "Created {}\n",
            dir.path().join("papyrus-lint.yaml").display()
        )
    );
    let config = fs::read_to_string(dir.path().join("papyrus-lint.yaml"))
        .expect("init should create papyrus-lint.yaml");
    assert!(config.contains("trailing_whitespace: true"));
}

#[test]
fn init_merges_a_config_placed_next_to_the_executable() {
    let exe_dir = tempfile::tempdir().expect("failed to create temp directory");
    write_file(
        &exe_dir.path().join("papyrus-lint.yaml"),
        "semicolon: true\n",
    );

    let project_dir = tempfile::tempdir().expect("failed to create temp directory");
    let output = run_copied_cli(
        exe_dir.path(),
        &["init", "--game", "skyrim"],
        project_dir.path(),
    );

    assert!(output.status.success());
    let config = fs::read_to_string(project_dir.path().join("papyrus-lint.yaml"))
        .expect("init should create papyrus-lint.yaml");
    assert!(config.contains("semicolon: true"));
    // Settings the base config didn't set still fall back to the default.
    assert!(config.contains("trailing_whitespace: true"));
}

#[test]
fn init_preset_flag_selects_a_user_preset_from_the_presets_directory() {
    let exe_dir = tempfile::tempdir().expect("failed to create temp directory");
    write_file(
        &exe_dir.path().join("presets/my-preset.yaml"),
        "semicolon: true\nrules:\n  identifier_casing: false\n",
    );

    let project_dir = tempfile::tempdir().expect("failed to create temp directory");
    let output = run_copied_cli(
        exe_dir.path(),
        &["init", "--game", "skyrim", "--preset", "my-preset"],
        project_dir.path(),
    );

    assert!(output.status.success());
    let config = fs::read_to_string(project_dir.path().join("papyrus-lint.yaml"))
        .expect("init should create papyrus-lint.yaml");
    assert!(config.contains("semicolon: true"));
    assert!(config.contains("  identifier_casing: false\n"));
    // Settings the user preset didn't set still fall back to the engine's
    // built-in default.
    assert!(config.contains("  trailing_whitespace: true\n"));
}

#[test]
fn init_preset_flag_reports_an_error_for_a_name_matching_no_built_in_or_user_preset() {
    let exe_dir = tempfile::tempdir().expect("failed to create temp directory");
    write_file(&exe_dir.path().join("presets/my-preset.yaml"), "");

    let project_dir = tempfile::tempdir().expect("failed to create temp directory");
    let output = run_copied_cli(
        exe_dir.path(),
        &["init", "--game", "skyrim", "--preset", "does-not-exist"],
        project_dir.path(),
    );

    assert_eq!(output.status.code(), Some(2));
    let stderr = String::from_utf8(output.stderr).expect("stderr should be UTF-8");
    assert!(stderr.contains("unknown preset 'does-not-exist'"));
    assert!(!project_dir.path().join("papyrus-lint.yaml").exists());
}

#[test]
fn preset_add_creates_a_new_user_preset_next_to_the_executable() {
    let exe_dir = tempfile::tempdir().expect("failed to create temp directory");
    let project_dir = tempfile::tempdir().expect("failed to create temp directory");
    let source = project_dir.path().join("papyrus-lint.yaml");
    write_file(&source, "semicolon: true\n");

    let output = run_copied_cli(
        exe_dir.path(),
        &["preset", "add", "my-team", &source.to_string_lossy()],
        project_dir.path(),
    );

    assert!(output.status.success());
    let preset_path = exe_dir.path().join("presets/my-team.yaml");
    assert_eq!(
        String::from_utf8(output.stdout).expect("stdout should be UTF-8"),
        format!("Added preset 'my-team' at {}\n", preset_path.display())
    );
    assert_eq!(
        fs::read_to_string(&preset_path).expect("preset file should have been created"),
        "semicolon: true\n"
    );
}

#[test]
fn preset_add_makes_the_preset_selectable_via_init() {
    let exe_dir = tempfile::tempdir().expect("failed to create temp directory");
    let source_dir = tempfile::tempdir().expect("failed to create temp directory");
    let source = source_dir.path().join("papyrus-lint.yaml");
    write_file(
        &source,
        "semicolon: true\nrules:\n  identifier_casing: false\n",
    );

    let add_output = run_copied_cli(
        exe_dir.path(),
        &["preset", "add", "my-team", &source.to_string_lossy()],
        source_dir.path(),
    );
    assert!(add_output.status.success());

    let project_dir = tempfile::tempdir().expect("failed to create temp directory");
    let init_output = run_copied_cli(
        exe_dir.path(),
        &["init", "--game", "skyrim", "--preset", "my-team"],
        project_dir.path(),
    );

    assert!(init_output.status.success());
    let config = fs::read_to_string(project_dir.path().join("papyrus-lint.yaml"))
        .expect("init should create papyrus-lint.yaml");
    assert!(config.contains("semicolon: true"));
    assert!(config.contains("  identifier_casing: false\n"));
}

#[test]
fn preset_add_refuses_to_overwrite_an_existing_preset_without_yes() {
    let exe_dir = tempfile::tempdir().expect("failed to create temp directory");
    write_file(
        &exe_dir.path().join("presets/my-team.yaml"),
        "semicolon: true\n",
    );

    let source_dir = tempfile::tempdir().expect("failed to create temp directory");
    let source = source_dir.path().join("papyrus-lint.yaml");
    write_file(&source, "semicolon: false\n");

    let output = run_copied_cli(
        exe_dir.path(),
        &["preset", "add", "my-team", &source.to_string_lossy()],
        source_dir.path(),
    );

    assert_eq!(output.status.code(), Some(2));
    let stderr = String::from_utf8(output.stderr).expect("stderr should be UTF-8");
    assert!(stderr.contains("already exists"));
    assert!(stderr.contains("--yes"));
    assert_eq!(
        fs::read_to_string(exe_dir.path().join("presets/my-team.yaml"))
            .expect("existing preset should be readable"),
        "semicolon: true\n"
    );
}

#[test]
fn preset_add_overwrites_an_existing_preset_with_yes() {
    let exe_dir = tempfile::tempdir().expect("failed to create temp directory");
    write_file(
        &exe_dir.path().join("presets/my-team.yaml"),
        "semicolon: true\n",
    );

    let source_dir = tempfile::tempdir().expect("failed to create temp directory");
    let source = source_dir.path().join("papyrus-lint.yaml");
    write_file(&source, "semicolon: false\n");

    let output = run_copied_cli(
        exe_dir.path(),
        &[
            "preset",
            "add",
            "my-team",
            &source.to_string_lossy(),
            "--yes",
        ],
        source_dir.path(),
    );

    assert!(output.status.success());
    assert_eq!(
        fs::read_to_string(exe_dir.path().join("presets/my-team.yaml"))
            .expect("overwritten preset should be readable"),
        "semicolon: false\n"
    );
}

#[test]
fn preset_list_prints_only_the_built_ins_with_no_user_presets() {
    let output = run_cli(&["preset", "list"]);

    assert!(output.status.success());
    assert!(output.stderr.is_empty());
    assert_eq!(
        String::from_utf8(output.stdout).expect("stdout should be UTF-8"),
        "strict\nstandard\ncareful\n"
    );
}

#[test]
fn preset_list_includes_user_presets_after_the_built_ins() {
    let exe_dir = tempfile::tempdir().expect("failed to create temp directory");
    write_file(&exe_dir.path().join("presets/my-team.yaml"), "");
    write_file(&exe_dir.path().join("presets/alpha.yaml"), "");

    let project_dir = tempfile::tempdir().expect("failed to create temp directory");
    let output = run_copied_cli(exe_dir.path(), &["preset", "list"], project_dir.path());

    assert!(output.status.success());
    assert_eq!(
        String::from_utf8(output.stdout).expect("stdout should be UTF-8"),
        "strict\nstandard\ncareful\nalpha\nmy-team\n"
    );
}

#[test]
fn preset_add_rejects_a_name_matching_a_built_in_preset() {
    let exe_dir = tempfile::tempdir().expect("failed to create temp directory");
    let source_dir = tempfile::tempdir().expect("failed to create temp directory");
    let source = source_dir.path().join("papyrus-lint.yaml");
    write_file(&source, "semicolon: true\n");

    let output = run_copied_cli(
        exe_dir.path(),
        &["preset", "add", "strict", &source.to_string_lossy()],
        source_dir.path(),
    );

    assert_eq!(output.status.code(), Some(2));
    let stderr = String::from_utf8(output.stderr).expect("stderr should be UTF-8");
    assert!(stderr.contains("built-in preset"));
    assert!(!exe_dir.path().join("presets").exists());
}

#[test]
fn preset_add_errors_when_the_source_file_does_not_exist() {
    let exe_dir = tempfile::tempdir().expect("failed to create temp directory");
    let source_dir = tempfile::tempdir().expect("failed to create temp directory");

    let output = run_copied_cli(
        exe_dir.path(),
        &["preset", "add", "my-team", "does-not-exist.yaml"],
        source_dir.path(),
    );

    assert_eq!(output.status.code(), Some(2));
    assert!(!exe_dir.path().join("presets/my-team.yaml").exists());
}

#[test]
fn init_defaults_to_the_strict_preset() {
    let dir = tempfile::tempdir().expect("failed to create temp directory");

    let output = run_cli_in(&["init", "--game", "skyrim"], dir.path());

    assert!(output.status.success());
    let config = fs::read_to_string(dir.path().join("papyrus-lint.yaml"))
        .expect("init should create papyrus-lint.yaml");
    // Strict is the default, and matches every rule on (including pure
    // style/naming nits like identifier casing).
    assert!(config.contains("  identifier_casing: true\n"));
}

#[test]
fn init_preset_flag_selects_the_standard_preset() {
    let dir = tempfile::tempdir().expect("failed to create temp directory");

    let output = run_cli_in(
        &["init", "--game", "skyrim", "--preset", "standard"],
        dir.path(),
    );

    assert!(output.status.success());
    let config = fs::read_to_string(dir.path().join("papyrus-lint.yaml"))
        .expect("init should create papyrus-lint.yaml");
    assert!(config.contains("  identifier_casing: false\n"));
    assert!(config.contains("  trailing_whitespace: true\n"));
}

#[test]
fn init_preset_flag_accepts_the_equals_form_case_insensitively() {
    let dir = tempfile::tempdir().expect("failed to create temp directory");

    let output = run_cli_in(
        &["init", "--game", "skyrim", "--preset=CAREFUL"],
        dir.path(),
    );

    assert!(output.status.success());
    let config = fs::read_to_string(dir.path().join("papyrus-lint.yaml"))
        .expect("init should create papyrus-lint.yaml");
    assert!(config.contains("cyclomatic_complexity_warning: 20\n"));
    assert!(config.contains("  trailing_whitespace: false\n"));
}

#[test]
fn init_rejects_an_unknown_preset() {
    let dir = tempfile::tempdir().expect("failed to create temp directory");

    let output = run_cli_in(
        &["init", "--game", "skyrim", "--preset", "lenient"],
        dir.path(),
    );

    assert_eq!(output.status.code(), Some(2));
    assert!(output.stdout.is_empty());
    let stderr = String::from_utf8(output.stderr).expect("stderr should be UTF-8");
    assert!(stderr.contains("unknown preset 'lenient'"));
    assert!(!dir.path().join("papyrus-lint.yaml").exists());
}

#[test]
fn init_rejects_missing_blank_and_extra_preset_arguments() {
    let dir = tempfile::tempdir().expect("failed to create temp directory");

    for args in [
        vec!["init", "--game", "skyrim", "--preset"],
        vec!["init", "--game", "skyrim", "--preset="],
        vec!["init", "--game", "skyrim", "--preset=strict", "unexpected"],
    ] {
        let output = run_cli_in(&args, dir.path());

        assert_eq!(output.status.code(), Some(2), "arguments: {args:?}");
        assert!(output.stdout.is_empty(), "arguments: {args:?}");
        assert!(
            String::from_utf8(output.stderr)
                .expect("stderr should be UTF-8")
                .starts_with("Usage: PapyrusLinterCLI"),
            "arguments: {args:?}"
        );
        assert!(
            !dir.path().join("papyrus-lint.yaml").exists(),
            "invalid init arguments must not create a config: {args:?}"
        );
    }
}

#[test]
fn preset_dispatch_rejects_missing_or_unknown_subcommands() {
    for args in [vec!["preset"], vec!["preset", "remove"]] {
        let output = run_cli(&args);

        assert_eq!(output.status.code(), Some(2), "arguments: {args:?}");
        assert!(output.stdout.is_empty(), "arguments: {args:?}");
        assert!(
            String::from_utf8(output.stderr)
                .expect("stderr should be UTF-8")
                .starts_with("Usage: PapyrusLinterCLI"),
            "arguments: {args:?}"
        );
    }
}

#[test]
fn preset_add_rejects_invalid_argument_shapes() {
    for args in [
        vec!["preset", "add"],
        vec!["preset", "add", "name"],
        vec!["preset", "add", "name", "config.yaml", "extra"],
        vec!["preset", "add", "name", "config.yaml", "--force"],
    ] {
        let output = run_cli(&args);

        assert_eq!(output.status.code(), Some(2), "arguments: {args:?}");
        assert!(output.stdout.is_empty(), "arguments: {args:?}");
        assert!(
            String::from_utf8(output.stderr)
                .expect("stderr should be UTF-8")
                .starts_with("Usage: PapyrusLinterCLI"),
            "arguments: {args:?}"
        );
    }
}

#[test]
fn init_refuses_to_replace_an_existing_config_through_the_binary() {
    let dir = tempfile::tempdir().expect("failed to create temp directory");
    let config_path = dir.path().join("papyrus-lint.yaml");
    write_file(&config_path, "rules:\n  semicolon: false\n");

    let output = run_cli_in(&["init", "--game", "skyrim"], dir.path());

    assert_eq!(output.status.code(), Some(2));
    assert!(output.stdout.is_empty());
    assert!(String::from_utf8(output.stderr)
        .expect("stderr should be UTF-8")
        .contains("config already exists"));
    assert_eq!(
        fs::read_to_string(config_path).expect("existing config should remain readable"),
        "rules:\n  semicolon: false\n"
    );
}
