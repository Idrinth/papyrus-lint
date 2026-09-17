//! `doctor`'s own report formatting: the `--json` document and the
//! plain-text `[<status>] <message>` lines plus summary, built from the
//! [`super::checks::DoctorCheck`]s a run collected. Split out from
//! [`super`] so the report's shape and rendering stay independent of how
//! each check was produced.

use std::io::Write;
use std::path::Path;

use serde::Serialize;

use super::checks::{DoctorCheck, DoctorStatus};

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

pub(super) fn write_doctor_report(
    json: bool,
    project_root: &Path,
    checks: Vec<DoctorCheck>,
    success: bool,
    stdout: &mut impl Write,
) {
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
        return;
    }
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
