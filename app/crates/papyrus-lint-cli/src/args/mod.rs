//! Parses and validates [`crate::run`]'s arguments.
//!
//! [`parse`] owns clap's view of the CLI: `init`/`preset add`/`doctor` as
//! real subcommands, and a plain lint/fix/`--blob` run as the default
//! command (flags mixed with `fix` and the path in any order). [`validate`]
//! applies the business rules layered on a lint/fix/`--blob` extraction
//! (mutually exclusive flags, enum coercion, numeric ranges). [`help`] turns
//! a rejected [`ArgsError`] into the text actually written to `stderr`.
//!
//! [`parse_init_preset`] / [`parse_preset_add_args`] stay unit-testable
//! without going through [`parse_cli`].

mod help;
mod parse;
pub(super) mod validate;

use std::path::PathBuf;

pub(crate) use help::write_args_error;
pub(crate) use parse::{parse_cli, ParsedCli};

#[cfg(test)]
pub(crate) use parse::{
    parse_init_preset, parse_preset_add_args, InitPresetError, PresetAddArgsError,
};

use crate::output::{ColorChoice, OutputFormat};

/// Why [`parse_cli`] rejected `args`. `Usage` covers every case
/// [`crate::run`] reports with the generic [`crate::USAGE`] text (a missing
/// flag value, an unrecognized subcommand, or an unrecognized combination
/// of positional arguments); every other variant carries whatever its own
/// more specific message needs.
#[derive(Debug, PartialEq, Eq)]
pub(crate) enum ArgsError {
    Usage,
    InvalidFormat(String),
    InvalidDoctorFormat(String),
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

/// `doctor`'s arguments after its output format has been validated.
#[derive(Debug, PartialEq, Eq)]
pub(crate) struct DoctorArgs {
    pub(crate) json: bool,
    pub(crate) config: Option<String>,
    pub(crate) script_root: Vec<String>,
    pub(crate) input_path: PathBuf,
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

/// What [`parse_cli`] parsed a lint, fix, or `--blob` invocation into:
/// `version` (reported and exited before anything else is even looked at),
/// `--blob <source>`, or a plain lint/fix run.
#[derive(Debug, PartialEq)]
pub(crate) enum ParsedCommand {
    Version,
    Blob(BlobArgs),
    Lint(LintArgs),
}

#[cfg(test)]
#[path = "args_tests.rs"]
mod tests;
