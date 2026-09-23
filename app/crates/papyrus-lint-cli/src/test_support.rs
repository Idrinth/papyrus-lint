//! Shared helpers for this crate's unit tests.
//!
//! Each `src/*.rs` file keeps its own `#[cfg(test)]` module, matching the
//! other crates; helpers that several of those modules need live here.

use std::fs;
use std::path::{Path, PathBuf};

use crate::run;

pub(crate) fn write_file(path: &Path, contents: &str) {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).expect("failed to create parent dir");
    }
    fs::write(path, contents).expect("failed to write file");
}

fn with_required_subcommand(args: &[String]) -> Vec<String> {
    match args.first().map(String::as_str) {
        None | Some("init") | Some("preset") | Some("doctor") | Some("lint") | Some("fix")
        | Some("help") | Some("version") => args.to_vec(),
        _ => {
            let mut prefixed = vec!["lint".to_string()];
            prefixed.extend(args.iter().cloned());
            prefixed
        }
    }
}

pub(crate) fn run_captured_with_terminal_stdout(
    args: &[String],
    stdout_is_terminal: bool,
) -> (u8, String, String) {
    let args = with_required_subcommand(args);
    let mut stdout = Vec::new();
    let mut stderr = Vec::new();
    let code = run(&args, &mut stdout, &mut stderr, stdout_is_terminal);
    (
        code,
        String::from_utf8(stdout).expect("stdout should be utf8"),
        String::from_utf8(stderr).expect("stderr should be utf8"),
    )
}

pub(crate) fn run_captured(args: &[String]) -> (u8, String, String) {
    run_captured_with_terminal_stdout(args, false)
}

#[cfg(unix)]
pub(crate) fn write_stub_compiler(dir: &Path, script: &str) -> PathBuf {
    use std::os::unix::fs::PermissionsExt;

    let path = dir.join("stub-compiler.sh");
    write_file(&path, script);
    fs::set_permissions(&path, fs::Permissions::from_mode(0o755))
        .expect("failed to make stub compiler executable");
    path
}
