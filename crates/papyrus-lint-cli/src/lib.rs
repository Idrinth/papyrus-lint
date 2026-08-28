//! Library backing the `PapyrusLinterCLI` command-line interface.
//!
//! ```text
//! PapyrusLinterCLI <path-to-achlist-or-psc>
//! PapyrusLinterCLI fix <path-to-achlist-or-psc>
//! ```
//!
//! Resolves every `.psc` entry listed in the given `.achlist` file (see
//! [`papyrus_lint_core::achlist`]) — or, if given a single `.psc` file
//! directly, treats that file as the achlist's sole entry — lints each
//! against the project's `papyrus-lint.yaml`/`.yml` configuration —
//! looked up next to the input file, falling back to
//! [`papyrus_lints::Config::default`] if it has none (see
//! [`papyrus_lint_core::config`]) — and prints the diagnostics found, one
//! per line. Calls to functions declared on other scripts under the
//! project root are resolved the same way the desktop app resolves them
//! (see [`papyrus_lint_core::function_table`]), so the CLI's "Argument
//! type check"/"Return type check" results match the app's.
//!
//! With the `fix` subcommand, every automatic fix (see
//! [`papyrus_lints::repair`]) is applied to each resolved script first,
//! rewriting it on disk if it changed, before the (now possibly smaller)
//! set of remaining diagnostics is reported the same way.
//!
//! This crate is used both by the standalone `PapyrusLinterCLI` binary
//! (`src/main.rs`) and by the desktop app (`src-tauri`), which runs it in
//! place of launching its GUI whenever it's given command-line arguments.

use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};

use papyrus_lint_core::function_table::FunctionTable;
use papyrus_lint_core::{achlist, config};

pub const USAGE: &str = "Usage: PapyrusLinterCLI <path-to-achlist-or-psc>\n       \
PapyrusLinterCLI fix <path-to-achlist-or-psc>\n\n\
Lints every .psc script listed in the given .achlist file, or a single\n\
.psc file given directly, using the project's papyrus-lint.yaml/.yml\n\
configuration (looked up next to the input file, falling back to\n\
defaults if it has none).\n\n\
With the `fix` subcommand, applies every automatic fix (see README.md)\n\
to those scripts first, rewriting each one on disk if it changed, then\n\
reports whatever diagnostics remain the same way.\n\n\
Options:\n\
  -h, --help     Show this help message\n\
  -V, --version  Print the PapyrusLinterCLI version\n\n\
Exit status: 0 if no problems were found (or none met the configured\n\
fail_on_warning/fail_on_info threshold), 1 if any did, 2 on a usage or\n\
I/O error.\n";

/// The crate's version, as set in `crates/papyrus-lint-cli/Cargo.toml`
/// (kept in sync with the desktop app's version at release time). Printed
/// by `--version`/`-V`.
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

/// Runs the CLI against `args` (the program's arguments, excluding the
/// binary name itself), writing lint output to `stdout` and usage/error
/// text to `stderr`. Returns the process exit code: `0` if linting found
/// no diagnostics that count as a failure (or `--version`/`-V` was given),
/// `1` if it found at least one, or `2` on a usage or I/O error. A
/// `[warning]`/`[info]`-level diagnostic only counts as a failure when the
/// project's `papyrus-lint.yaml` sets `fail_on_warning`/`fail_on_info`
/// (both `false` by default); an `[error]`-level diagnostic, or one with no
/// level tag, always counts. Diagnostics are still printed either way.
pub fn run(args: &[String], stdout: &mut impl Write, stderr: &mut impl Write) -> u8 {
    let (fix, input_path) = match args {
        [flag] if flag == "--version" || flag == "-V" => {
            let _ = writeln!(stdout, "PapyrusLinterCLI {VERSION}");
            return 0;
        }
        [sub, path] if sub == "fix" => (true, PathBuf::from(path)),
        [path] if path != "-h" && path != "--help" && path != "fix" => (false, PathBuf::from(path)),
        _ => {
            let _ = write!(stderr, "{USAGE}");
            return 2;
        }
    };

    let is_psc_file = input_path
        .extension()
        .and_then(|ext| ext.to_str())
        .is_some_and(|ext| ext.eq_ignore_ascii_case("psc"));

    let project_root = input_path
        .parent()
        .filter(|dir| !dir.as_os_str().is_empty())
        .map(Path::to_path_buf)
        .unwrap_or_else(|| PathBuf::from("."));

    let lint_config = match config::load_config(&project_root) {
        Ok(config) => config,
        Err(err) => {
            let _ = writeln!(stderr, "error: failed to load lint config: {err}");
            return 2;
        }
    };

    let script_paths: Vec<PathBuf> = if is_psc_file {
        vec![input_path]
    } else {
        let entries = match achlist::parse_achlist(&input_path) {
            Ok(entries) => entries,
            Err(err) => {
                let _ = writeln!(stderr, "error: {err}");
                return 2;
            }
        };

        entries
            .into_iter()
            .filter(|path| {
                path.extension()
                    .and_then(|ext| ext.to_str())
                    .is_some_and(|ext| ext.eq_ignore_ascii_case("psc"))
            })
            .collect()
    };

    let mut function_table = FunctionTable::new(project_root);
    let mut total_diagnostics = 0usize;
    let mut files_with_diagnostics = 0usize;
    let mut files_fixed = 0usize;
    let mut should_fail = false;

    for script_path in &script_paths {
        let source = match fs::read_to_string(script_path) {
            Ok(source) => source,
            Err(err) => {
                let _ = writeln!(
                    stderr,
                    "error: failed to read {}: {err}",
                    script_path.display()
                );
                return 2;
            }
        };

        let source = if fix {
            let repaired = papyrus_lints::repair(&source, &lint_config);
            if repaired != source {
                if let Err(err) = fs::write(script_path, &repaired) {
                    let _ = writeln!(
                        stderr,
                        "error: failed to write {}: {err}",
                        script_path.display()
                    );
                    return 2;
                }
                files_fixed += 1;
            }
            repaired
        } else {
            source
        };

        let mut diagnostics =
            papyrus_lints::lint_with_external_arguments(&source, &lint_config, &mut function_table);
        if diagnostics.is_empty() {
            continue;
        }

        diagnostics.sort_by_key(|d| (d.line, d.column));
        for diagnostic in &diagnostics {
            let _ = writeln!(
                stdout,
                "{}:{}:{}: [{}] {}",
                script_path.display(),
                diagnostic.line,
                diagnostic.column,
                diagnostic.rule,
                diagnostic.message
            );
            should_fail = should_fail || lint_config.should_fail_on(diagnostic);
        }
        files_with_diagnostics += 1;
        total_diagnostics += diagnostics.len();
    }

    let fixed_suffix = if fix {
        format!(" ({files_fixed} script(s) fixed.)")
    } else {
        String::new()
    };

    if total_diagnostics == 0 {
        let _ = writeln!(
            stdout,
            "PapyrusLinterCLI: no problems found in {} script(s).{fixed_suffix}",
            script_paths.len()
        );
        0
    } else {
        let _ = writeln!(
            stdout,
            "PapyrusLinterCLI: {total_diagnostics} problem(s) found in {files_with_diagnostics} of {} script(s).{fixed_suffix}",
            script_paths.len()
        );
        if should_fail {
            1
        } else {
            0
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn write_file(path: &Path, contents: &str) {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).expect("failed to create parent dir");
        }
        fs::write(path, contents).expect("failed to write file");
    }

    fn run_captured(args: &[String]) -> (u8, String, String) {
        let mut stdout = Vec::new();
        let mut stderr = Vec::new();
        let code = run(args, &mut stdout, &mut stderr);
        (
            code,
            String::from_utf8(stdout).expect("stdout should be utf8"),
            String::from_utf8(stderr).expect("stderr should be utf8"),
        )
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
    }

    #[test]
    fn prints_usage_with_too_many_arguments() {
        let (code, _stdout, stderr) = run_captured(&["a".to_string(), "b".to_string()]);

        assert_eq!(code, 2);
        assert!(stderr.contains("Usage: PapyrusLinterCLI"));
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
            "ScriptName Example   \n",
        );
        write_file(
            &dir.path().join("sources.achlist"),
            r#"["scripts/source/Example.psc"]"#,
        );
        let achlist_path = dir.path().join("sources.achlist");

        let (code, stdout, _stderr) = run_captured(&[achlist_path.to_string_lossy().into_owned()]);

        assert_eq!(code, 1);
        assert!(stdout.contains("[trailing-whitespace]"));
        assert!(stdout.contains("1 problem(s) found in 1 of 1 script(s)"));
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
    fn lints_a_single_psc_file_passed_directly() {
        let dir = tempfile::tempdir().expect("failed to create temp dir");
        let script_path = dir.path().join("Example.psc");
        write_file(&script_path, "ScriptName Example   \n");

        let (code, stdout, _stderr) = run_captured(&[script_path.to_string_lossy().into_owned()]);

        assert_eq!(code, 1);
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
    fn honors_the_project_yaml_config_for_a_single_psc_file() {
        let dir = tempfile::tempdir().expect("failed to create temp dir");
        let script_path = dir.path().join("Example.psc");
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
}
