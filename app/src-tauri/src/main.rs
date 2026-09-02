use std::env;
use std::io::{self, IsTerminal};
use std::process::ExitCode;

/// Launched with no arguments, this binary starts the desktop app, same as
/// always. Launched with an `.achlist` path (or a single `.psc` path, or
/// `-h`/`--help`), it lints non-interactively instead, exactly like the
/// standalone `PapyrusLinterCLI` binary (`app/crates/papyrus-lint-cli`), which
/// stays available on its own for use cases (e.g. a CI pipeline) that
/// shouldn't depend on the desktop app's binary at all.
///
fn main() -> ExitCode {
    let args: Vec<String> = env::args().skip(1).collect();
    detach_unused_windows_console(&args);
    let mut stdout = io::stdout();
    let stdout_is_terminal = stdout.is_terminal();
    dispatch(
        &args,
        &mut stdout,
        &mut io::stderr(),
        stdout_is_terminal,
        papyrus_lint_lib::run,
    )
}

/// A console-subsystem executable is required for Windows shells to wait for
/// CLI mode and capture its stdout/stderr reliably. When the same executable
/// is opened without arguments, detach the automatically-created console
/// before starting Tauri so the ordinary desktop experience remains GUI-only.
#[cfg(all(windows, not(debug_assertions)))]
fn detach_unused_windows_console(args: &[String]) {
    if args.is_empty() {
        unsafe extern "system" {
            fn FreeConsole() -> i32;
        }

        // SAFETY: FreeConsole takes no pointers and simply detaches this
        // process from its console. Failure is harmless (there may be none).
        unsafe {
            FreeConsole();
        }
    }
}

#[cfg(not(all(windows, not(debug_assertions))))]
fn detach_unused_windows_console(_args: &[String]) {}

fn dispatch(
    args: &[String],
    stdout: &mut impl io::Write,
    stderr: &mut impl io::Write,
    stdout_is_terminal: bool,
    launch_desktop: impl FnOnce(),
) -> ExitCode {
    if args.is_empty() {
        launch_desktop();
        ExitCode::SUCCESS
    } else {
        let code = papyrus_lint_cli::run(args, stdout, stderr, stdout_is_terminal);
        ExitCode::from(code)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::Cell;

    #[test]
    fn no_arguments_launches_the_desktop_app() {
        let launched = Cell::new(false);

        let code = dispatch(&[], &mut Vec::new(), &mut Vec::new(), false, || {
            launched.set(true)
        });

        assert_eq!(code, ExitCode::SUCCESS);
        assert!(launched.get());
    }

    #[test]
    fn arguments_are_forwarded_to_the_cli() {
        let mut stdout = Vec::new();
        let mut stderr = Vec::new();
        let launched = Cell::new(false);

        let code = dispatch(
            &["--version".to_string()],
            &mut stdout,
            &mut stderr,
            false,
            || launched.set(true),
        );

        assert_eq!(code, ExitCode::SUCCESS);
        assert_eq!(
            String::from_utf8(stdout).unwrap(),
            format!("PapyrusLinterCLI {}\n", papyrus_lint_cli::VERSION)
        );
        assert!(stderr.is_empty());
        assert!(!launched.get());
    }

    #[test]
    fn json_for_an_existing_script_is_forwarded_to_the_cli() {
        let temp = tempfile::tempdir().unwrap();
        let script = temp.path().join("Existing.psc");
        std::fs::write(&script, "ScriptName Existing\n").unwrap();
        let mut stdout = Vec::new();
        let mut stderr = Vec::new();

        let code = dispatch(
            &["--json".to_string(), script.display().to_string()],
            &mut stdout,
            &mut stderr,
            false,
            || panic!("desktop app must not launch in CLI mode"),
        );

        assert_eq!(code, ExitCode::SUCCESS);
        let report: serde_json::Value = serde_json::from_slice(&stdout).unwrap();
        assert_eq!(report["scripts_checked"], 1);
        assert_eq!(report["files"][0]["path"], script.display().to_string());
        assert!(stderr.is_empty());
    }

    #[test]
    fn cli_failure_is_returned_as_the_process_exit_code() {
        let code = dispatch(
            &["--help".to_string()],
            &mut Vec::new(),
            &mut Vec::new(),
            false,
            || panic!("desktop app must not launch in CLI mode"),
        );

        assert_eq!(code, ExitCode::from(2));
    }
}
