use std::fs;
use std::io::Write;
use std::path::Path;

use papyrus_lint_core::config;
use papyrus_lint_core::content_hash;

use crate::output::*;
use crate::VERSION;

/// Lints `source` directly as an in-memory "blob" of Papyrus source text —
/// e.g. a script buffer piped in from an editor or another tool — instead of
/// resolving it from an achlist/`.psc`/directory path on disk, for
/// [`run`]'s `--blob <source>` flag. There's no real file backing `source`,
/// so none of the project-level machinery a normal run needs applies: no
/// project root discovery, no cross-script argument/return type resolution
/// (a call into another script is treated the same as one into an unknown
/// script, since [`papyrus_lints::lint`] never resolves external
/// signatures), no `conflicting_script_versions`/`stale_compiled_output`/
/// `script_filename_mismatch` project lints, and no `compile_check`. The
/// reported path is the literal string `<blob>`, since there's no real path
/// to display.
///
/// `config_path` mirrors `--config <path>`: given, lint configuration is
/// loaded from that file; omitted, the engine's default configuration is
/// used, since there's no project root to discover a `papyrus-lint.yaml`/
/// `.yml` from. `tag_filter` (already validated/normalized via
/// [`normalize_tag_filter`]) restricts the reported diagnostics to one
/// tagged kind, the same as a normal run's `--tag`. `stdout_is_terminal`
/// feeds `--color auto`'s terminal detection the same way [`run`] itself
/// does.
///
/// Returns `0` if no diagnostic counted as a failure (per
/// `fail_on_warning`/`fail_on_info`), `1` if any did, or `2` on a `--config`
/// load failure or a failure to write `--output <path>`.
#[allow(clippy::too_many_arguments)]
pub(crate) fn run_blob(
    source: &str,
    config_path: Option<&Path>,
    output_format: OutputFormat,
    hash_source: bool,
    quiet_warnings: bool,
    quiet_info: bool,
    tag_filter: Option<&str>,
    color_choice: ColorChoice,
    output_path: Option<&Path>,
    stdout_is_terminal: bool,
    stdout: &mut impl Write,
    stderr: &mut impl Write,
) -> u8 {
    pub(crate) const BLOB_PATH: &str = "<blob>";

    let lint_config = match config_path {
        Some(path) => match config::load_config_from_path(path) {
            Ok(config) => config,
            Err(err) => {
                let _ = writeln!(stderr, "error: failed to load lint config: {err}");
                return 2;
            }
        },
        None => papyrus_lints::Config::default(),
    };

    let mut diagnostics = papyrus_lints::lint(source, &lint_config);
    if let Some(tag) = tag_filter {
        diagnostics.retain(|diagnostic| {
            papyrus_lints::tags::tags_for(diagnostic.rule).is_some_and(|rule_tags| {
                rule_tags
                    .kinds
                    .iter()
                    .any(|kind| kind.eq_ignore_ascii_case(tag))
            })
        });
    }
    diagnostics.sort_by_key(|d| (d.line, d.column));

    // Quiet flags only affect presentation; a hidden diagnostic still
    // participates in the configured failure threshold and exit code, the
    // same as a normal file's diagnostics do in `run`.
    let should_fail = diagnostics
        .iter()
        .any(|diagnostic| lint_config.should_fail_on(diagnostic));
    diagnostics.retain(|diagnostic| {
        !((quiet_warnings && diagnostic.level() == "warning")
            || (quiet_info && diagnostic.level() == "info"))
    });

    let use_color = match color_choice {
        ColorChoice::Always => true,
        ColorChoice::Never => false,
        ColorChoice::Auto => {
            output_path.is_none() && stdout_is_terminal && std::env::var_os("NO_COLOR").is_none()
        }
    };

    let total_diagnostics = diagnostics.len();
    let json_diagnostics: Vec<JsonDiagnostic> = diagnostics
        .iter()
        .map(|d| JsonDiagnostic {
            line: d.line,
            column: d.column,
            rule: d.rule,
            level: d.level(),
            message: d.message.clone(),
            doc_url: doc_url_for(d.rule),
        })
        .collect();

    let mut report_buf: Vec<u8> = Vec::new();

    match output_format {
        OutputFormat::Json => {
            let report = JsonReport {
                files: vec![JsonFileReport {
                    path: BLOB_PATH.to_string(),
                    diagnostics: json_diagnostics,
                    diff: None,
                }],
                scripts_checked: 1,
                files_with_diagnostics: if total_diagnostics > 0 { 1 } else { 0 },
                total_diagnostics,
                files_fixed: None,
                dry_run: false,
                success: !should_fail,
            };
            let _ = writeln!(
                report_buf,
                "{}",
                serde_json::to_string_pretty(&report).unwrap_or_else(|_| "{}".to_string())
            );
        }
        OutputFormat::Ai => {
            let ai_files = if json_diagnostics.is_empty() {
                Vec::new()
            } else {
                let rule_counts = rule_counts(&json_diagnostics);
                let severity_counts = severity_counts(&json_diagnostics);
                let ai_source = if hash_source {
                    AiSource::Hash {
                        algorithm: "md5",
                        hash: content_hash::md5_hex(source),
                    }
                } else {
                    AiSource::Content {
                        content: source.to_string(),
                    }
                };
                vec![AiFileReport {
                    path: BLOB_PATH.to_string(),
                    severity_counts,
                    rule_counts,
                    diagnostics: json_diagnostics,
                    source: ai_source,
                }]
            };
            let total_rule_counts =
                rule_counts(ai_files.iter().flat_map(|file| file.diagnostics.iter()));
            let total_severity_counts =
                severity_counts(ai_files.iter().flat_map(|file| file.diagnostics.iter()));
            let mut triggered_rules: Vec<&'static str> = ai_files
                .iter()
                .flat_map(|file| file.diagnostics.iter().map(|diagnostic| diagnostic.rule))
                .collect();
            triggered_rules.sort_unstable();
            triggered_rules.dedup();
            let rule_details = triggered_rules
                .into_iter()
                .filter_map(papyrus_lints::tags::tags_for)
                .map(|tags| AiRuleDetails {
                    rule: tags.rule,
                    description: tags.description,
                    kinds: tags.kinds,
                    importance: tags.importance,
                    auto_fixable: tags.auto_fixable(),
                    doc_url: tags.doc_url(),
                })
                .collect();
            let report = AiReport {
                schema:
                    "https://papyrus-lint.idrinth.de/schema/papyrus-lint-ai-export.v3.schema.json",
                header: AiHeader {
                    tool: "Papyrus Lint",
                    version: VERSION,
                    website: "https://papyrus-lint.idrinth.de",
                    target_game: "Skyrim SE/AE",
                    generated_at: generated_at(),
                },
                configuration: ai_configuration(&lint_config),
                findings: AiFindings {
                    files: ai_files,
                    total_diagnostics,
                    severity_counts: total_severity_counts,
                    rule_counts: total_rule_counts,
                },
                rule_details,
            };
            let _ = writeln!(
                report_buf,
                "{}",
                serde_json::to_string_pretty(&report).unwrap_or_else(|_| "{}".to_string())
            );
        }
        OutputFormat::Plain => {
            for diagnostic in &diagnostics {
                let _ = writeln!(
                    report_buf,
                    "{}",
                    format_diagnostic_line(BLOB_PATH, diagnostic, use_color)
                );
            }
            let summary_color = if total_diagnostics == 0 {
                ANSI_GREEN
            } else if should_fail {
                ANSI_RED
            } else {
                ANSI_YELLOW
            };
            let summary = if total_diagnostics == 0 {
                "PapyrusLinterCLI: no problems found in the given blob.".to_string()
            } else {
                format!("PapyrusLinterCLI: {total_diagnostics} problem(s) found in the given blob.")
            };
            let _ = writeln!(
                report_buf,
                "{}",
                colorize(&summary, summary_color, use_color)
            );
        }
    }

    if let Some(output_path) = output_path {
        if let Err(err) = fs::write(output_path, &report_buf) {
            let _ = writeln!(
                stderr,
                "error: failed to write {}: {err}",
                output_path.display()
            );
            return 2;
        }
    } else {
        let _ = stdout.write_all(&report_buf);
    }

    if should_fail {
        1
    } else {
        0
    }
}
