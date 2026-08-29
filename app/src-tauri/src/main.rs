// Prevents additional console window on Windows in release, DO NOT REMOVE!!
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::env;
use std::io;
use std::process::ExitCode;

/// Launched with no arguments, this binary starts the desktop app, same as
/// always. Launched with an `.achlist` path (or a single `.psc` path, or
/// `-h`/`--help`), it lints non-interactively instead, exactly like the
/// standalone `PapyrusLinterCLI` binary (`crates/papyrus-lint-cli`), which
/// stays available on its own for use cases (e.g. a CI pipeline) that
/// shouldn't depend on the desktop app's binary at all.
///
/// On Windows release builds this binary is compiled without a console
/// (see the `windows_subsystem` attribute above), so its CLI mode is
/// best-effort there; the standalone `PapyrusLinterCLI` binary is the
/// reliable way to lint from a Windows console/script.
fn main() -> ExitCode {
    let args: Vec<String> = env::args().skip(1).collect();
    dispatch(
        &args,
        &mut io::stdout(),
        &mut io::stderr(),
        papyrus_lint_lib::run,
    )
}

fn dispatch(
    args: &[String],
    stdout: &mut impl io::Write,
    stderr: &mut impl io::Write,
    launch_desktop: impl FnOnce(),
) -> ExitCode {
    if args.is_empty() {
        launch_desktop();
        ExitCode::SUCCESS
    } else {
        let code = papyrus_lint_cli::run(args, stdout, stderr);
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

        let code = dispatch(&[], &mut Vec::new(), &mut Vec::new(), || launched.set(true));

        assert_eq!(code, ExitCode::SUCCESS);
        assert!(launched.get());
    }

    #[test]
    fn arguments_are_forwarded_to_the_cli() {
        let mut stdout = Vec::new();
        let mut stderr = Vec::new();
        let launched = Cell::new(false);

        let code = dispatch(&["--version".to_string()], &mut stdout, &mut stderr, || {
            launched.set(true)
        });

        assert_eq!(code, ExitCode::SUCCESS);
        assert_eq!(
            String::from_utf8(stdout).unwrap(),
            format!("PapyrusLinterCLI {}\n", papyrus_lint_cli::VERSION)
        );
        assert!(stderr.is_empty());
        assert!(!launched.get());
    }

    #[test]
    fn cli_failure_is_returned_as_the_process_exit_code() {
        let code = dispatch(
            &["--help".to_string()],
            &mut Vec::new(),
            &mut Vec::new(),
            || panic!("desktop app must not launch in CLI mode"),
        );

        assert_eq!(code, ExitCode::from(2));
    }
}
