//! Each individual health check `doctor` runs (see [`super::run_doctor`]):
//! resolving which `.psc` files the given achlist/`.psc`/directory path
//! actually names, and validating the configured/conventional paths a
//! matching lint/fix run would use, without linting any script. Split out
//! from [`super`] so the checks themselves — one function per concern — are
//! easy to tell apart from `doctor`'s own argument parsing and report
//! formatting.

use std::path::{Path, PathBuf};

use papyrus_lint_config as config;
use papyrus_lint_core::achlist;
use papyrus_lint_core::ppj;
use papyrus_lint_core::script_locator::{find_psc_files_recursively, CANDIDATE_DIRS};
use serde::Serialize;

use crate::project::{absolutize, is_ppj_path, is_psc_path};

/// One health-check result reported by [`super::run_doctor`]: whether a path
/// assumed by convention or named in the project's configuration actually
/// exists, or whether that configuration itself parses at all.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub(crate) enum DoctorStatus {
    Ok,
    Warning,
    Error,
}

impl DoctorStatus {
    pub(crate) fn label(self) -> &'static str {
        match self {
            DoctorStatus::Ok => "ok",
            DoctorStatus::Warning => "warning",
            DoctorStatus::Error => "error",
        }
    }
}

/// One line of `doctor`'s report: a single check's outcome and a
/// human-readable explanation of it, printed as `[<status>] <message>` in
/// the plain-text report or as a `{status, message}` object in the
/// `--json` one (see [`super::report::DoctorReport`]).
#[derive(Debug, Serialize)]
pub(crate) struct DoctorCheck {
    pub(crate) status: DoctorStatus,
    pub(crate) message: String,
}

impl DoctorCheck {
    pub(crate) fn ok(message: String) -> Self {
        DoctorCheck {
            status: DoctorStatus::Ok,
            message,
        }
    }

    pub(crate) fn warning(message: String) -> Self {
        DoctorCheck {
            status: DoctorStatus::Warning,
            message,
        }
    }

    pub(crate) fn error(message: String) -> Self {
        DoctorCheck {
            status: DoctorStatus::Error,
            message,
        }
    }
}

/// Resolves `input_path` into the scripts `doctor` reports on, alongside any
/// `<Import>` search paths it names — populated only for a `.ppj` input, the
/// same as [`crate::run_scan`]'s own `collect_script_paths`, so `doctor`
/// validates the same additional roots a matching lint/fix run would
/// actually use.
pub(super) fn collect_doctor_input_checks(
    input_path: &Path,
    is_psc_file: bool,
    is_directory: bool,
    checks: &mut Vec<DoctorCheck>,
) -> (Vec<PathBuf>, Vec<String>) {
    if is_psc_file {
        return (collect_doctor_psc_checks(input_path, checks), Vec::new());
    }
    if is_directory {
        return (
            collect_doctor_directory_checks(input_path, checks),
            Vec::new(),
        );
    }
    if is_ppj_path(input_path) {
        return collect_doctor_ppj_checks(input_path, checks);
    }
    (
        collect_doctor_achlist_checks(input_path, checks),
        Vec::new(),
    )
}

fn collect_doctor_psc_checks(input_path: &Path, checks: &mut Vec<DoctorCheck>) -> Vec<PathBuf> {
    if input_path.is_file() {
        checks.push(DoctorCheck::ok(format!(
            "script {} exists",
            input_path.display()
        )));
        vec![input_path.to_path_buf()]
    } else {
        checks.push(DoctorCheck::error(format!(
            "script {} does not exist",
            input_path.display()
        )));
        Vec::new()
    }
}

fn collect_doctor_directory_checks(
    input_path: &Path,
    checks: &mut Vec<DoctorCheck>,
) -> Vec<PathBuf> {
    let script_paths = find_psc_files_recursively(input_path);
    checks.push(if script_paths.is_empty() {
        DoctorCheck::warning(format!(
            "no .psc files found under {}",
            input_path.display()
        ))
    } else {
        DoctorCheck::ok(format!(
            "found {} .psc file(s) under {}",
            script_paths.len(),
            input_path.display()
        ))
    });
    script_paths
}

fn collect_doctor_achlist_checks(input_path: &Path, checks: &mut Vec<DoctorCheck>) -> Vec<PathBuf> {
    if !input_path.is_file() {
        checks.push(DoctorCheck::error(format!(
            "achlist {} does not exist",
            input_path.display()
        )));
        return Vec::new();
    }
    match achlist::parse_achlist(input_path) {
        Ok(entries) => {
            let missing: Vec<&PathBuf> = entries.iter().filter(|path| !path.is_file()).collect();
            if missing.is_empty() {
                checks.push(DoctorCheck::ok(format!(
                    "every entry in {} exists on disk ({} total)",
                    input_path.display(),
                    entries.len()
                )));
            } else {
                for path in &missing {
                    checks.push(DoctorCheck::error(format!(
                        "achlist entry {} does not exist",
                        path.display()
                    )));
                }
            }
            entries
                .into_iter()
                .filter(|path| is_psc_path(path))
                .collect()
        }
        Err(err) => {
            checks.push(DoctorCheck::error(format!(
                "failed to parse achlist {}: {err}",
                input_path.display()
            )));
            Vec::new()
        }
    }
}

fn collect_doctor_ppj_checks(
    input_path: &Path,
    checks: &mut Vec<DoctorCheck>,
) -> (Vec<PathBuf>, Vec<String>) {
    if !input_path.is_file() {
        checks.push(DoctorCheck::error(format!(
            "ppj {} does not exist",
            input_path.display()
        )));
        return (Vec::new(), Vec::new());
    }
    match ppj::parse_ppj(input_path) {
        Ok(project) => {
            let missing: Vec<&PathBuf> = project
                .scripts
                .iter()
                .filter(|path| !path.is_file())
                .collect();
            if missing.is_empty() {
                checks.push(DoctorCheck::ok(format!(
                    "every entry in {} exists on disk ({} total)",
                    input_path.display(),
                    project.scripts.len()
                )));
            } else {
                for path in &missing {
                    checks.push(DoctorCheck::error(format!(
                        "ppj entry {} does not exist",
                        path.display()
                    )));
                }
            }
            let scripts = project
                .scripts
                .into_iter()
                .filter(|path| is_psc_path(path))
                .collect();
            let imports = project
                .imports
                .iter()
                .map(|import| absolutize(import))
                .collect();
            (scripts, imports)
        }
        Err(err) => {
            checks.push(DoctorCheck::error(format!(
                "failed to parse ppj {}: {err}",
                input_path.display()
            )));
            (Vec::new(), Vec::new())
        }
    }
}

pub(super) fn check_lint_config(
    project_root: &Path,
    config_path: Option<&Path>,
    checks: &mut Vec<DoctorCheck>,
) {
    match config_path.map_or_else(
        || config::load_config(project_root),
        config::load_config_from_path,
    ) {
        Ok(_) => {
            let description = match config_path {
                Some(path) => format!("explicit config {}", path.display()),
                None => match config::config_file_path(project_root) {
                    Some(path) => format!("config {}", path.display()),
                    None => "no papyrus-lint.yaml/.yml found; using default settings".to_string(),
                },
            };
            checks.push(DoctorCheck::ok(format!(
                "{description} loaded successfully"
            )));
        }
        Err(err) => {
            let source = config_path
                .map(|path| path.display().to_string())
                .unwrap_or_else(|| project_root.display().to_string());
            checks.push(DoctorCheck::error(format!(
                "failed to load lint config from {source}: {err}"
            )));
        }
    }
}

pub(super) fn load_additional_script_roots(
    project_root: &Path,
    has_explicit_config: bool,
    cli_script_roots: Vec<String>,
    checks: &mut Vec<DoctorCheck>,
) -> Vec<String> {
    // `--config` bypasses the project root's own additional_script_roots
    // entirely, the same as it does for a normal lint/fix run (see `run`).
    let mut additional_script_roots = if has_explicit_config {
        Vec::new()
    } else {
        match config::load_script_roots(project_root) {
            Ok(roots) => roots,
            Err(err) => {
                checks.push(DoctorCheck::error(format!(
                    "failed to load additional_script_roots: {err}"
                )));
                Vec::new()
            }
        }
    };
    additional_script_roots.extend(cli_script_roots);
    additional_script_roots
}

pub(super) fn load_lookup_script_roots(
    project_root: &Path,
    config_path: Option<&Path>,
    checks: &mut Vec<DoctorCheck>,
) -> Vec<String> {
    match config_path.map_or_else(
        || config::load_lookup_script_roots(project_root),
        config::load_lookup_script_roots_from_path,
    ) {
        Ok(roots) => roots,
        Err(err) => {
            checks.push(DoctorCheck::error(format!(
                "failed to load lookup_script_roots: {err}"
            )));
            Vec::new()
        }
    }
}

pub(super) fn check_configured_roots(
    project_root: &Path,
    roots: &[String],
    ok_label: &str,
    missing_label: &str,
    checks: &mut Vec<DoctorCheck>,
) {
    for root in roots {
        let path = Path::new(root);
        let resolved = if path.is_absolute() {
            path.to_path_buf()
        } else {
            project_root.join(path)
        };
        if resolved.is_dir() {
            checks.push(DoctorCheck::ok(format!(
                "{ok_label} {} exists",
                resolved.display()
            )));
        } else {
            checks.push(DoctorCheck::warning(format!(
                "{missing_label} {} does not exist",
                resolved.display()
            )));
        }
    }
}

pub(super) fn check_conventional_roots(project_root: &Path, checks: &mut Vec<DoctorCheck>) {
    let conventional_roots: Vec<PathBuf> = CANDIDATE_DIRS
        .iter()
        .map(|dir| project_root.join(dir))
        .filter(|path| path.is_dir())
        .collect();
    if conventional_roots.is_empty() {
        checks.push(DoctorCheck::warning(format!(
            "neither scripts/source nor source/scripts exists under {}",
            project_root.display()
        )));
        return;
    }
    for root in &conventional_roots {
        checks.push(DoctorCheck::ok(format!("{} exists", root.display())));
    }
}

pub(super) fn check_compiler(project_root: &Path, checks: &mut Vec<DoctorCheck>) {
    match config::load_compiler_path(project_root) {
        Ok(Some(path)) => {
            if Path::new(&path).is_file() {
                checks.push(DoctorCheck::ok(format!(
                    "configured compiler_path {path} exists"
                )));
            } else {
                checks.push(DoctorCheck::error(format!(
                    "configured compiler_path {path} does not exist"
                )));
            }
        }
        // Unset and unauto-detectable is only a problem once `compile_check`
        // actually needs it (checked separately below); most projects never
        // enable that, so it's not a warning on its own.
        Ok(None) => match config::auto_detect_compiler_path(project_root) {
            Some(path) => checks.push(DoctorCheck::ok(format!(
                "compiler_path not set; auto-detected {}",
                path.display()
            ))),
            None => checks.push(DoctorCheck::ok(
                "compiler_path not set and could not be auto-detected".to_string(),
            )),
        },
        Err(err) => checks.push(DoctorCheck::error(format!(
            "failed to load compiler_path: {err}"
        ))),
    }

    match config::load_compile_check(project_root) {
        Ok(true) => {
            if matches!(config::resolve_compiler_path(project_root), Ok(None)) {
                checks.push(DoctorCheck::warning(
                    "compile_check is enabled but no PapyrusCompiler.exe could be resolved"
                        .to_string(),
                ));
            }
        }
        Ok(false) => {}
        Err(err) => checks.push(DoctorCheck::error(format!(
            "failed to load compile_check: {err}"
        ))),
    }
}
