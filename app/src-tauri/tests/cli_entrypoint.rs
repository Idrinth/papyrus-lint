//! Exercises the desktop executable's real process entry point in CLI mode.
//! Unit tests cover `dispatch` directly, but launching the Cargo-provided
//! binary also covers argument collection, console handling, and process exit.

use std::process::Command;

#[test]
fn desktop_binary_forwards_version_requests_to_the_cli() {
    let output = Command::new(env!("CARGO_BIN_EXE_PapyrusLinter"))
        .arg("--version")
        .output()
        .expect("desktop binary should launch");

    assert!(output.status.success());
    assert_eq!(
        String::from_utf8(output.stdout).unwrap(),
        format!("PapyrusLinterCLI {}\n", papyrus_lint_cli::VERSION)
    );
    assert!(output.stderr.is_empty());
}

#[test]
fn desktop_binary_forwards_the_short_version_flag_to_the_cli() {
    let output = Command::new(env!("CARGO_BIN_EXE_PapyrusLinter"))
        .arg("-V")
        .output()
        .expect("desktop binary should launch");

    assert!(output.status.success());
    assert_eq!(
        String::from_utf8(output.stdout).unwrap(),
        format!("PapyrusLinterCLI {}\n", papyrus_lint_cli::VERSION)
    );
    assert!(output.stderr.is_empty());
}

#[test]
fn desktop_binary_forwards_help_requests_and_the_cli_exit_code() {
    let output = Command::new(env!("CARGO_BIN_EXE_PapyrusLinter"))
        .arg("--help")
        .output()
        .expect("desktop binary should launch");

    assert_eq!(output.status.code(), Some(2));
    assert!(output.stdout.is_empty());
    assert_eq!(
        String::from_utf8(output.stderr).unwrap(),
        papyrus_lint_cli::USAGE
    );
}

#[test]
fn desktop_binary_lints_a_single_script_as_json() {
    let temp = tempfile::tempdir().unwrap();
    let script = temp.path().join("Clean.psc");
    std::fs::write(&script, "ScriptName Clean\n").unwrap();

    let output = Command::new(env!("CARGO_BIN_EXE_PapyrusLinter"))
        .args(["--json", script.to_str().unwrap()])
        .output()
        .expect("desktop binary should launch");

    assert!(output.status.success());
    assert!(output.stderr.is_empty());
    let report: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(report["scripts_checked"], 1);
    assert_eq!(report["files"][0]["path"], script.display().to_string());
    assert_eq!(report["files"][0]["diagnostics"], serde_json::json!([]));
}

#[test]
fn desktop_binary_propagates_an_invalid_input_error() {
    let temp = tempfile::tempdir().unwrap();
    let missing = temp.path().join("Missing.psc");

    let output = Command::new(env!("CARGO_BIN_EXE_PapyrusLinter"))
        .arg(&missing)
        .output()
        .expect("desktop binary should launch");

    assert_eq!(output.status.code(), Some(2));
    assert!(output.stdout.is_empty());
    let stderr = String::from_utf8(output.stderr).unwrap();
    assert!(
        stderr.contains("failed to read"),
        "unexpected stderr: {stderr}"
    );
    assert!(stderr.contains(&missing.display().to_string()));
}

#[test]
fn desktop_binary_preserves_the_cli_failure_code_and_json_diagnostics() {
    let temp = tempfile::tempdir().unwrap();
    let script = temp.path().join("Findings.psc");
    std::fs::write(
        &script,
        "ScriptName Findings\n\nFunction Run()\n    Game.GetPlayer()\nEndFunction\n",
    )
    .unwrap();

    let output = Command::new(env!("CARGO_BIN_EXE_PapyrusLinter"))
        .args(["--json", script.to_str().unwrap()])
        .output()
        .expect("desktop binary should launch");

    assert_eq!(output.status.code(), Some(1));
    assert!(output.stderr.is_empty());
    let report: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(report["success"], false);
    let diagnostics = report["files"][0]["diagnostics"].as_array().unwrap();
    let forbidden_function = diagnostics
        .iter()
        .find(|diagnostic| {
            diagnostic["rule"].as_str() == Some(papyrus_lints::forbidden_functions::RULE)
        })
        .expect("Game.GetPlayer should produce a forbidden-functions diagnostic");
    assert_eq!(forbidden_function["line"], 4);
}

#[test]
fn desktop_binary_preserves_plain_text_diagnostics() {
    let temp = tempfile::tempdir().unwrap();
    let script = temp.path().join("Findings.psc");
    std::fs::write(&script, "ScriptName Findings   \n").unwrap();

    let output = Command::new(env!("CARGO_BIN_EXE_PapyrusLinter"))
        .arg(&script)
        .output()
        .expect("desktop binary should launch");

    assert!(output.status.success());
    assert!(output.stderr.is_empty());
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(stdout.contains(&script.display().to_string()));
    assert!(stdout.contains("[trailing-whitespace]"));
    assert!(
        !stdout.contains('\x1b'),
        "piped output must not contain ANSI color"
    );
}

#[test]
fn desktop_binary_lints_every_script_listed_in_an_achlist() {
    let temp = tempfile::tempdir().unwrap();
    let first = temp.path().join("First.psc");
    let second = temp.path().join("Second.psc");
    let achlist = temp.path().join("scripts.achlist");
    std::fs::write(&first, "ScriptName First\n").unwrap();
    std::fs::write(&second, "ScriptName Second\n").unwrap();
    std::fs::write(&achlist, r#"["First.psc", "Second.psc"]"#).unwrap();

    let output = Command::new(env!("CARGO_BIN_EXE_PapyrusLinter"))
        .args(["--json", achlist.to_str().unwrap()])
        .output()
        .expect("desktop binary should launch");

    assert!(output.status.success());
    assert!(output.stderr.is_empty());
    let report: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(report["scripts_checked"], 2);
    let paths: Vec<&str> = report["files"]
        .as_array()
        .unwrap()
        .iter()
        .map(|file| file["path"].as_str().unwrap())
        .collect();
    assert!(paths.contains(&first.to_str().unwrap()));
    assert!(paths.contains(&second.to_str().unwrap()));
}

#[test]
fn desktop_binary_reports_a_malformed_achlist() {
    let temp = tempfile::tempdir().unwrap();
    let achlist = temp.path().join("broken.achlist");
    std::fs::write(&achlist, r#"["Unclosed.psc""#).unwrap();

    let output = Command::new(env!("CARGO_BIN_EXE_PapyrusLinter"))
        .arg(&achlist)
        .output()
        .expect("desktop binary should launch");

    assert_eq!(output.status.code(), Some(2));
    assert!(output.stdout.is_empty());
    let stderr = String::from_utf8(output.stderr).unwrap();
    assert!(
        stderr.contains("failed to parse"),
        "unexpected stderr: {stderr}"
    );
    assert!(stderr.contains("EOF"), "unexpected stderr: {stderr}");
}

#[test]
fn desktop_binary_reports_an_empty_directory_as_a_successful_lint_run() {
    let temp = tempfile::tempdir().unwrap();

    let output = Command::new(env!("CARGO_BIN_EXE_PapyrusLinter"))
        .args(["--json", temp.path().to_str().unwrap()])
        .output()
        .expect("desktop binary should launch");

    assert!(output.status.success());
    assert!(output.stderr.is_empty());
    let report: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(report["success"], true);
    assert_eq!(report["scripts_checked"], 0);
    assert_eq!(report["files"], serde_json::json!([]));
}

#[test]
fn desktop_binary_rejects_extra_positional_arguments() {
    let output = Command::new(env!("CARGO_BIN_EXE_PapyrusLinter"))
        .args(["First.psc", "Second.psc"])
        .output()
        .expect("desktop binary should launch");

    assert_eq!(output.status.code(), Some(2));
    assert!(output.stdout.is_empty());
    assert_eq!(
        String::from_utf8(output.stderr).unwrap(),
        papyrus_lint_cli::USAGE
    );
}

#[test]
fn desktop_binary_dry_run_reports_fixes_without_changing_the_script() {
    let temp = tempfile::tempdir().unwrap();
    let script = temp.path().join("NeedsFix.psc");
    let source = "ScriptName NeedsFix   \n";
    std::fs::write(&script, source).unwrap();

    let output = Command::new(env!("CARGO_BIN_EXE_PapyrusLinter"))
        .args(["--json", "fix", "--dry-run", script.to_str().unwrap()])
        .output()
        .expect("desktop binary should launch");

    assert!(output.status.success());
    assert!(output.stderr.is_empty());
    assert_eq!(std::fs::read_to_string(&script).unwrap(), source);

    let report: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(report["dry_run"], true);
    assert_eq!(report["files"][0]["diagnostics"], serde_json::json!([]));
    let diff = report["files"][0]["diff"]
        .as_str()
        .expect("changed file should include a diff");
    assert!(diff.contains("-ScriptName NeedsFix   "));
    assert!(diff.contains("+ScriptName NeedsFix"));
}

#[test]
fn desktop_binary_fix_writes_changes_to_the_script() {
    let temp = tempfile::tempdir().unwrap();
    let script = temp.path().join("NeedsFix.psc");
    std::fs::write(&script, "ScriptName NeedsFix   \n").unwrap();

    let output = Command::new(env!("CARGO_BIN_EXE_PapyrusLinter"))
        .args(["--json", "fix", script.to_str().unwrap()])
        .output()
        .expect("desktop binary should launch");

    assert!(output.status.success());
    assert!(output.stderr.is_empty());
    assert_eq!(
        std::fs::read_to_string(&script).unwrap(),
        "ScriptName NeedsFix\n"
    );

    let report: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(report["success"], true);
    assert_eq!(report["files"][0]["diagnostics"], serde_json::json!([]));
}

#[test]
fn desktop_binary_recursively_lints_a_directory() {
    let temp = tempfile::tempdir().unwrap();
    let nested = temp.path().join("nested").join("deeper");
    std::fs::create_dir_all(&nested).unwrap();
    let top_level = temp.path().join("TopLevel.psc");
    let nested_script = nested.join("Nested.psc");
    std::fs::write(&top_level, "ScriptName TopLevel\n").unwrap();
    std::fs::write(&nested_script, "ScriptName Nested\n").unwrap();
    std::fs::write(nested.join("ignored.txt"), "ScriptName Ignored\n").unwrap();

    let output = Command::new(env!("CARGO_BIN_EXE_PapyrusLinter"))
        .args(["--json", temp.path().to_str().unwrap()])
        .output()
        .expect("desktop binary should launch");

    assert!(output.status.success());
    assert!(output.stderr.is_empty());
    let report: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(report["scripts_checked"], 2);
    let paths: Vec<&str> = report["files"]
        .as_array()
        .unwrap()
        .iter()
        .map(|file| file["path"].as_str().unwrap())
        .collect();
    assert!(paths.contains(&top_level.to_str().unwrap()));
    assert!(paths.contains(&nested_script.to_str().unwrap()));
}

#[test]
fn desktop_binary_lints_inline_source_without_a_file() {
    let output = Command::new(env!("CARGO_BIN_EXE_PapyrusLinter"))
        .args([
            "--json",
            "--blob",
            "ScriptName Inline\n\nFunction Run()\n    Game.GetPlayer()\nEndFunction\n",
        ])
        .output()
        .expect("desktop binary should launch");

    assert_eq!(output.status.code(), Some(1));
    assert!(output.stderr.is_empty());
    let report: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(report["success"], false);
    assert_eq!(report["scripts_checked"], 1);
    assert_eq!(report["files"][0]["path"], "<blob>");
    assert!(report["files"][0]["diagnostics"]
        .as_array()
        .unwrap()
        .iter()
        .any(|diagnostic| {
            diagnostic["rule"].as_str() == Some(papyrus_lints::forbidden_functions::RULE)
                && diagnostic["line"] == 4
        }));
}

#[test]
fn desktop_binary_rejects_combining_inline_source_with_a_path() {
    let output = Command::new(env!("CARGO_BIN_EXE_PapyrusLinter"))
        .args(["--blob", "ScriptName Inline\n", "Example.psc"])
        .output()
        .expect("desktop binary should launch");

    assert_eq!(output.status.code(), Some(2));
    assert!(output.stdout.is_empty());
    assert_eq!(
        String::from_utf8(output.stderr).unwrap(),
        "error: --blob can't be combined with a path argument (or `fix`)\n"
    );
}

#[test]
fn desktop_binary_can_apply_one_fix_type_without_applying_others() {
    let temp = tempfile::tempdir().unwrap();
    let script = temp.path().join("TargetedFix.psc");
    std::fs::write(
        &script,
        "ScriptName TargetedFix   \n\nFunction Run(Int left,Int right)\nEndFunction\n",
    )
    .unwrap();

    let output = Command::new(env!("CARGO_BIN_EXE_PapyrusLinter"))
        .args([
            "--json",
            "fix",
            "--type",
            papyrus_lints::trailing_whitespace::RULE,
            script.to_str().unwrap(),
        ])
        .output()
        .expect("desktop binary should launch");

    assert!(output.status.success());
    assert!(output.stderr.is_empty());
    assert_eq!(
        std::fs::read_to_string(&script).unwrap(),
        "ScriptName TargetedFix\n\nFunction Run(Int left,Int right)\nEndFunction\n"
    );
    let report: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(report["files_fixed"], 1);
    assert!(report["files"][0]["diagnostics"]
        .as_array()
        .unwrap()
        .iter()
        .any(|diagnostic| {
            diagnostic["rule"].as_str() == Some(papyrus_lints::comma_spacing::RULE)
        }));
    assert!(report["files"][0]["diagnostics"]
        .as_array()
        .unwrap()
        .iter()
        .all(|diagnostic| {
            diagnostic["rule"].as_str() != Some(papyrus_lints::trailing_whitespace::RULE)
        }));
}

#[test]
fn desktop_binary_forwards_doctor_and_its_success_status() {
    let temp = tempfile::tempdir().unwrap();
    let script = temp.path().join("scripts/source/Example.psc");
    std::fs::create_dir_all(script.parent().unwrap()).unwrap();
    std::fs::write(&script, "ScriptName Example\n").unwrap();

    let output = Command::new(env!("CARGO_BIN_EXE_PapyrusLinter"))
        .args(["doctor", script.to_str().unwrap()])
        .output()
        .expect("desktop binary should launch");

    assert!(output.status.success());
    assert!(output.stderr.is_empty());
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(stdout.contains("[ok]"));
    assert!(stdout.contains("no problems found"));
}

#[test]
fn desktop_binary_forwards_ai_format_reports() {
    let temp = tempfile::tempdir().unwrap();
    let script = temp.path().join("Example.psc");
    std::fs::write(&script, "ScriptName Example   \n").unwrap();

    let output = Command::new(env!("CARGO_BIN_EXE_PapyrusLinter"))
        .args(["--format", "ai", script.to_str().unwrap()])
        .output()
        .expect("desktop binary should launch");

    assert!(output.status.success());
    assert!(output.stderr.is_empty());
    let report: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(report["header"]["tool"], "Papyrus Lint");
    assert!(report["files"][0]["diagnostics"]
        .as_array()
        .unwrap()
        .iter()
        .any(|diagnostic| {
            diagnostic["rule"].as_str() == Some(papyrus_lints::trailing_whitespace::RULE)
        }));
}

#[test]
fn desktop_binary_forwards_a_tag_filter() {
    let temp = tempfile::tempdir().unwrap();
    let script = temp.path().join("Example.psc");
    std::fs::write(
        &script,
        "ScriptName Example   \n\nFunction Run()\n    Game.GetPlayer()\nEndFunction\n",
    )
    .unwrap();

    let output = Command::new(env!("CARGO_BIN_EXE_PapyrusLinter"))
        .args(["--json", "--tag", "style", script.to_str().unwrap()])
        .output()
        .expect("desktop binary should launch");

    assert!(output.status.success());
    assert!(output.stderr.is_empty());
    let report: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    let diagnostics = report["files"][0]["diagnostics"].as_array().unwrap();
    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic["rule"].as_str() == Some(papyrus_lints::trailing_whitespace::RULE)
    }));
    assert!(diagnostics.iter().all(|diagnostic| {
        diagnostic["rule"].as_str() != Some(papyrus_lints::forbidden_functions::RULE)
    }));
}

#[test]
fn desktop_binary_init_creates_a_config_in_the_working_directory() {
    let temp = tempfile::tempdir().unwrap();

    let output = Command::new(env!("CARGO_BIN_EXE_PapyrusLinter"))
        .arg("init")
        .current_dir(temp.path())
        .output()
        .expect("desktop binary should launch");

    assert!(output.status.success());
    assert!(output.stderr.is_empty());
    let config_path = temp.path().join("papyrus-lint.yaml");
    assert_eq!(
        String::from_utf8(output.stdout).unwrap(),
        format!("Created {}\n", config_path.display())
    );
    let config = std::fs::read_to_string(config_path).unwrap();
    assert!(config.contains("trailing_whitespace: true"));
}
