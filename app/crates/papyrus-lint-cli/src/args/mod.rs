//! Parses and validates [`crate::run`]'s own arguments — a plain lint/fix
//! run, or `--blob <source>` — once `init`, `preset add`, and `doctor` (each
//! dispatched, and parsed, by their own module before `run` ever reaches
//! this) are ruled out. Split out from `run` so the parsing and validation
//! themselves are fully testable without touching the filesystem, the same
//! reasoning behind [`crate::init::parse_init_preset`]/
//! [`crate::init::parse_preset_add_args`].
//!
//! Split further into three concerns: [`parse`] (recognizing `clap`'s own
//! `--flag`/`--flag=value`/`--flag value` syntax, nothing more),
//! [`validate`] (the business rules layered on top of that — mutually
//! exclusive flags, enum coercion, numeric ranges), and [`help`] (turning a
//! rejected [`ArgsError`] into the text actually written to `stderr`).

mod help;
mod parse;
mod validate;

use std::path::PathBuf;

pub(crate) use help::write_args_error;

use crate::output::{ColorChoice, OutputFormat};

/// Why [`parse_run_args`] rejected `args`. `Usage` covers every case
/// [`crate::run`] reports with the generic [`crate::USAGE`] text (a missing
/// flag value, or an unrecognized combination of positional arguments);
/// every other variant carries whatever its own more specific message
/// needs.
#[derive(Debug, PartialEq, Eq)]
pub(crate) enum ArgsError {
    Usage,
    JsonAndFormatConflict,
    InvalidFormat(String),
    HashSourceRequiresAi,
    InvalidColor(String),
    BlobWithPathArgument,
    BlobWithFixFlags,
    BlobWithScriptRootProgressThreads,
    UnknownTag(String),
    ProgressRequiresOutput,
    TypeAndTagConflict,
    UnknownRule(String),
    RuleHasNoFix(String),
    InvalidLine(String),
    InvalidThreads(String),
}

/// `--blob <source>`'s own arguments, already validated and normalized the
/// same way a plain lint/fix run's are, ready to hand to
/// [`crate::run_blob`].
#[derive(Debug, PartialEq)]
pub(crate) struct BlobArgs {
    pub(crate) source: String,
    pub(crate) config_path: Option<PathBuf>,
    pub(crate) output_format: OutputFormat,
    pub(crate) hash_source: bool,
    pub(crate) quiet_warnings: bool,
    pub(crate) quiet_info: bool,
    pub(crate) tag_filter: Option<String>,
    pub(crate) color_choice: ColorChoice,
    pub(crate) output_path: Option<PathBuf>,
}

/// A plain lint/fix run's fully parsed and validated arguments.
#[derive(Debug, PartialEq)]
pub(crate) struct LintArgs {
    pub(crate) fix: bool,
    pub(crate) input_path: PathBuf,
    pub(crate) output_format: OutputFormat,
    pub(crate) quiet_warnings: bool,
    pub(crate) quiet_info: bool,
    pub(crate) short_paths: bool,
    pub(crate) progress: bool,
    pub(crate) dry_run: bool,
    pub(crate) hash_source: bool,
    pub(crate) config_path: Option<PathBuf>,
    pub(crate) output_path: Option<PathBuf>,
    pub(crate) cli_script_roots: Vec<String>,
    pub(crate) tag_filter: Option<String>,
    pub(crate) rule_filter: Option<&'static str>,
    pub(crate) target_line: Option<usize>,
    pub(crate) color_choice: ColorChoice,
    pub(crate) thread_count: usize,
}

/// What [`parse_run_args`] parsed `args` into: `--version`/`-V` (reported
/// and exited before anything else is even looked at), `--blob <source>`,
/// or a plain lint/fix run.
#[derive(Debug, PartialEq)]
pub(crate) enum ParsedCommand {
    Version,
    Blob(BlobArgs),
    Lint(LintArgs),
}

/// Parses and validates `args` into a [`ParsedCommand`]: [`parse::parse_raw`]
/// recognizes `clap`'s own syntax, then [`validate::validate`] applies every
/// usage check a plain lint/fix run or `--blob` needs before any of the
/// actual work (resolving paths, loading config, linting) begins.
pub(crate) fn parse_run_args(args: &[String]) -> Result<ParsedCommand, ArgsError> {
    validate::validate(parse::parse_raw(args)?)
}

#[cfg(test)]
#[path = "args_tests.rs"]
mod tests;
