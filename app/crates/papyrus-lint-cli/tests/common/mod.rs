#![allow(dead_code)]

use std::fs;
use std::path::Path;
use std::process::{Command, Output};

pub fn write_file(path: &Path, contents: &str) {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).expect("failed to create parent directory");
    }
    fs::write(path, contents).expect("failed to write fixture");
}

pub fn run_cli(args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_PapyrusLinterCLI"))
        .args(normalize_args(args))
        .output()
        .expect("failed to run PapyrusLinterCLI")
}

pub fn run_cli_in(args: &[&str], current_dir: &Path) -> Output {
    Command::new(env!("CARGO_BIN_EXE_PapyrusLinterCLI"))
        .args(normalize_args(args))
        .current_dir(current_dir)
        .output()
        .expect("failed to run PapyrusLinterCLI")
}

/// Copies the built CLI into `exe_dir` (so `init` / `preset add` see a
/// directory that can already contain other executable-adjacent files, e.g. a
/// base config or a `presets` directory) and runs it with `args` inside
/// `current_dir`.
/// Immediately after `fs::copy`, some CI filesystems (overlayfs in
/// particular) briefly still report the freshly written copy as busy
/// (`ETXTBSY`) when it's exec'd, even though the copy itself has already
/// completed; a short, bounded retry absorbs that race instead of flaking
/// the test.
pub fn run_copied_cli(exe_dir: &Path, args: &[&str], current_dir: &Path) -> Output {
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
            .args(normalize_args(args))
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

/// Integration tests still spell many lint runs without the `lint`
/// subcommand and with the removed `--json` alias. Rewrite those into the
/// current CLI (`lint`/`fix` plus `--format=json`) without changing tests
/// that already name a subcommand.
fn normalize_args(args: &[&str]) -> Vec<String> {
    if args.is_empty() {
        return Vec::new();
    }
    match args[0] {
        "init" | "preset" | "doctor" | "lint" | "fix" | "help" | "version" => {
            return rewrite_json_alias(args);
        }
        "--help" | "-h" => return args.iter().map(|arg| (*arg).to_string()).collect(),
        _ => {}
    }

    let keep_removed_json_alias =
        args.contains(&"--json") && args.iter().any(|arg| arg.starts_with("--format"));
    let mut fix = false;
    let mut rest = Vec::with_capacity(args.len());
    for arg in args {
        if *arg == "fix" {
            fix = true;
        } else if *arg == "--json" && !keep_removed_json_alias {
            rest.push("--format=json".to_string());
        } else {
            rest.push((*arg).to_string());
        }
    }
    let mut normalized = vec![if fix { "fix" } else { "lint" }.to_string()];
    normalized.extend(rest);
    normalized
}

fn rewrite_json_alias(args: &[&str]) -> Vec<String> {
    args.iter()
        .map(|arg| {
            if *arg == "--json" {
                "--format=json".to_string()
            } else {
                (*arg).to_string()
            }
        })
        .collect()
}
