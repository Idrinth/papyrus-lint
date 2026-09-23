//! Turns an [`ArgsError`] into what [`crate::run`] actually writes to
//! `stderr`: the generic [`crate::USAGE`] text for [`ArgsError::Usage`], or
//! a one-line `error: ...` message for every other variant. Split out from
//! parsing/validation so the user-facing wording lives in one place,
//! independent of how an error was produced.

use std::io::Write;

use super::ArgsError;
use crate::USAGE;

pub(crate) fn write_args_error(err: ArgsError, stderr: &mut impl Write) {
    match err {
        ArgsError::Usage => {
            let _ = write!(stderr, "{USAGE}");
        }
        err => {
            let _ = writeln!(stderr, "{}", args_error_message(&err));
        }
    }
}

fn args_error_message(err: &ArgsError) -> String {
    match err {
        ArgsError::Usage => unreachable!("Usage is reported via USAGE, not a one-line error"),
        ArgsError::InvalidFormat(_)
        | ArgsError::InvalidDoctorFormat(_)
        | ArgsError::HashSourceRequiresAi
        | ArgsError::InvalidColor(_) => format_flag_error(err),
        ArgsError::BlobWithPathArgument
        | ArgsError::BlobWithFixFlags
        | ArgsError::BlobWithScriptRootProgressThreads => blob_flag_error(err),
        ArgsError::UnknownTag(_)
        | ArgsError::ProgressRequiresOutput
        | ArgsError::TypeAndTagConflict
        | ArgsError::UnknownRule(_)
        | ArgsError::RuleHasNoFix(_)
        | ArgsError::InvalidLine(_)
        | ArgsError::InvalidThreads(_) => lint_flag_error(err),
    }
}

fn format_flag_error(err: &ArgsError) -> String {
    match err {
        ArgsError::InvalidFormat(value) => {
            format!("error: --format must be 'plain', 'json', or 'ai', got '{value}'")
        }
        ArgsError::InvalidDoctorFormat(value) => {
            format!("error: doctor --format must be 'plain' or 'json', got '{value}'")
        }
        ArgsError::HashSourceRequiresAi => "error: --hash-source requires --format ai".to_string(),
        ArgsError::InvalidColor(value) => {
            format!("error: --color must be 'auto', 'always', or 'never', got '{value}'")
        }
        _ => unreachable!("format_flag_error called with a non-format error"),
    }
}

fn blob_flag_error(err: &ArgsError) -> String {
    match err {
        ArgsError::BlobWithPathArgument => {
            "error: --blob can't be combined with a path argument (or `fix`)".to_string()
        }
        ArgsError::BlobWithFixFlags => {
            "error: --blob can't be combined with fix/--type/--line/--dry-run".to_string()
        }
        ArgsError::BlobWithScriptRootProgressThreads => {
            "error: --blob can't be combined with --script-root/--progress/--threads".to_string()
        }
        _ => unreachable!("blob_flag_error called with a non-blob error"),
    }
}

fn lint_flag_error(err: &ArgsError) -> String {
    match err {
        ArgsError::UnknownTag(value) => format!("error: unknown tag '{value}'"),
        ArgsError::ProgressRequiresOutput => {
            "error: --progress requires --output <path>".to_string()
        }
        ArgsError::TypeAndTagConflict => "error: --type and --tag can't be combined".to_string(),
        ArgsError::UnknownRule(value) => format!("error: unknown rule '{value}'"),
        ArgsError::RuleHasNoFix(value) => format!("error: rule '{value}' has no automatic fix"),
        ArgsError::InvalidLine(value) => {
            format!("error: --line must be a positive integer, got '{value}'")
        }
        ArgsError::InvalidThreads(value) => {
            format!("error: --threads must be a positive integer, got '{value}'")
        }
        _ => unreachable!("lint_flag_error called with a non-lint-flag error"),
    }
}
