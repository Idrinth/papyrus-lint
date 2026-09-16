use std::io::Write;
use std::path::{Path, PathBuf};

use papyrus_lint_core::script_locator::{find_psc_files_recursively, CANDIDATE_DIRS};
use papyrus_lint_core::{achlist, config};
use serde::Serialize;

use crate::project::{find_candidate_pair_root, find_psc_project_root};
use crate::USAGE;

/// One health-check result reported by [`run_doctor`]: whether a path
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
/// `--json` one (see [`DoctorReport`]).
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

/// The full report printed by `doctor` with `--json`, mirroring the
/// plain-text `[<status>] <message>` lines plus the summary printed
/// without it.
#[derive(Debug, Serialize)]
pub(crate) struct DoctorReport {
    pub(crate) project_root: String,
    pub(crate) checks: Vec<DoctorCheck>,
    /// Whether every check passed (no `warning`/`error` among them), i.e.
    /// whether the run would exit `0`.
    pub(crate) success: bool,
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
/// prints a single [`DoctorReport`] document instead of the plain-text
/// `[<status>] <message>` lines.
///
/// Returns `0` if every check passed, `1` if any reported a `warning` or
/// `error`, or `2` on a usage error (a missing flag value, or a
/// missing/extra positional argument).
pub(crate) fn run_doctor(args: &[String], stdout: &mut impl Write, stderr: &mut impl Write) -> u8 {
    let mut json = false;
    let mut config_path: Option<PathBuf> = None;
    let mut cli_script_roots: Vec<String> = Vec::new();
    let mut positionals: Vec<String> = Vec::new();

    let mut input = args.iter().cloned();
    while let Some(arg) = input.next() {
        if arg == "--json" {
            json = true;
        } else if arg == "--config" {
            let Some(value) = input.next() else {
                let _ = write!(stderr, "{USAGE}");
                return 2;
            };
            config_path = Some(PathBuf::from(value));
        } else if arg == "--script-root" {
            let Some(value) = input.next() else {
                let _ = write!(stderr, "{USAGE}");
                return 2;
            };
            cli_script_roots.push(value);
        } else {
            positionals.push(arg);
        }
    }

    let input_path = match positionals.as_slice() {
        [path] => PathBuf::from(path),
        _ => {
            let _ = write!(stderr, "{USAGE}");
            return 2;
        }
    };

    let mut checks: Vec<DoctorCheck> = Vec::new();

    let is_psc_file = input_path
        .extension()
        .and_then(|ext| ext.to_str())
        .is_some_and(|ext| ext.eq_ignore_ascii_case("psc"));
    let is_directory = !is_psc_file && input_path.is_dir();

    let mut script_paths: Vec<PathBuf> = Vec::new();
    if is_psc_file {
        if input_path.is_file() {
            checks.push(DoctorCheck::ok(format!(
                "script {} exists",
                input_path.display()
            )));
            script_paths.push(input_path.clone());
        } else {
            checks.push(DoctorCheck::error(format!(
                "script {} does not exist",
                input_path.display()
            )));
        }
    } else if is_directory {
        script_paths = find_psc_files_recursively(&input_path);
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
    } else if !input_path.is_file() {
        checks.push(DoctorCheck::error(format!(
            "achlist {} does not exist",
            input_path.display()
        )));
    } else {
        match achlist::parse_achlist(&input_path) {
            Ok(entries) => {
                let missing: Vec<&PathBuf> =
                    entries.iter().filter(|path| !path.is_file()).collect();
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
                script_paths = entries
                    .into_iter()
                    .filter(|path| {
                        path.extension()
                            .and_then(|ext| ext.to_str())
                            .is_some_and(|ext| ext.eq_ignore_ascii_case("psc"))
                    })
                    .collect();
            }
            Err(err) => {
                checks.push(DoctorCheck::error(format!(
                    "failed to parse achlist {}: {err}",
                    input_path.display()
                )));
            }
        }
    }

    // Mirrors `run`'s own project-root resolution (see
    // `find_psc_project_root`/`find_candidate_pair_root`) so `doctor`
    // reports on the same project a matching lint/fix run would use.
    let project_root = if is_psc_file {
        find_psc_project_root(&input_path)
    } else {
        script_paths
            .iter()
            .find_map(|path| find_candidate_pair_root(path))
            .unwrap_or_else(|| {
                if is_directory {
                    input_path.clone()
                } else {
                    input_path
                        .ancestors()
                        .nth(1)
                        .filter(|dir| !dir.as_os_str().is_empty())
                        .map(Path::to_path_buf)
                        .unwrap_or_else(|| PathBuf::from("."))
                }
            })
    };
    checks.push(DoctorCheck::ok(format!(
        "project root resolved to {}",
        project_root.display()
    )));

    match config_path.as_deref().map_or_else(
        || config::load_config(&project_root),
        config::load_config_from_path,
    ) {
        Ok(_) => {
            let description = match config_path.as_deref() {
                Some(path) => format!("explicit config {}", path.display()),
                None => match config::config_file_path(&project_root) {
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
                .as_deref()
                .map(|path| path.display().to_string())
                .unwrap_or_else(|| project_root.display().to_string());
            checks.push(DoctorCheck::error(format!(
                "failed to load lint config from {source}: {err}"
            )));
        }
    }

    // `--config` bypasses the project root's own additional_script_roots
    // entirely, the same as it does for a normal lint/fix run (see `run`).
    let mut additional_script_roots = if config_path.is_some() {
        Vec::new()
    } else {
        match config::load_script_roots(&project_root) {
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

    for root in &additional_script_roots {
        let path = Path::new(root);
        let resolved = if path.is_absolute() {
            path.to_path_buf()
        } else {
            project_root.join(path)
        };
        if resolved.is_dir() {
            checks.push(DoctorCheck::ok(format!(
                "additional script root {} exists",
                resolved.display()
            )));
        } else {
            checks.push(DoctorCheck::warning(format!(
                "configured additional script root {} does not exist",
                resolved.display()
            )));
        }
    }

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
    } else {
        for root in &conventional_roots {
            checks.push(DoctorCheck::ok(format!("{} exists", root.display())));
        }
    }

    match config::load_compiler_path(&project_root) {
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
        Ok(None) => match config::auto_detect_compiler_path(&project_root) {
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

    match config::load_compile_check(&project_root) {
        Ok(true) => {
            if matches!(config::resolve_compiler_path(&project_root), Ok(None)) {
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

    let success = !checks.iter().any(|check| check.status != DoctorStatus::Ok);

    if json {
        let report = DoctorReport {
            project_root: project_root.display().to_string(),
            checks,
            success,
        };
        let _ = writeln!(
            stdout,
            "{}",
            serde_json::to_string_pretty(&report).unwrap_or_else(|_| "{}".to_string())
        );
    } else {
        for check in &checks {
            let _ = writeln!(stdout, "[{}] {}", check.status.label(), check.message);
        }
        let summary = if success {
            "PapyrusLinterCLI doctor: no problems found.".to_string()
        } else {
            let problems = checks
                .iter()
                .filter(|check| check.status != DoctorStatus::Ok)
                .count();
            format!("PapyrusLinterCLI doctor: {problems} problem(s) found.")
        };
        let _ = writeln!(stdout, "{summary}");
    }

    if success {
        0
    } else {
        1
    }
}
