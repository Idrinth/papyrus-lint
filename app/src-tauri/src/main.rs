use std::env;
use std::io::{self, IsTerminal};
use std::process::ExitCode;

/// Launched with no arguments, this binary starts the desktop app, same as
/// always. Launched with `lint` (or `fix`/`doctor`/`init`/`version`, or
/// `-h`/`--help`), it runs non-interactively instead, exactly like the
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
    stdout: &mut (impl io::Write + Send),
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
#[path = "main_tests.rs"]
mod tests;
