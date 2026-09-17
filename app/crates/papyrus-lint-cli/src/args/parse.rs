//! Recognizes [`crate::run`]'s own raw argument syntax — `--flag`,
//! `--flag=value`/`--flag value`, repeatable options, and a missing value —
//! via `clap`, with no business-rule validation of its own (mutually
//! exclusive flags, enum coercion, numeric ranges are
//! [`super::validate`]'s job). Split out purely so [`RawArgs`]'s shape (what
//! `clap` itself is responsible for) is easy to tell apart from what's
//! layered on top of it.

use clap::Parser;

use super::ArgsError;

/// The main lint/fix/`--blob` invocation's flags and options, extracted by
/// `clap` before [`super::validate`]'s own semantic validation runs on the
/// raw values. `-h`/`--help` and `-V`/`--version` are declared here too
/// (rather than left to clap's own built-in handling) so the caller can keep
/// reporting them the same way it always has. `fix`'s target path, `fix`
/// itself, and `--blob`'s own exclusivity with a path/`fix` are all resolved
/// from the loose `positionals` left over afterward, since every flag above
/// can appear before, after, or mixed in with them in any order.
#[derive(Parser, Debug)]
#[command(
    no_binary_name = true,
    disable_help_flag = true,
    disable_version_flag = true
)]
pub(super) struct RawArgs {
    #[arg(long, short = 'h')]
    pub(super) help: bool,
    #[arg(long, short = 'V')]
    pub(super) version: bool,
    #[arg(long)]
    pub(super) json: bool,
    #[arg(long)]
    pub(super) quiet_warnings: bool,
    #[arg(long)]
    pub(super) quiet_info: bool,
    #[arg(long)]
    pub(super) short_paths: bool,
    #[arg(long)]
    pub(super) progress: bool,
    #[arg(long)]
    pub(super) dry_run: bool,
    #[arg(long)]
    pub(super) hash_source: bool,
    #[arg(long)]
    pub(super) config: Option<String>,
    #[arg(long = "script-root")]
    pub(super) script_root: Vec<String>,
    #[arg(long)]
    pub(super) output: Option<String>,
    #[arg(long = "type")]
    pub(super) type_filter: Option<String>,
    #[arg(long)]
    pub(super) line: Option<String>,
    #[arg(long)]
    pub(super) tag: Option<String>,
    #[arg(long)]
    pub(super) format: Option<String>,
    #[arg(long)]
    pub(super) color: Option<String>,
    #[arg(long)]
    pub(super) threads: Option<String>,
    #[arg(long)]
    pub(super) blob: Option<String>,
    pub(super) positionals: Vec<String>,
}

/// Parses `args` into a [`RawArgs`], mapping any `clap` failure (an
/// unrecognized flag, or a value-taking flag given with no value) to
/// [`ArgsError::Usage`].
pub(super) fn parse_raw(args: &[String]) -> Result<RawArgs, ArgsError> {
    RawArgs::try_parse_from(args).map_err(|_| ArgsError::Usage)
}
