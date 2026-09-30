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
pub(super) fn validate(mut raw: RawArgs, fix: bool) -> Result<ParsedCommand, ArgsError> {
    let output_format = parse_output_format(raw.format.as_deref())?;
    if raw.hash_source && output_format != OutputFormat::Ai {
        return Err(ArgsError::HashSourceRequiresAi);
    }
    let color_choice = parse_color_choice(raw.color.as_deref())?;

    if let Some(source) = raw.blob.take() {
        return validate_blob(raw, source, fix, output_format, color_choice);
    }

    validate_lint(raw, fix, output_format, color_choice)
}

fn validate_blob(
    raw: RawArgs,
    source: String,
    fix: bool,
    output_format: OutputFormat,
    color_choice: ColorChoice,
) -> Result<ParsedCommand, ArgsError> {
    if fix {
        return Err(ArgsError::BlobWithFixFlags);
    }
    parse_blob_command(
        source,
        &raw.positionals,
        raw.dry_run,
        raw.type_filter.as_deref(),
        raw.line.as_deref(),
        raw.progress,
        &raw.script_root,
        raw.threads.as_deref(),
        raw.tag,
        raw.config.map(PathBuf::from),
        output_format,
        raw.hash_source,
        raw.quiet_warnings,
        raw.quiet_info,
        color_choice,
        raw.output.map(PathBuf::from),
    )
}

fn validate_lint(
    raw: RawArgs,
    fix: bool,
    output_format: OutputFormat,
    color_choice: ColorChoice,
) -> Result<ParsedCommand, ArgsError> {
    let config_path = raw.config.map(PathBuf::from);
    let output_path = raw.output.map(PathBuf::from);

    let input_path = parse_lint_positionals(
        &raw.positionals,
        fix,
        raw.type_filter.as_deref(),
        raw.line.as_deref(),
        raw.dry_run,
    )?;

    if raw.progress && output_path.is_none() {
        return Err(ArgsError::ProgressRequiresOutput);
    }

    if raw.type_filter.is_some() && raw.tag.is_some() {
        return Err(ArgsError::TypeAndTagConflict);
    }

    let tag_filter = normalize_tag_filter(raw.tag).map_err(ArgsError::UnknownTag)?;

    let rule_filter = parse_rule_filter(raw.type_filter)?;
    let target_line = parse_line_filter(raw.line)?;
    let thread_count = parse_thread_count(raw.threads)?;

    Ok(ParsedCommand::Lint(LintArgs {
        fix,
        input_path,
        output_format,
        quiet_warnings: raw.quiet_warnings,
        quiet_info: raw.quiet_info,
        short_paths: raw.short_paths,
        progress: raw.progress,
        dry_run: raw.dry_run,
        hash_source: raw.hash_source,
        config_path,
        output_path,
        cli_script_roots: raw.script_root,
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
        Some("short") => Ok(OutputFormat::Short),
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
