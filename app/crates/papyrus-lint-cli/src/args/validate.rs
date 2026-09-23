//! The business rules layered on top of [`super::parse`]'s raw `clap`
//! extraction: mutually exclusive flags, enum coercion (`--format`,
//! `--color`), numeric ranges (`--line`, `--threads`), and `--blob`'s own
//! narrower grammar. Split out from [`super::parse`] so "what shape can the
//! arguments take" and "what combinations of that shape are actually
//! allowed" stay easy to tell apart.

use std::path::PathBuf;

use super::parse::RawArgs;
use super::{ArgsError, BlobArgs, LintArgs, ParsedCommand};
use crate::output::{normalize_tag_filter, ColorChoice, OutputFormat};

/// Validates `raw` into a [`ParsedCommand`], performing every usage check a
/// lint/fix run or `--blob` needs before any of the actual work
/// (resolving paths, loading config, linting) begins. `fix` is supplied by
/// the `lint` vs `fix` subcommand rather than a positional token.
pub(super) fn validate(raw: RawArgs, fix: bool) -> Result<ParsedCommand, ArgsError> {
    if raw.help {
        return Err(ArgsError::Usage);
    }
    if raw.version {
        return Ok(ParsedCommand::Version);
    }

    let quiet_warnings = raw.quiet_warnings;
    let quiet_info = raw.quiet_info;
    let short_paths = raw.short_paths;
    let progress = raw.progress;
    let dry_run = raw.dry_run;
    let hash_source = raw.hash_source;
    let config_path: Option<PathBuf> = raw.config.map(PathBuf::from);
    let output_path: Option<PathBuf> = raw.output.map(PathBuf::from);
    let cli_script_roots: Vec<String> = raw.script_root;
    let type_filter = raw.type_filter;
    let line_filter = raw.line;
    let tag_filter = raw.tag;
    let format_flag = raw.format;
    let color_flag = raw.color;
    let threads_flag = raw.threads;
    let blob_flag = raw.blob;
    let args = raw.positionals;

    let output_format = parse_output_format(format_flag.as_deref())?;

    if hash_source && output_format != OutputFormat::Ai {
        return Err(ArgsError::HashSourceRequiresAi);
    }

    let color_choice = parse_color_choice(color_flag.as_deref())?;

    if let Some(source) = blob_flag {
        return parse_blob_command(
            source,
            &args,
            dry_run,
            type_filter.as_deref(),
            line_filter.as_deref(),
            progress,
            &cli_script_roots,
            threads_flag.as_deref(),
            tag_filter,
            config_path,
            output_format,
            hash_source,
            quiet_warnings,
            quiet_info,
            color_choice,
            output_path,
        );
    }

    let input_path = parse_lint_positionals(
        &args,
        fix,
        type_filter.as_deref(),
        line_filter.as_deref(),
        dry_run,
    )?;

    if progress && output_path.is_none() {
        return Err(ArgsError::ProgressRequiresOutput);
    }

    if type_filter.is_some() && tag_filter.is_some() {
        return Err(ArgsError::TypeAndTagConflict);
    }

    let tag_filter = normalize_tag_filter(tag_filter).map_err(ArgsError::UnknownTag)?;

    let rule_filter = parse_rule_filter(type_filter)?;
    let target_line = parse_line_filter(line_filter)?;
    let thread_count = parse_thread_count(threads_flag)?;

    Ok(ParsedCommand::Lint(LintArgs {
        fix,
        input_path,
        output_format,
        quiet_warnings,
        quiet_info,
        short_paths,
        progress,
        dry_run,
        hash_source,
        config_path,
        output_path,
        cli_script_roots,
        tag_filter,
        rule_filter,
        target_line,
        color_choice,
        thread_count,
    }))
}

fn parse_lint_positionals(
    args: &[String],
    fix: bool,
    type_filter: Option<&str>,
    line_filter: Option<&str>,
    dry_run: bool,
) -> Result<PathBuf, ArgsError> {
    let input_path = match args {
        [path] => PathBuf::from(path),
        _ => return Err(ArgsError::Usage),
    };
    if !fix && (type_filter.is_some() || line_filter.is_some() || dry_run) {
        return Err(ArgsError::Usage);
    }
    Ok(input_path)
}

fn parse_output_format(format_flag: Option<&str>) -> Result<OutputFormat, ArgsError> {
    match format_flag {
        None | Some("plain") => Ok(OutputFormat::Plain),
        Some("json") => Ok(OutputFormat::Json),
        Some("ai") => Ok(OutputFormat::Ai),
        Some(value) => Err(ArgsError::InvalidFormat(value.to_string())),
    }
}

fn parse_color_choice(color_flag: Option<&str>) -> Result<ColorChoice, ArgsError> {
    match color_flag {
        None | Some("auto") => Ok(ColorChoice::Auto),
        Some("always") => Ok(ColorChoice::Always),
        Some("never") => Ok(ColorChoice::Never),
        Some(value) => Err(ArgsError::InvalidColor(value.to_string())),
    }
}

#[allow(clippy::too_many_arguments)]
fn parse_blob_command(
    source: String,
    args: &[String],
    dry_run: bool,
    type_filter: Option<&str>,
    line_filter: Option<&str>,
    progress: bool,
    cli_script_roots: &[String],
    threads_flag: Option<&str>,
    tag_filter: Option<String>,
    config_path: Option<PathBuf>,
    output_format: OutputFormat,
    hash_source: bool,
    quiet_warnings: bool,
    quiet_info: bool,
    color_choice: ColorChoice,
    output_path: Option<PathBuf>,
) -> Result<ParsedCommand, ArgsError> {
    if !args.is_empty() {
        return Err(ArgsError::BlobWithPathArgument);
    }
    if dry_run || type_filter.is_some() || line_filter.is_some() {
        return Err(ArgsError::BlobWithFixFlags);
    }
    if progress || !cli_script_roots.is_empty() || threads_flag.is_some() {
        return Err(ArgsError::BlobWithScriptRootProgressThreads);
    }
    let tag_filter = normalize_tag_filter(tag_filter).map_err(ArgsError::UnknownTag)?;
    Ok(ParsedCommand::Blob(BlobArgs {
        source,
        config_path,
        output_format,
        hash_source,
        quiet_warnings,
        quiet_info,
        tag_filter,
        color_choice,
        output_path,
    }))
}

fn parse_rule_filter(type_filter: Option<String>) -> Result<Option<&'static str>, ArgsError> {
    let Some(value) = type_filter else {
        return Ok(None);
    };
    let normalized = value.replace('_', "-").to_ascii_lowercase();
    match papyrus_lints::FIXABLE_RULE_IDS
        .iter()
        .find(|rule| **rule == normalized)
    {
        Some(rule) => Ok(Some(*rule)),
        None => {
            if papyrus_lints::KNOWN_RULE_IDS
                .iter()
                .any(|rule| *rule == normalized)
            {
                Err(ArgsError::RuleHasNoFix(value))
            } else {
                Err(ArgsError::UnknownRule(value))
            }
        }
    }
}

fn parse_line_filter(line_filter: Option<String>) -> Result<Option<usize>, ArgsError> {
    match line_filter {
        Some(value) => match value.parse::<usize>() {
            Ok(line) if line >= 1 => Ok(Some(line)),
            _ => Err(ArgsError::InvalidLine(value)),
        },
        None => Ok(None),
    }
}

fn parse_thread_count(threads_flag: Option<String>) -> Result<usize, ArgsError> {
    match threads_flag {
        Some(value) => match value.parse::<usize>() {
            Ok(threads) if threads >= 1 => Ok(threads),
            _ => Err(ArgsError::InvalidThreads(value)),
        },
        None => Ok(papyrus_lint_core::parallel::default_thread_count()),
    }
}
