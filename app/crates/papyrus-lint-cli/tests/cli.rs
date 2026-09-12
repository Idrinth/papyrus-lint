//! End-to-end tests for the standalone `PapyrusLinterCLI` binary.
//!
//! The unit tests in `src/lib.rs` exercise the shared `run` function. These
//! tests additionally verify that the binary entry point forwards arguments,
//! writes to the expected process streams, and returns the documented status.

use std::fs;
use std::path::Path;
use std::process::{Command, Output};

fn write_file(path: &Path, contents: &str) {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).expect("failed to create parent directory");
    }
    fs::write(path, contents).expect("failed to write fixture");
}

fn run_cli(args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_PapyrusLinterCLI"))
        .args(args)
        .output()
        .expect("failed to run PapyrusLinterCLI")
}

fn run_cli_in(args: &[&str], current_dir: &Path) -> Output {
    Command::new(env!("CARGO_BIN_EXE_PapyrusLinterCLI"))
        .args(args)
        .current_dir(current_dir)
        .output()
        .expect("failed to run PapyrusLinterCLI")
}

#[test]
fn help_is_written_to_stderr_with_the_usage_error_status() {
    let output = run_cli(&["--help"]);

    assert_eq!(output.status.code(), Some(2));
    assert!(output.stdout.is_empty());
    assert!(String::from_utf8(output.stderr)
        .expect("stderr should be UTF-8")
        .starts_with("Usage: PapyrusLinterCLI"));
}

#[test]
fn no_arguments_prints_usage_through_the_binary_entry_point() {
    let output = run_cli(&[]);

    assert_eq!(output.status.code(), Some(2));
    assert!(output.stdout.is_empty());
    assert!(String::from_utf8(output.stderr)
        .expect("stderr should be UTF-8")
        .starts_with("Usage: PapyrusLinterCLI"));
}

#[test]
fn version_is_written_to_stdout() {
    let output = run_cli(&["--version"]);

    assert!(output.status.success());
    assert!(output.stderr.is_empty());
    assert_eq!(
        String::from_utf8(output.stdout).expect("stdout should be UTF-8"),
        format!("PapyrusLinterCLI {}\n", env!("CARGO_PKG_VERSION"))
    );
}

#[test]
fn short_version_flag_is_written_to_stdout() {
    let output = run_cli(&["-V"]);

    assert!(output.status.success());
    assert!(output.stderr.is_empty());
    assert_eq!(
        String::from_utf8(output.stdout).expect("stdout should be UTF-8"),
        format!("PapyrusLinterCLI {}\n", env!("CARGO_PKG_VERSION"))
    );
}

#[test]
fn extra_positional_argument_reports_usage_without_writing_to_stdout() {
    let output = run_cli(&["first.achlist", "second.achlist"]);

    assert_eq!(output.status.code(), Some(2));
    assert!(output.stdout.is_empty());
    let stderr = String::from_utf8(output.stderr).expect("stderr should be UTF-8");
    assert!(stderr.starts_with("Usage: PapyrusLinterCLI"));
}

#[test]
fn json_mode_lints_a_script_through_the_binary_entry_point() {
    let dir = tempfile::tempdir().expect("failed to create temp directory");
    let script = dir.path().join("scripts/source/Example.psc");
    write_file(&script, "ScriptName Example   \n");

    let output = run_cli(&["--json", &script.to_string_lossy()]);

    assert!(output.status.success());
    assert!(output.stderr.is_empty());
    let report: serde_json::Value =
        serde_json::from_slice(&output.stdout).expect("stdout should contain JSON");
    assert_eq!(report["scripts_checked"], 1);
    assert_eq!(report["total_diagnostics"], 1);
    assert_eq!(
        report["files"][0]["diagnostics"][0]["rule"],
        "trailing-whitespace"
    );
}

#[test]
fn ai_format_includes_source_and_triggered_rule_details() {
    let dir = tempfile::tempdir().expect("failed to create temp directory");
    let script = dir.path().join("scripts/source/Example.psc");
    let source = "ScriptName Example   \n";
    write_file(&script, source);

    let output = run_cli(&["--format", "ai", &script.to_string_lossy()]);

    assert!(output.status.success());
    assert!(output.stderr.is_empty());
    let report: serde_json::Value =
        serde_json::from_slice(&output.stdout).expect("stdout should contain an AI export");
    assert_eq!(report["header"]["tool"], "Papyrus Lint");
    assert_eq!(
        report["header"]["website"],
        "https://papyrus-lint.idrinth.de"
    );
    assert_eq!(report["header"]["target_game"], "Skyrim SE/AE");
    let generated_at = report["header"]["generated_at"]
        .as_str()
        .expect("generated_at should be a string");
    assert_eq!(generated_at.len(), 24);
    assert!(generated_at.ends_with('Z'));
    assert_eq!(report["findings"]["files"][0]["source"], source);
    assert_eq!(
        report["findings"]["files"][0]["diagnostics"][0]["rule"],
        "trailing-whitespace"
    );
    assert_eq!(report["rule_details"][0]["rule"], "trailing-whitespace");
    assert!(report["rule_details"][0]["description"].is_string());
    assert_eq!(report["rule_details"][0]["auto_fixable"], true);
}

#[test]
fn ai_format_omits_scripts_without_diagnostics() {
    let dir = tempfile::tempdir().expect("failed to create temp directory");
    let scripts = dir.path().join("scripts/source");
    write_file(
        &scripts.join("WithIssues.psc"),
        "ScriptName WithIssues   \n",
    );
    write_file(&scripts.join("Clean.psc"), "ScriptName Clean\n");

    let output = run_cli(&["--format", "ai", &scripts.to_string_lossy()]);

    assert!(output.status.success());
    assert!(output.stderr.is_empty());
    let report: serde_json::Value =
        serde_json::from_slice(&output.stdout).expect("stdout should contain an AI export");
    let files = report["findings"]["files"]
        .as_array()
        .expect("AI export files should be an array");
    assert_eq!(report["findings"]["files_with_diagnostics"], 1);
    assert_eq!(files.len(), 1);
    assert!(files[0]["path"]
        .as_str()
        .expect("AI export path should be a string")
        .ends_with("WithIssues.psc"));
    assert!(!output
        .stdout
        .windows(b"Clean.psc".len())
        .any(|window| window == b"Clean.psc"));
}

#[test]
fn fix_mode_rewrites_a_script_through_the_binary_entry_point() {
    let dir = tempfile::tempdir().expect("failed to create temp directory");
    let script = dir.path().join("scripts/source/Example.psc");
    write_file(&script, "ScriptName Example   \n");

    let output = run_cli(&["fix", &script.to_string_lossy()]);

    assert!(output.status.success());
    assert!(output.stderr.is_empty());
    assert_eq!(
        fs::read_to_string(script).expect("fixed script should be readable"),
        "ScriptName Example\n"
    );
    assert!(String::from_utf8(output.stdout)
        .expect("stdout should be UTF-8")
        .contains("(1 script(s) fixed.)"));
}

#[test]
fn fix_dry_run_prints_a_diff_without_writing_through_the_binary_entry_point() {
    let dir = tempfile::tempdir().expect("failed to create temp directory");
    let script = dir.path().join("scripts/source/Example.psc");
    write_file(&script, "ScriptName Example   \n");

    let output = run_cli(&["fix", "--dry-run", &script.to_string_lossy()]);

    assert!(output.status.success());
    assert!(output.stderr.is_empty());
    assert_eq!(
        fs::read_to_string(&script).expect("script should be unchanged"),
        "ScriptName Example   \n"
    );
    let stdout = String::from_utf8(output.stdout).expect("stdout should be UTF-8");
    assert!(stdout.contains(&format!("--- {}\n", script.display())));
    assert!(stdout.contains("-ScriptName Example   \n"));
    assert!(stdout.contains("+ScriptName Example\n"));
    assert!(stdout.contains("(1 script(s) would be fixed.)"));
}

#[test]
fn init_creates_a_config_in_the_process_working_directory() {
    let dir = tempfile::tempdir().expect("failed to create temp directory");

    let output = run_cli_in(&["init"], dir.path());

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

/// Copies the built `PapyrusLinterCLI` binary into `exe_dir` (which may
/// already contain other executable-adjacent files, e.g. a base config or a
/// `presets` directory) and runs it with `args` inside `current_dir`.
/// Immediately after `fs::copy`, some CI filesystems (overlayfs in
/// particular) briefly still report the freshly written copy as busy
/// (`ETXTBSY`) when it's exec'd, even though the copy itself has already
/// completed; a short, bounded retry absorbs that race instead of flaking
/// the test.
fn run_copied_cli(exe_dir: &Path, args: &[&str], current_dir: &Path) -> Output {
    let exe_path = exe_dir.join(
        Path::new(env!("CARGO_BIN_EXE_PapyrusLinterCLI"))
            .file_name()
            .expect("binary path should have a file name"),
    );
    fs::copy(env!("CARGO_BIN_EXE_PapyrusLinterCLI"), &exe_path)
        .expect("failed to copy the CLI binary next to a base config");

    let mut attempts_left = 20;
    loop {
        match Command::new(&exe_path)
            .args(args)
            .current_dir(current_dir)
            .output()
        {
            Ok(output) => break output,
            Err(err) if err.raw_os_error() == Some(26) && attempts_left > 1 => {
                attempts_left -= 1;
                std::thread::sleep(std::time::Duration::from_millis(50));
            }
            Err(err) => panic!("failed to run the copied PapyrusLinterCLI binary: {err}"),
        }
    }
}

#[test]
fn init_merges_a_config_placed_next_to_the_executable() {
    let exe_dir = tempfile::tempdir().expect("failed to create temp directory");
    write_file(
        &exe_dir.path().join("papyrus-lint.yaml"),
        "semicolon: true\n",
    );

    let project_dir = tempfile::tempdir().expect("failed to create temp directory");
    let output = run_copied_cli(exe_dir.path(), &["init"], project_dir.path());

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
        &["init", "--preset", "my-preset"],
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
        &["init", "--preset", "does-not-exist"],
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
        &["init", "--preset", "my-team"],
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

    let output = run_cli_in(&["init"], dir.path());

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

    let output = run_cli_in(&["init", "--preset", "standard"], dir.path());

    assert!(output.status.success());
    let config = fs::read_to_string(dir.path().join("papyrus-lint.yaml"))
        .expect("init should create papyrus-lint.yaml");
    assert!(config.contains("  identifier_casing: false\n"));
    assert!(config.contains("  trailing_whitespace: true\n"));
}

#[test]
fn init_preset_flag_accepts_the_equals_form_case_insensitively() {
    let dir = tempfile::tempdir().expect("failed to create temp directory");

    let output = run_cli_in(&["init", "--preset=CAREFUL"], dir.path());

    assert!(output.status.success());
    let config = fs::read_to_string(dir.path().join("papyrus-lint.yaml"))
        .expect("init should create papyrus-lint.yaml");
    assert!(config.contains("cyclomatic_complexity_warning: 20\n"));
    assert!(config.contains("  trailing_whitespace: false\n"));
}

#[test]
fn init_rejects_an_unknown_preset() {
    let dir = tempfile::tempdir().expect("failed to create temp directory");

    let output = run_cli_in(&["init", "--preset", "lenient"], dir.path());

    assert_eq!(output.status.code(), Some(2));
    assert!(output.stdout.is_empty());
    let stderr = String::from_utf8(output.stderr).expect("stderr should be UTF-8");
    assert!(stderr.contains("unknown preset 'lenient'"));
    assert!(!dir.path().join("papyrus-lint.yaml").exists());
}

#[test]
fn output_flag_redirects_json_without_writing_to_stdout() {
    let dir = tempfile::tempdir().expect("failed to create temp directory");
    let script = dir.path().join("scripts/source/Example.psc");
    let report_path = dir.path().join("reports/lint.json");
    write_file(&script, "ScriptName Example   \n");
    fs::create_dir_all(report_path.parent().unwrap()).expect("failed to create reports directory");

    let output = run_cli(&[
        "--json",
        "--output",
        &report_path.to_string_lossy(),
        &script.to_string_lossy(),
    ]);

    assert!(output.status.success());
    assert!(output.stdout.is_empty());
    assert!(output.stderr.is_empty());
    let report: serde_json::Value = serde_json::from_str(
        &fs::read_to_string(report_path).expect("JSON report should be written"),
    )
    .expect("output file should contain JSON");
    assert_eq!(report["scripts_checked"], 1);
    assert_eq!(report["total_diagnostics"], 1);
    assert_eq!(
        report["files"][0]["diagnostics"][0]["rule"],
        "trailing-whitespace"
    );
}

#[test]
fn missing_script_reports_an_io_error_on_stderr() {
    let dir = tempfile::tempdir().expect("failed to create temp directory");
    let missing_script = dir.path().join("scripts/source/Missing.psc");

    let output = run_cli(&[&missing_script.to_string_lossy()]);

    assert_eq!(output.status.code(), Some(2));
    assert!(output.stdout.is_empty());
    let stderr = String::from_utf8(output.stderr).expect("stderr should be UTF-8");
    assert!(stderr.starts_with("error: failed to read "));
    assert!(stderr.contains("Missing.psc"));
}

#[test]
fn lint_errors_produce_a_failure_status_through_the_binary_entry_point() {
    let dir = tempfile::tempdir().expect("failed to create temp directory");
    let script = dir.path().join("scripts/source/Example.psc");
    write_file(
        &script,
        "ScriptName Example\n\nFunction DoThing()\n    Game.GetPlayer()\nEndFunction\n",
    );

    let output = run_cli(&[&script.to_string_lossy()]);

    assert_eq!(output.status.code(), Some(1));
    assert!(output.stderr.is_empty());
    let stdout = String::from_utf8(output.stdout).expect("stdout should be UTF-8");
    assert!(stdout.contains("[forbidden-functions]"));
    assert!(stdout.contains("[error]"));
    assert!(stdout.contains("problem(s) found in 1 of 1 script(s)"));
}

#[test]
fn quiet_warnings_hides_output_without_changing_the_binary_exit_status() {
    let dir = tempfile::tempdir().expect("failed to create temp directory");
    let script = dir.path().join("scripts/source/Example.psc");
    write_file(&script, "ScriptName Example   \n");

    let output = run_cli(&["--quiet-warnings", &script.to_string_lossy()]);

    assert!(output.status.success());
    assert!(output.stderr.is_empty());
    let stdout = String::from_utf8(output.stdout).expect("stdout should be UTF-8");
    assert!(!stdout.contains("[trailing-whitespace]"));
    assert!(stdout.contains("no problems found in 1 script"));
}

#[test]
fn quiet_info_filters_json_without_changing_the_binary_exit_status() {
    let dir = tempfile::tempdir().expect("failed to create temp directory");
    let script = dir.path().join("scripts/source/Example.psc");
    write_file(
        &script,
        "ScriptName Example\n\nGlobalVariable Property Value Auto\n\nFunction Test()\n    Value.GetValueInt()\nEndFunction\n",
    );
    write_file(
        &dir.path().join("papyrus-lint.yaml"),
        "fail_on_info: true\n",
    );

    let output = run_cli(&["--json", "--quiet-info", &script.to_string_lossy()]);

    assert_eq!(output.status.code(), Some(1));
    assert!(output.stderr.is_empty());
    let report: serde_json::Value =
        serde_json::from_slice(&output.stdout).expect("stdout should contain JSON");
    assert_eq!(report["success"], false);
    assert!(report["files"][0]["diagnostics"]
        .as_array()
        .expect("diagnostics should be an array")
        .iter()
        .all(|diagnostic| diagnostic["level"] != "info"));
}

#[test]
fn short_paths_are_used_in_plain_text_through_the_binary_entry_point() {
    let dir = tempfile::tempdir().expect("failed to create temp directory");
    let script = dir.path().join("scripts/source/Example.psc");
    write_file(&script, "ScriptName Example   \n");

    let output = run_cli(&["--short-paths", &script.to_string_lossy()]);

    assert!(output.status.success());
    assert!(output.stderr.is_empty());
    let stdout = String::from_utf8(output.stdout).expect("stdout should be UTF-8");
    let relative_path = Path::new("scripts/source/Example.psc")
        .to_string_lossy()
        .into_owned();
    assert!(stdout.contains(&format!("{relative_path}:1:")));
    assert!(!stdout.contains(dir.path().to_string_lossy().as_ref()));
}

#[test]
fn tag_filter_is_forwarded_through_the_binary_entry_point() {
    let dir = tempfile::tempdir().expect("failed to create temp directory");
    let script = dir.path().join("scripts/source/Example.psc");
    write_file(
        &script,
        "ScriptName Example   \n\nFunction DoThing()\n    Game.GetPlayer()\nEndFunction\n",
    );

    let output = run_cli(&["--tag", "style", &script.to_string_lossy()]);

    assert!(output.status.success());
    assert!(output.stderr.is_empty());
    let stdout = String::from_utf8(output.stdout).expect("stdout should be UTF-8");
    assert!(stdout.contains("[trailing-whitespace]"));
    assert!(!stdout.contains("[forbidden-functions]"));
}

#[test]
fn typed_fix_only_repairs_the_selected_rule_through_the_binary() {
    let dir = tempfile::tempdir().expect("failed to create temp directory");
    let script = dir.path().join("scripts/source/Example.psc");
    write_file(
        &script,
        "ScriptName Example\n\nFunction Add(Int left,Int right)\nEndFunction   \n",
    );

    let output = run_cli(&["fix", "--type", "comma-spacing", &script.to_string_lossy()]);

    assert!(output.status.success());
    assert!(output.stderr.is_empty());
    assert_eq!(
        fs::read_to_string(script).expect("fixed script should be readable"),
        "ScriptName Example\n\nFunction Add(Int left, Int right)\nEndFunction   \n"
    );
    let stdout = String::from_utf8(output.stdout).expect("stdout should be UTF-8");
    assert!(stdout.contains("[trailing-whitespace]"));
    assert!(!stdout.contains("[comma-spacing]"));
}

#[test]
fn line_scoped_fix_only_rewrites_the_selected_line_through_the_binary() {
    let dir = tempfile::tempdir().expect("failed to create temp directory");
    let script = dir.path().join("scripts/source/Example.psc");
    write_file(
        &script,
        "ScriptName Example   \n\nFunction DoThing()   \nEndFunction\n",
    );

    let output = run_cli(&["fix", "--line=3", &script.to_string_lossy()]);

    assert!(output.status.success());
    assert!(output.stderr.is_empty());
    assert_eq!(
        fs::read_to_string(script).expect("fixed script should be readable"),
        "ScriptName Example   \n\nFunction DoThing()\nEndFunction\n"
    );
    let stdout = String::from_utf8(output.stdout).expect("stdout should be UTF-8");
    assert!(stdout.contains("[trailing-whitespace]"));
    assert!(stdout.contains("(1 script(s) fixed.)"));
}

#[test]
fn explicit_config_disables_a_rule_through_the_binary_entry_point() {
    let dir = tempfile::tempdir().expect("failed to create temp directory");
    let script = dir.path().join("scripts/source/Example.psc");
    let config = dir.path().join("config/override.yaml");
    write_file(&script, "ScriptName Example   \n");
    write_file(&config, "rules:\n  trailing_whitespace: false\n");

    let output = run_cli(&[
        "--config",
        &config.to_string_lossy(),
        &script.to_string_lossy(),
    ]);

    assert!(output.status.success());
    assert!(output.stderr.is_empty());
    assert_eq!(
        String::from_utf8(output.stdout).expect("stdout should be UTF-8"),
        "PapyrusLinterCLI: no problems found in 1 script(s).\n"
    );
}

#[test]
fn color_always_emits_ansi_escapes_through_the_binary_entry_point() {
    let dir = tempfile::tempdir().expect("failed to create temp directory");
    let script = dir.path().join("scripts/source/Example.psc");
    write_file(&script, "ScriptName Example   \n");

    let output = run_cli(&["--color=always", &script.to_string_lossy()]);

    assert!(output.status.success());
    assert!(output.stderr.is_empty());
    let stdout = String::from_utf8(output.stdout).expect("stdout should be UTF-8");
    assert!(stdout.contains("[trailing-whitespace]"));
    assert!(stdout.contains('\x1b'));
}

#[test]
fn invalid_color_value_reports_usage_through_the_binary_entry_point() {
    let dir = tempfile::tempdir().expect("failed to create temp directory");
    let script = dir.path().join("scripts/source/Example.psc");
    write_file(&script, "ScriptName Example\n");

    let output = run_cli(&["--color", "sometimes", &script.to_string_lossy()]);

    assert_eq!(output.status.code(), Some(2));
    assert!(output.stdout.is_empty());
    let stderr = String::from_utf8(output.stderr).expect("stderr should be UTF-8");
    assert_eq!(
        stderr,
        "error: --color must be 'auto', 'always', or 'never', got 'sometimes'\n"
    );
}

#[test]
fn missing_output_directory_reports_an_io_error_through_the_binary() {
    let dir = tempfile::tempdir().expect("failed to create temp directory");
    let script = dir.path().join("scripts/source/Example.psc");
    let report = dir.path().join("missing/report.txt");
    write_file(&script, "ScriptName Example\n");

    let output = run_cli(&[
        "--output",
        &report.to_string_lossy(),
        &script.to_string_lossy(),
    ]);

    assert_eq!(output.status.code(), Some(2));
    assert!(output.stdout.is_empty());
    let stderr = String::from_utf8(output.stderr).expect("stderr should be UTF-8");
    assert!(stderr.starts_with("error: failed to write "));
    assert!(stderr.contains("report.txt"));
}

#[test]
fn achlist_json_report_includes_clean_and_dirty_scripts() {
    let dir = tempfile::tempdir().expect("failed to create temp directory");
    let clean_script = dir.path().join("scripts/source/Clean.psc");
    let dirty_script = dir.path().join("scripts/source/Dirty.psc");
    let achlist = dir.path().join("sources.achlist");
    write_file(&clean_script, "ScriptName Clean\n");
    write_file(&dirty_script, "ScriptName Dirty   \n");
    write_file(
        &achlist,
        r#"["scripts/source/Clean.psc", "scripts/source/Dirty.psc"]"#,
    );

    let output = run_cli(&["--json", "--short-paths", &achlist.to_string_lossy()]);

    assert!(output.status.success());
    assert!(output.stderr.is_empty());
    let report: serde_json::Value =
        serde_json::from_slice(&output.stdout).expect("stdout should contain JSON");
    assert_eq!(report["scripts_checked"], 2);
    assert_eq!(report["files_with_diagnostics"], 1);
    assert_eq!(report["total_diagnostics"], 1);
    let files = report["files"]
        .as_array()
        .expect("files should be an array");
    assert_eq!(files.len(), 2);
    assert_eq!(
        files[0]["path"],
        Path::new("scripts/source/Clean.psc")
            .to_string_lossy()
            .as_ref()
    );
    assert_eq!(files[0]["diagnostics"].as_array().unwrap().len(), 0);
    assert_eq!(
        files[1]["path"],
        Path::new("scripts/source/Dirty.psc")
            .to_string_lossy()
            .as_ref()
    );
    assert_eq!(files[1]["diagnostics"][0]["rule"], "trailing-whitespace");
}

#[test]
fn invalid_config_is_reported_by_the_binary_without_a_lint_report() {
    let dir = tempfile::tempdir().expect("failed to create temp directory");
    let script = dir.path().join("scripts/source/Example.psc");
    write_file(&script, "ScriptName Example\n");
    write_file(
        &dir.path().join("papyrus-lint.yaml"),
        "rules: not-a-rule-map\n",
    );

    let output = run_cli(&["--json", &script.to_string_lossy()]);

    assert_eq!(output.status.code(), Some(2));
    assert!(output.stdout.is_empty());
    let stderr = String::from_utf8(output.stderr).expect("stderr should be UTF-8");
    assert!(stderr.starts_with("error: failed to load lint config:"));
    assert!(stderr.contains("expected struct Rules"));
}

#[test]
fn malformed_achlist_is_reported_by_the_binary_without_a_lint_report() {
    let dir = tempfile::tempdir().expect("failed to create temp directory");
    let achlist = dir.path().join("sources.achlist");
    write_file(&achlist, r#"{"not": "an array"}"#);

    let output = run_cli(&[&achlist.to_string_lossy()]);

    assert_eq!(output.status.code(), Some(2));
    assert!(output.stdout.is_empty());
    let stderr = String::from_utf8(output.stderr).expect("stderr should be UTF-8");
    assert!(stderr.starts_with("error: failed to parse achlist file:"));
    assert!(stderr.contains("expected a sequence"));
}

#[test]
fn init_refuses_to_replace_an_existing_config_through_the_binary() {
    let dir = tempfile::tempdir().expect("failed to create temp directory");
    let config_path = dir.path().join("papyrus-lint.yaml");
    write_file(&config_path, "rules:\n  semicolon: false\n");

    let output = run_cli_in(&["init"], dir.path());

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
