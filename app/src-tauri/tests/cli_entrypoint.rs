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
