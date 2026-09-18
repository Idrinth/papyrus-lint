//! Recognizes the CLI's raw argument syntax via `clap`: `init`, `preset add`,
//! and `doctor` as real subcommands, and a plain lint/fix/`--blob` invocation
//! as the default command (no subcommand word). `clap` owns `--flag`,
//! `--flag=value`/`--flag value`, repeatable options, required positionals,
//! and a missing value. Business-rule validation of a lint/fix/`--blob` run
//! (mutually exclusive flags, enum coercion, numeric ranges) stays
//! [`super::validate`]'s job.

use std::path::PathBuf;

use clap::{Args, Parser, Subcommand};
use papyrus_lint_config::presets;

use super::{ArgsError, ParsedCommand};

/// Top-level clap parser for a full `PapyrusLinterCLI` invocation, including
/// the `init`/`preset`/`doctor` subcommands. Lint/fix/`--blob` flags live on
/// the parent (via [`RawArgs`]) so they can still appear before, after, or
/// mixed in with `fix` and the target path in any order. `args_conflicts_with_subcommands`
/// keeps a flag given *before* `init`/`preset`/`doctor` from silently attaching
/// to that subcommand — those names still have to be `args[0]`, matching the
/// previous first-token dispatch.
#[derive(Parser, Debug)]
#[command(
    no_binary_name = true,
    disable_help_flag = true,
    disable_version_flag = true,
    disable_help_subcommand = true,
    args_conflicts_with_subcommands = true
)]
pub(super) struct Cli {
    #[command(subcommand)]
    pub(super) command: Option<RootCommand>,
    #[command(flatten)]
    pub(super) run: RawArgs,
}

#[derive(Subcommand, Debug)]
pub(super) enum RootCommand {
    #[command(
        disable_help_flag = true,
        disable_version_flag = true,
        disable_help_subcommand = true
    )]
    Init(InitRawArgs),
    #[command(
        disable_help_flag = true,
        disable_version_flag = true,
        disable_help_subcommand = true
    )]
    Preset {
        #[command(subcommand)]
        command: PresetCommand,
    },
    #[command(
        disable_help_flag = true,
        disable_version_flag = true,
        disable_help_subcommand = true
    )]
    Doctor(DoctorRawArgs),
}

#[derive(Subcommand, Debug)]
pub(super) enum PresetCommand {
    #[command(disable_help_flag = true, disable_version_flag = true)]
    Add(PresetAddRawArgs),
}

/// `init`'s own flags. Also usable standalone via [`Parser::try_parse_from`]
/// on the arguments following `init` (see [`crate::init::parse_init_preset`]).
#[derive(Parser, Debug)]
#[command(
    no_binary_name = true,
    disable_help_flag = true,
    disable_version_flag = true
)]
pub(crate) struct InitRawArgs {
    #[arg(long)]
    pub(crate) preset: Option<String>,
}

/// `preset add`'s own flags/positionals. Unlike [`RawArgs`]'s catch-all
/// positionals, an unrecognized `--flag` here is a hard `clap` parse error
/// rather than a stray positional, matching `preset add`'s narrower grammar
/// (only `--yes` plus exactly two positionals are valid).
#[derive(Parser, Debug)]
#[command(
    no_binary_name = true,
    disable_help_flag = true,
    disable_version_flag = true
)]
pub(crate) struct PresetAddRawArgs {
    #[arg(long)]
    pub(crate) yes: bool,
    pub(crate) name: String,
    pub(crate) source_path: PathBuf,
}

/// `doctor`'s own flags and its required path positional.
#[derive(Parser, Debug)]
#[command(
    no_binary_name = true,
    disable_help_flag = true,
    disable_version_flag = true
)]
pub(crate) struct DoctorRawArgs {
    #[arg(long)]
    pub(crate) json: bool,
    #[arg(long)]
    pub(crate) config: Option<String>,
    #[arg(long = "script-root")]
    pub(crate) script_root: Vec<String>,
    pub(crate) input_path: PathBuf,
}

/// The main lint/fix/`--blob` invocation's flags and options. Flattened into
/// [`Cli`] so clap can tell `init`/`preset`/`doctor` apart from a path named
/// something else, while still accepting `fix` as a positional mixed in with
/// flags. `-h`/`--help` and `-V`/`--version` are declared here (rather than
/// left to clap's own built-in handling) so the caller can keep reporting
/// them the same way it always has.
#[derive(Args, Debug)]
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

/// Wrapper so [`parse_raw`] can keep parsing a lint/fix/`--blob` invocation
/// on its own (without treating `init`/`preset`/`doctor` as subcommands),
/// which is what [`super::parse_run_args`]'s tests exercise.
#[derive(Parser, Debug)]
#[command(
    no_binary_name = true,
    disable_help_flag = true,
    disable_version_flag = true,
    disable_help_subcommand = true
)]
struct LintOnlyArgs {
    #[command(flatten)]
    run: RawArgs,
}

/// Parses `args` into a [`RawArgs`], mapping any `clap` failure (an
/// unrecognized flag, or a value-taking flag given with no value) to
/// [`ArgsError::Usage`].
pub(super) fn parse_raw(args: &[String]) -> Result<RawArgs, ArgsError> {
    LintOnlyArgs::try_parse_from(args)
        .map(|parsed| parsed.run)
        .map_err(|_| ArgsError::Usage)
}

/// What [`parse_cli`] parsed a full invocation into, including the
/// `init`/`preset add`/`doctor` subcommands that used to be peeled off by
/// matching `args[0]` by hand.
#[derive(Debug)]
pub(crate) enum ParsedCli {
    Init(presets::Preset),
    PresetAdd {
        name: String,
        source_path: PathBuf,
        overwrite: bool,
    },
    Doctor(DoctorRawArgs),
    Run(ParsedCommand),
}

/// Parses a full `PapyrusLinterCLI` argument list (excluding the binary name)
/// through clap's subcommand tree, then — for a default lint/fix/`--blob`
/// invocation — [`super::validate`].
pub(crate) fn parse_cli(args: &[String]) -> Result<ParsedCli, ArgsError> {
    let cli = Cli::try_parse_from(args).map_err(|_| ArgsError::Usage)?;
    match cli.command {
        Some(RootCommand::Init(_)) => parse_init_preset(&args[1..])
            .map(ParsedCli::Init)
            .map_err(|_| ArgsError::Usage),
        Some(RootCommand::Preset {
            command: PresetCommand::Add(_),
        }) => parse_preset_add_args(&args[2..])
            .map(|(name, source_path, overwrite)| ParsedCli::PresetAdd {
                name,
                source_path,
                overwrite,
            })
            .map_err(|_| ArgsError::Usage),
        Some(RootCommand::Doctor(raw)) => Ok(ParsedCli::Doctor(raw)),
        None => super::parse_run_args(args).map(ParsedCli::Run),
    }
}

pub(crate) fn parse_init_preset(rest: &[String]) -> Result<presets::Preset, InitPresetError> {
    let raw = InitRawArgs::try_parse_from(rest).map_err(|_| InitPresetError::Usage)?;
    match raw.preset {
        Some(value) => presets::Preset::parse(&value).ok_or(InitPresetError::Usage),
        None => Ok(presets::Preset::default()),
    }
}

/// Why [`parse_init_preset`] rejected `init`'s arguments: a missing
/// `--preset` value, an argument that isn't `--preset`/`--preset=<name>` at
/// all, or a blank `--preset=` value all get the generic [`crate::USAGE`] text.
#[derive(Debug, PartialEq, Eq)]
pub(crate) enum InitPresetError {
    Usage,
}

/// Parses the arguments following `preset add` into
/// `(name, source_path, overwrite)`.
pub(crate) fn parse_preset_add_args(
    rest: &[String],
) -> Result<(String, PathBuf, bool), PresetAddArgsError> {
    let raw = PresetAddRawArgs::try_parse_from(rest).map_err(|_| PresetAddArgsError::Usage)?;
    Ok((raw.name, raw.source_path, raw.yes))
}

/// Why [`parse_preset_add_args`] rejected `preset add`'s arguments.
#[derive(Debug, PartialEq, Eq)]
pub(crate) enum PresetAddArgsError {
    Usage,
}
