// Prevents additional console window on Windows in release, DO NOT REMOVE!!
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::env;
use std::io;
use std::process::ExitCode;

/// Launched with no arguments, this binary starts the desktop app, same as
/// always. Launched with an `.achlist` path (or `-h`/`--help`), it lints
/// non-interactively instead, exactly like the standalone `papyrus-lint`
/// CLI binary (`crates/papyrus-lint-cli`), which stays available on its
/// own for use cases (e.g. a CI pipeline) that shouldn't depend on the
/// desktop app's binary at all.
///
/// On Windows release builds this binary is compiled without a console
/// (see the `windows_subsystem` attribute above), so its CLI mode is
/// best-effort there; the standalone `papyrus-lint` binary is the
/// reliable way to lint from a Windows console/script.
fn main() -> ExitCode {
    let args: Vec<String> = env::args().skip(1).collect();
    if args.is_empty() {
        papyrus_lint_lib::run();
        ExitCode::SUCCESS
    } else {
        let code = papyrus_lint_cli::run(&args, &mut io::stdout(), &mut io::stderr());
        ExitCode::from(code)
    }
}
