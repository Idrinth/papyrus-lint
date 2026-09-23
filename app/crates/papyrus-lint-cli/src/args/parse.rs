//! Recognizes the CLI's raw argument syntax via `clap`: `init`, `preset add`,
//! `doctor`, `lint`, `fix`, `help`, and `version` as real subcommands. A
//! lint/`--blob` run requires the `lint` subcommand; `fix` is its own
//! subcommand. `clap` owns `--flag`, `--flag=value`/`--flag value`,
//! repeatable options, required positionals, and a missing value.
//! Business-rule validation of a lint/fix/`--blob` run (mutually exclusive
//! flags, enum coercion, numeric ranges) stays [`super::validate`]'s job.

use std::path::PathBuf;

use clap::{Args, Parser, Subcommand};
use papyrus_lint_config::presets;
use papyrus_lints::Game;

use super::{ArgsError, DoctorArgs, ParsedCommand};

/// Top-level clap parser for a full `PapyrusLinterCLI` invocation. Every
/// action (`init`/`preset`/`doctor`/`lint`/`fix`/`help`/`version`) is a
/// subcommand.
#[derive(Parser, Debug)]
#[command(
    no_binary_name = true,
    disable_help_flag = true,
    disable_version_flag = true,
    disable_help_subcommand = true
)]
pub(super) struct Cli {
    #[command(subcommand)]
    pub(super) command: Option<RootCommand>,
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
    #[command(
        disable_help_flag = true,
        disable_version_flag = true,
        disable_help_subcommand = true
    )]
    Lint(RawArgs),
    #[command(
        disable_help_flag = true,
        disable_version_flag = true,
        disable_help_subcommand = true
    )]
    Fix(RawArgs),
    #[command(
        disable_help_flag = true,
        disable_version_flag = true,
        disable_help_subcommand = true
    )]
    Help,
    #[command(
        disable_help_flag = true,
        disable_version_flag = true,
        disable_help_subcommand = true
    )]
    Version,
}

#[derive(Subcommand, Debug)]
pub(super) enum PresetCommand {
    #[command(disable_help_flag = true, disable_version_flag = true)]
    Add(PresetAddRawArgs),
    #[command(disable_help_flag = true, disable_version_flag = true)]
    List,
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
    #[arg(long)]
    pub(crate) game: String,
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
    pub(crate) format: Option<String>,
    #[arg(long)]
    pub(crate) config: Option<String>,
    #[arg(long = "script-root")]
    pub(crate) script_root: Vec<String>,
    pub(crate) input_path: PathBuf,
}

/// The main lint/fix/`--blob` invocation's flags and options.
#[derive(Args, Debug)]
pub(super) struct RawArgs {
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

/// What [`parse_cli`] parsed a full invocation into, including the
/// `init`/`preset add`/`doctor` subcommands that used to be peeled off by
/// matching `args[0]` by hand.
#[derive(Debug)]
pub(crate) enum ParsedCli {
    Init {
        preset: presets::Preset,
        game: Game,
    },
    PresetAdd {
        name: String,
        source_path: PathBuf,
        overwrite: bool,
    },
    PresetList,
    Doctor(DoctorArgs),
    Run(ParsedCommand),
}

/// Parses a full `PapyrusLinterCLI` argument list (excluding the binary name)
/// through clap's subcommand tree, then — for `lint`/`fix` — [`super::validate`].
pub(crate) fn parse_cli(args: &[String]) -> Result<ParsedCli, ArgsError> {
    let cli = Cli::try_parse_from(args).map_err(|_| ArgsError::Usage)?;
    match cli.command {
        Some(RootCommand::Init(_)) => parse_init_preset(&args[1..])
            .map(|(preset, game)| ParsedCli::Init { preset, game })
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
        Some(RootCommand::Preset {
            command: PresetCommand::List,
        }) => Ok(ParsedCli::PresetList),
        Some(RootCommand::Doctor(raw)) => validate_doctor(raw).map(ParsedCli::Doctor),
        Some(RootCommand::Lint(raw)) => super::validate::validate(raw, false).map(ParsedCli::Run),
        Some(RootCommand::Fix(raw)) => super::validate::validate(raw, true).map(ParsedCli::Run),
        Some(RootCommand::Help) => Err(ArgsError::Usage),
        Some(RootCommand::Version) => Ok(ParsedCli::Run(ParsedCommand::Version)),
        None => Err(ArgsError::Usage),
    }
}

fn validate_doctor(raw: DoctorRawArgs) -> Result<DoctorArgs, ArgsError> {
    let json = match raw.format.as_deref() {
        None | Some("plain") => false,
        Some("json") => true,
        Some(value) => return Err(ArgsError::InvalidDoctorFormat(value.to_string())),
    };
    Ok(DoctorArgs {
        json,
        config: raw.config,
        script_root: raw.script_root,
        input_path: raw.input_path,
    })
}

pub(crate) fn parse_init_preset(
    rest: &[String],
) -> Result<(presets::Preset, Game), InitPresetError> {
    let raw = InitRawArgs::try_parse_from(rest).map_err(|_| InitPresetError::Usage)?;
    let game = raw
        .game
        .to_ascii_lowercase()
        .parse::<Game>()
        .map_err(|_| InitPresetError::Usage)?;
    let preset = match raw.preset {
        Some(value) => presets::Preset::parse(&value).ok_or(InitPresetError::Usage)?,
        None => presets::Preset::default(),
    };
    Ok((preset, game))
}

/// Why [`parse_init_preset`] rejected `init`'s arguments: a missing
/// `--game`, an unknown game, a missing `--preset` value, or an extra
/// argument all get the generic [`crate::USAGE`] text.
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
