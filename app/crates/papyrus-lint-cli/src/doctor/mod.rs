//! The `doctor` subcommand: validates a project's setup — the paths its
//! configuration assumes or names — without linting any script. Split into
//! [`checks`] (each individual health check) and [`report`] (the
//! plain-text/`--json` rendering of the checks a run collected), leaving
//! this module with just `doctor`'s own argument parsing and the top-level
//! orchestration that runs every check in order.

mod checks;
mod report;

use std::io::Write;
use std::path::PathBuf;

use clap::Parser;

use checks::{
    check_compiler, check_configured_roots, check_conventional_roots, check_lint_config,
    collect_doctor_input_checks, load_additional_script_roots, load_lookup_script_roots,
    DoctorCheck, DoctorStatus,
};
use report::write_doctor_report;

use crate::project::{is_psc_path, resolve_input_project_root};
use crate::USAGE;

/// `doctor`'s own flags/positional, extracted by `clap` the same way
/// `args.rs`'s `RawArgs` is for the main lint/fix invocation.
#[derive(Parser, Debug)]
#[command(
    no_binary_name = true,
    disable_help_flag = true,
    disable_version_flag = true
)]
struct DoctorRawArgs {
    #[arg(long)]
    json: bool,
    #[arg(long)]
    config: Option<String>,
    #[arg(long = "script-root")]
    script_root: Vec<String>,
    positionals: Vec<String>,
}

/// Runs the `doctor` subcommand against `args` (i.e. `args[1..]` in
/// [`run`]): validates a project's configuration and the paths it assumes
/// or names — without linting any script — and reports one line per check.
/// Unlike a usage error, a failed check never aborts the remaining ones:
/// `doctor` always runs every check it can and reports the full picture in
/// one go.
///
/// Accepts the same positional `<path-to-achlist-or-psc-or-directory>` as a
/// plain lint run, plus `--config <path>` and one or more `--script-root
/// <path>` (see [`USAGE`]), so it reports on exactly the project
/// configuration a matching lint/fix run would actually use. `--json`
/// prints a single [`report::DoctorReport`] document instead of the
/// plain-text `[<status>] <message>` lines.
///
/// Returns `0` if every check passed, `1` if any reported a `warning` or
/// `error`, or `2` on a usage error (a missing flag value, or a
/// missing/extra positional argument).
pub(crate) fn run_doctor(args: &[String], stdout: &mut impl Write, stderr: &mut impl Write) -> u8 {
    let raw = match DoctorRawArgs::try_parse_from(args) {
        Ok(raw) => raw,
        Err(_) => {
            let _ = write!(stderr, "{USAGE}");
            return 2;
        }
    };
    let json = raw.json;
    let config_path: Option<PathBuf> = raw.config.map(PathBuf::from);
    let cli_script_roots: Vec<String> = raw.script_root;

    let input_path = match raw.positionals.as_slice() {
        [path] => PathBuf::from(path),
        _ => {
            let _ = write!(stderr, "{USAGE}");
            return 2;
        }
    };

    let mut checks: Vec<DoctorCheck> = Vec::new();

    let is_psc_file = is_psc_path(&input_path);
    let is_directory = !is_psc_file && input_path.is_dir();
    let script_paths =
        collect_doctor_input_checks(&input_path, is_psc_file, is_directory, &mut checks);

    // Mirrors `run`'s own project-root resolution (see
    // `find_psc_project_root`/`find_candidate_pair_root`) so `doctor`
    // reports on the same project a matching lint/fix run would use.
    let project_root =
        resolve_input_project_root(&input_path, &script_paths, is_psc_file, is_directory);
    checks.push(DoctorCheck::ok(format!(
        "project root resolved to {}",
        project_root.display()
    )));

    check_lint_config(&project_root, config_path.as_deref(), &mut checks);
    let additional_script_roots = load_additional_script_roots(
        &project_root,
        config_path.is_some(),
        cli_script_roots,
        &mut checks,
    );
    check_configured_roots(
        &project_root,
        &additional_script_roots,
        "additional script root",
        "configured additional script root",
        &mut checks,
    );

    let lookup_script_roots =
        load_lookup_script_roots(&project_root, config_path.as_deref(), &mut checks);
    check_configured_roots(
        &project_root,
        &lookup_script_roots,
        "lookup script root (analysis only)",
        "configured lookup script root",
        &mut checks,
    );

    check_conventional_roots(&project_root, &mut checks);
    check_compiler(&project_root, &mut checks);

    let success = !checks.iter().any(|check| check.status != DoctorStatus::Ok);
    write_doctor_report(json, &project_root, checks, success, stdout);

    if success {
        0
    } else {
        1
    }
}

#[cfg(test)]
mod tests {
    use crate::test_support::*;
    use std::fs;

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
}
