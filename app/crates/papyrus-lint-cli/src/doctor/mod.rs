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
mod tests;
