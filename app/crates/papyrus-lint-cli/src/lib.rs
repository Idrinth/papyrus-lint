//! Library backing the `PapyrusLinterCLI` command-line interface.
//!
//! ```text
//! PapyrusLinterCLI [--json] [--quiet-warnings] [--quiet-info] <path-to-achlist-or-psc>
//! PapyrusLinterCLI [--json] [--quiet-warnings] [--quiet-info] fix <path-to-achlist-or-psc>
//! PapyrusLinterCLI init
//! ```
//!
//! Resolves every `.psc` entry listed in the given `.achlist` file (see
//! [`papyrus_lint_core::achlist`]) — or, if given a single `.psc` file
//! directly, treats that file as the achlist's sole entry — lints each
//! against the project's `papyrus-lint.yaml`/`.yml` configuration, falling
//! back to [`papyrus_lints::Config::default`] if it has none (see
//! [`papyrus_lint_core::config`]) — and prints the diagnostics found, one
//! per line. The project root the config (and the function table below) is
//! looked up under is found by walking up from a resolved `.psc` file's own
//! position for a `scripts/source`/`source/scripts` directory pair (matching
//! [`papyrus_lint_core::script_locator::CANDIDATE_DIRS`], case-insensitively)
//! and taking the directory above that pair, e.g. `Data` for
//! `Data\Scripts\Source\abc.psc` — which also finds the right root for a
//! script nested further still, e.g. a namespaced
//! `Data\Scripts\Source\User\abc.psc`. For a bare `.psc` file given directly,
//! that walk starts from the file itself, falling back to two directories up
//! if no such pair is found in the path at all. For an `.achlist`, the same
//! walk is tried against each of its resolved `.psc` entries first, so a
//! project whose `.achlist` sits somewhere other than the project root (e.g.
//! a user drops it next to a game's `Data` directory while the actual
//! project, and its `papyrus-lint.yaml`, live in a subfolder alongside the
//! `scripts/source`/`source/scripts` tree) still finds the right root;
//! falling back to the achlist's own parent directory (the previous, simpler
//! rule) only if none of its entries match that layout. This is what lets
//! editor plugins that invoke the CLI on a single saved file (see
//! `SublimeLinter-contrib-papyrus-lint/linter.py`) still pick up the
//! project's config regardless of how the project organizes its scripts
//! under `scripts/source`. Calls to functions declared on other scripts under
//! the project root are resolved the same way the desktop app resolves
//! them (see [`papyrus_lint_core::function_table`]), so the CLI's
//! "Argument type check"/"Return type check" results match the app's.
//!
//! With the `fix` subcommand, every automatic fix (see
//! [`papyrus_lints::repair`]) is applied to each resolved script first,
//! rewriting it on disk if it changed, before the (now possibly smaller)
//! set of remaining diagnostics is reported the same way.
//!
//! With the `--json` flag (combinable with `fix`, in either argument
//! order), the diagnostics report is printed to stdout as a single JSON
//! document (see [`JsonReport`]) instead of the plain-text format, so
//! editor plugins and other tooling can consume it without scraping text.
//! `--quiet-warnings` and `--quiet-info` omit diagnostics of the corresponding
//! severity from either report format without changing the process exit code.
//!
//! With `--config <path>` (combinable with `fix`/`--json` in any order),
//! lint configuration is loaded directly from `<path>` instead of being
//! discovered from the project root, letting a caller (e.g. an editor
//! plugin with its own configured override) point at a config file with
//! any name, anywhere on disk. This also skips the project root's own
//! `additional_script_roots` (see below), since that config file is no
//! longer being read at all.
//!
//! With one or more `--script-root <path>` flags (combinable with
//! `fix`/`--json`/`--config` in any order), each given directory (resolved
//! relative to the project root unless already absolute) is searched
//! alongside `scripts/source`/`source/scripts` and the project's configured
//! `additional_script_roots` (see [`papyrus_lint_core::config::load_script_roots`])
//! when resolving cross-script lookups — useful for a script that imports
//! from a shared library location outside the project without adding it to
//! the project's own config file.
//!
//! With `--output <path>` (combinable with `fix`/`--json`/`--config`/
//! `--script-root` in any order), the report (plain text or JSON, per
//! `--json`) is written to `<path>` instead of stdout, so it can be stored
//! without piping. Usage/error text still goes to stderr either way, and
//! the exit code is unaffected.
//!
//! This crate is used both by the standalone `PapyrusLinterCLI` binary
//! (`src/main.rs`) and by the desktop app (`app/src-tauri`), which runs it in
//! place of launching its GUI whenever it's given command-line arguments.

use std::collections::HashMap;
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};

use papyrus_lint_core::function_table::FunctionTable;
use papyrus_lint_core::script_locator::CANDIDATE_DIRS;
use papyrus_lint_core::source_encoding::read_psc_source;
use papyrus_lint_core::{achlist, config};
use serde::Serialize;

/// Walks up `psc_path`'s ancestors looking for a directory pair matching
/// one of [`CANDIDATE_DIRS`] (`scripts/source` or `source/scripts`,
/// matched case-insensitively), and returns the directory above that pair
/// as the project root, or `None` if no such pair appears anywhere in
/// `psc_path`'s ancestry.
///
/// This finds the right root for a script nested further still, e.g. a
/// namespaced Fallout 4 script at `<root>/Scripts/Source/User/MyScript.psc`
/// — unlike a naive "two directories up" rule, which would land on
/// `Scripts` instead of `<root>` for that layout and then fail to discover
/// the project's config or resolve any cross-script lookups against it.
///
/// Used both for a bare `.psc` file given directly on the command line (see
/// [`find_psc_project_root`]) and, per script, for one resolved from an
/// `.achlist` file (see [`run`]) — so an achlist whose own parent directory
/// isn't the real project root (e.g. it was dropped next to a game's `Data`
/// directory while the project itself lives in a subfolder) still resolves
/// to the right root, as long as at least one of its entries sits under a
/// conventionally-named `scripts/source`/`source/scripts` tree.
fn find_candidate_pair_root(psc_path: &Path) -> Option<PathBuf> {
    let candidate_pairs: Vec<(&str, &str)> = CANDIDATE_DIRS
        .iter()
        .filter_map(|dir| dir.split_once('/'))
        .collect();

    let ancestors: Vec<&Path> = psc_path.ancestors().collect();
    for i in 1..ancestors.len().saturating_sub(1) {
        let inner_name = ancestors[i]
            .file_name()
            .and_then(|name| name.to_str())
            .map(str::to_ascii_lowercase);
        let outer_name = ancestors[i + 1]
            .file_name()
            .and_then(|name| name.to_str())
            .map(str::to_ascii_lowercase);
        let (Some(inner_name), Some(outer_name)) = (inner_name, outer_name) else {
            continue;
        };

        let matches_candidate = candidate_pairs
            .iter()
            .any(|(outer, inner)| *outer == outer_name && *inner == inner_name);

        if matches_candidate {
            if let Some(root) = ancestors[i + 1].parent() {
                return Some(root.to_path_buf());
            }
        }
    }

    None
}

/// Finds the project root for a bare `.psc` file given directly on the
/// command line.
///
/// Tries [`find_candidate_pair_root`] first, falling back to the previous
/// fixed "two directories up" behavior when no `scripts/source`/
/// `source/scripts` pair is found in the path at all (e.g. a `.psc` passed
/// from outside any conventionally-named scripts/source tree), so that case
/// is unaffected.
fn find_psc_project_root(psc_path: &Path) -> PathBuf {
    find_candidate_pair_root(psc_path).unwrap_or_else(|| {
        psc_path
            .ancestors()
            .nth(3)
            .filter(|dir| !dir.as_os_str().is_empty())
            .map(Path::to_path_buf)
            .unwrap_or_else(|| PathBuf::from("."))
    })
}

pub const USAGE: &str =
    "Usage: PapyrusLinterCLI [--json] [--quiet-warnings] [--quiet-info] [--config <path>] [--script-root <path>]... [--output <path>] <path-to-achlist-or-psc>\n       \
PapyrusLinterCLI [--json] [--quiet-warnings] [--quiet-info] [--config <path>] [--script-root <path>]... [--output <path>] fix <path-to-achlist-or-psc>\n\n\
PapyrusLinterCLI init\n\n\
Lints every .psc script listed in the given .achlist file, or a single\n\
.psc file given directly, using the project's papyrus-lint.yaml/.yml\n\
configuration (looked up next to the .achlist file, or two directories\n\
up from a bare .psc file, e.g. Data for Data\\Scripts\\Source\\abc.psc;\n\
falling back to defaults if it has none).\n\n\
With the `fix` subcommand, applies every automatic fix (see README.md)\n\
to those scripts first, rewriting each one on disk if it changed, then\n\
reports whatever diagnostics remain the same way.\n\n\
With the `init` subcommand, creates a default papyrus-lint.yaml in the\n\
current working directory without overwriting an existing config.\n\n\
Options:\n\
  -h, --help              Show this help message\n\
  -V, --version           Print the PapyrusLinterCLI version\n\
  --json                  Print the report to stdout as JSON instead of plain text\n\
  --quiet-warnings        Hide warning-level diagnostics from the report\n\
  --quiet-info            Hide info-level diagnostics from the report\n\
  --config <path>         Load lint configuration from this file instead of\n\
                          discovering papyrus-lint.yaml/.yml from the project root\n\
                          (also disables the project root's additional_script_roots;\n\
                          use --script-root to add any back explicitly)\n\
  --script-root <path>    An extra directory (relative to the project root,\n\
                          or absolute) to search for .psc files, besides\n\
                          scripts/source, source/scripts, and the project's\n\
                          configured additional_script_roots. Repeatable.\n\
  --output <path>         Write the report (plain text or JSON, per --json) to\n\
                          this file instead of stdout.\n\n\
Exit status: 0 if no problems were found (or none met the configured\n\
fail_on_warning/fail_on_info threshold), 1 if any did, 2 on a usage or\n\
I/O error.\n\n\
Contact:\n\
  Discord    https://discord.gg/idrinth\n\
  NexusMods  https://www.nexusmods.com/skyrimspecialedition/mods/189862\n\
  GitHub     https://github.com/idrinth/papyrus-lint\n";

/// The crate's version, as set in `crates/papyrus-lint-cli/Cargo.toml`
/// (kept in sync with the desktop app's version at release time). Printed
/// by `--version`/`-V`.
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

/// A single diagnostic as printed by `--json`, mirroring the plain-text
/// `<path>:<line>:<column>: [<rule>] <message>` line but with `level`
/// (see [`papyrus_lints::Diagnostic::level`]) broken out as its own field
/// rather than left for a consumer to parse back out of `message`.
#[derive(Debug, Serialize)]
pub struct JsonDiagnostic {
    pub line: usize,
    pub column: usize,
    pub rule: &'static str,
    pub level: &'static str,
    pub message: String,
}

/// One resolved script's diagnostics, as printed by `--json`. Every
/// resolved script gets an entry, even one with no diagnostics, so a
/// consumer (e.g. an editor plugin) can clear stale diagnostics for a
/// file that's since become clean.
#[derive(Debug, Serialize)]
pub struct JsonFileReport {
    pub path: String,
    pub diagnostics: Vec<JsonDiagnostic>,
}

/// The full report printed to stdout by `--json`, in place of the
/// plain-text diagnostics lines and summary.
#[derive(Debug, Serialize)]
pub struct JsonReport {
    pub files: Vec<JsonFileReport>,
    pub scripts_checked: usize,
    pub files_with_diagnostics: usize,
    pub total_diagnostics: usize,
    /// Only present when run with the `fix` subcommand.
    pub files_fixed: Option<usize>,
    /// Whether the run would exit `0`: no diagnostics counted as a
    /// failure per `fail_on_warning`/`fail_on_info` (see
    /// [`papyrus_lints::Config::should_fail_on`]).
    pub success: bool,
}

/// Runs the CLI against `args` (the program's arguments, excluding the
/// binary name itself), writing lint output to `stdout` and usage/error
/// text to `stderr`. Returns the process exit code: `0` if linting found
/// no diagnostics that count as a failure (or `--version`/`-V` was given),
/// `1` if it found at least one, or `2` on a usage or I/O error. A
/// `[warning]`/`[info]`-level diagnostic only counts as a failure when the
/// project's `papyrus-lint.yaml` sets `fail_on_warning`/`fail_on_info`
/// (both `false` by default); an `[error]`-level diagnostic, or one with no
/// level tag, always counts. Diagnostics are printed regardless of whether
/// they affect the exit code unless their level is hidden by a quiet flag.
pub fn run(args: &[String], stdout: &mut impl Write, stderr: &mut impl Write) -> u8 {
    if args == ["init"] {
        let current_dir = match std::env::current_dir() {
            Ok(dir) => dir,
            Err(err) => {
                let _ = writeln!(
                    stderr,
                    "error: failed to determine current directory: {err}"
                );
                return 2;
            }
        };
        return initialize_config(&current_dir, stdout, stderr);
    }

    let json = args.iter().any(|arg| arg == "--json");
    let quiet_warnings = args.iter().any(|arg| arg == "--quiet-warnings");
    let quiet_info = args.iter().any(|arg| arg == "--quiet-info");

    let mut config_path: Option<PathBuf> = None;
    let mut output_path: Option<PathBuf> = None;
    let mut cli_script_roots: Vec<String> = Vec::new();
    let mut positional_and_flags: Vec<String> = Vec::with_capacity(args.len());
    let mut input = args
        .iter()
        .filter(|arg| !matches!(arg.as_str(), "--json" | "--quiet-warnings" | "--quiet-info"))
        .cloned();
    while let Some(arg) = input.next() {
        if arg == "--config" {
            let Some(value) = input.next() else {
                let _ = write!(stderr, "{USAGE}");
                return 2;
            };
            config_path = Some(PathBuf::from(value));
        } else if arg == "--script-root" {
            let Some(value) = input.next() else {
                let _ = write!(stderr, "{USAGE}");
                return 2;
            };
            cli_script_roots.push(value);
        } else if arg == "--output" {
            let Some(value) = input.next() else {
                let _ = write!(stderr, "{USAGE}");
                return 2;
            };
            output_path = Some(PathBuf::from(value));
        } else {
            positional_and_flags.push(arg);
        }
    }
    let args = positional_and_flags;

    let (fix, input_path) = match args.as_slice() {
        [flag] if flag == "--version" || flag == "-V" => {
            let _ = writeln!(stdout, "PapyrusLinterCLI {VERSION}");
            return 0;
        }
        [sub, path] if sub == "fix" => (true, PathBuf::from(path)),
        [path] if path != "-h" && path != "--help" && path != "fix" => (false, PathBuf::from(path)),
        _ => {
            let _ = write!(stderr, "{USAGE}");
            return 2;
        }
    };

    let is_psc_file = input_path
        .extension()
        .and_then(|ext| ext.to_str())
        .is_some_and(|ext| ext.eq_ignore_ascii_case("psc"));

    let script_paths: Vec<PathBuf> = if is_psc_file {
        vec![input_path.clone()]
    } else {
        let entries = match achlist::parse_achlist(&input_path) {
            Ok(entries) => entries,
            Err(err) => {
                let _ = writeln!(stderr, "error: {err}");
                return 2;
            }
        };

        entries
            .into_iter()
            .filter(|path| {
                path.extension()
                    .and_then(|ext| ext.to_str())
                    .is_some_and(|ext| ext.eq_ignore_ascii_case("psc"))
            })
            .collect()
    };

    // A bare .psc file's project root is found by walking up for a
    // `scripts/source`/`source/scripts` directory pair (see
    // `find_psc_project_root`) so it still works when the script is nested
    // deeper still, e.g. under a namespaced subfolder. An .achlist's own
    // entries are tried the same way first, so a project whose .achlist
    // doesn't live in the project root still resolves correctly; only if
    // none of its resolved scripts sit under such a pair do we fall back to
    // the achlist's own parent directory (the conventional layout).
    let project_root = if is_psc_file {
        find_psc_project_root(&input_path)
    } else {
        script_paths
            .iter()
            .find_map(|path| find_candidate_pair_root(path))
            .unwrap_or_else(|| {
                input_path
                    .ancestors()
                    .nth(1)
                    .filter(|dir| !dir.as_os_str().is_empty())
                    .map(Path::to_path_buf)
                    .unwrap_or_else(|| PathBuf::from("."))
            })
    };

    let lint_config = match config_path.as_deref().map_or_else(
        || config::load_config(&project_root),
        config::load_config_from_path,
    ) {
        Ok(config) => config,
        Err(err) => {
            let _ = writeln!(stderr, "error: failed to load lint config: {err}");
            return 2;
        }
    };

    // `--config` bypasses discovering the project root's own
    // papyrus-lint.yaml/.yml entirely (see USAGE), so its
    // additional_script_roots is skipped too in that case; `--script-root`
    // still applies on top either way.
    let mut additional_script_roots = if config_path.is_some() {
        Vec::new()
    } else {
        match config::load_script_roots(&project_root) {
            Ok(roots) => roots,
            Err(err) => {
                let _ = writeln!(stderr, "error: failed to load lint config: {err}");
                return 2;
            }
        }
    };
    additional_script_roots.extend(cli_script_roots);

    // `strict_achlist_scope` (off by default, and skipped along with the
    // rest of the project root's config whenever `--config` is used) picks
    // between two ways of letting an achlist's entries resolve each other
    // across arbitrary, non-conventional source directories:
    //
    // - Off (the default): every listed entry's parent directory is added
    //   as a generic additional root, exactly as before this option
    //   existed. This is what an achlist-based project may already depend
    //   on — e.g. an unlisted sibling script in the same directory as a
    //   listed one still resolving — so normal usage sees no change at all.
    // - On: each listed script is instead registered directly by name (see
    //   `FunctionTable::with_known_scripts`), without treating its
    //   directory as a root. This never makes an unlisted file that happens
    //   to share a listed one's directory resolvable, and never requires
    //   scanning that directory at all, which matters a great deal on a
    //   large achlist whose entries are spread across many directories (see
    //   #311) — but it does mean a project relying on the off behavior
    //   above would see resolution/diagnostics change.
    let strict_achlist_scope = if config_path.is_some() {
        false
    } else {
        match config::load_strict_achlist_scope(&project_root) {
            Ok(value) => value,
            Err(err) => {
                let _ = writeln!(stderr, "error: failed to load lint config: {err}");
                return 2;
            }
        }
    };

    if !is_psc_file && !strict_achlist_scope {
        for script_path in &script_paths {
            let Some(parent) = script_path.parent() else {
                continue;
            };
            let root = parent
                .strip_prefix(&project_root)
                .unwrap_or(parent)
                .to_string_lossy()
                .into_owned();
            if !additional_script_roots.contains(&root) {
                additional_script_roots.push(root);
            }
        }
    }

    let mut function_table =
        FunctionTable::new_with_additional_roots(project_root, additional_script_roots);
    if strict_achlist_scope {
        function_table = function_table.with_known_scripts(&script_paths);
    }

    // Grouped by (lowercased) file name up front so checking for a
    // same-named achlist entry elsewhere in the list, below, stays
    // proportional to how many scripts actually share a name rather than to
    // the achlist's full size. Only needed in strict-scope mode: the
    // off-by-default directory-based `conflicting_script_versions` call
    // below already covers this (and more) via the directories just added
    // to `additional_script_roots`.
    let mut scripts_by_name: HashMap<String, Vec<PathBuf>> = HashMap::new();
    if strict_achlist_scope {
        for script_path in &script_paths {
            if let Some(name) = script_path.file_name().and_then(|name| name.to_str()) {
                scripts_by_name
                    .entry(name.to_ascii_lowercase())
                    .or_default()
                    .push(script_path.clone());
            }
        }
    }

    let mut total_diagnostics = 0usize;
    let mut files_with_diagnostics = 0usize;
    let mut files_fixed = 0usize;
    let mut should_fail = false;
    let mut json_files: Vec<JsonFileReport> = Vec::new();
    // Buffered so `--output <path>` can redirect the whole report to a file
    // instead of stdout, without duplicating the printing logic below.
    let mut report_buf: Vec<u8> = Vec::new();

    for script_path in &script_paths {
        let source = match read_psc_source(script_path) {
            Ok(source) => source,
            Err(err) => {
                let _ = writeln!(
                    stderr,
                    "error: failed to read {}: {err}",
                    script_path.display()
                );
                return 2;
            }
        };

        let source = if fix {
            let repaired = papyrus_lints::repair(&source, &lint_config);
            if repaired != source {
                if let Err(err) = fs::write(script_path, &repaired) {
                    let _ = writeln!(
                        stderr,
                        "error: failed to write {}: {err}",
                        script_path.display()
                    );
                    return 2;
                }
                files_fixed += 1;
            }
            repaired
        } else {
            source
        };

        let mut diagnostics =
            papyrus_lints::lint_with_external_arguments(&source, &lint_config, &mut function_table);
        if lint_config.rules.conflicting_script_versions {
            if strict_achlist_scope {
                // No directories were added to `additional_script_roots` in
                // this mode (see above), so `conflicting_script_versions`'s
                // own directory scan would find nothing among achlist
                // entries anyway; comparing the achlist's own listed
                // entries directly is what actually catches a same-named
                // collision here, without re-reporting one directory
                // scanning might otherwise also find (e.g. two entries
                // whose directories both also happen to be configured
                // `additional_script_roots`).
                if let Some(name) = script_path.file_name().and_then(|name| name.to_str()) {
                    if let Some(same_named) = scripts_by_name.get(&name.to_ascii_lowercase()) {
                        diagnostics.extend(
                            papyrus_lint_core::script_locator::conflicting_script_versions_among(
                                script_path,
                                same_named,
                            ),
                        );
                    }
                }
            } else {
                diagnostics.extend(
                    papyrus_lint_core::script_locator::conflicting_script_versions(
                        script_path,
                        function_table.root(),
                        function_table.additional_roots(),
                    ),
                );
            }
        }
        diagnostics.sort_by_key(|d| (d.line, d.column));

        // Quiet flags only affect presentation. A hidden diagnostic still
        // participates in the configured failure threshold and exit code.
        for diagnostic in &diagnostics {
            should_fail = should_fail || lint_config.should_fail_on(diagnostic);
        }
        diagnostics.retain(|diagnostic| {
            !((quiet_warnings && diagnostic.level() == "warning")
                || (quiet_info && diagnostic.level() == "info"))
        });

        for diagnostic in &diagnostics {
            if !json {
                let _ = writeln!(
                    report_buf,
                    "{}:{}:{}: [{}] {}",
                    script_path.display(),
                    diagnostic.line,
                    diagnostic.column,
                    diagnostic.rule,
                    diagnostic.message
                );
            }
        }

        if json {
            json_files.push(JsonFileReport {
                path: script_path.display().to_string(),
                diagnostics: diagnostics
                    .iter()
                    .map(|d| JsonDiagnostic {
                        line: d.line,
                        column: d.column,
                        rule: d.rule,
                        level: d.level(),
                        message: d.message.clone(),
                    })
                    .collect(),
            });
        }

        if !diagnostics.is_empty() {
            files_with_diagnostics += 1;
            total_diagnostics += diagnostics.len();
        }
    }

    let success = !should_fail;

    if json {
        let report = JsonReport {
            files: json_files,
            scripts_checked: script_paths.len(),
            files_with_diagnostics,
            total_diagnostics,
            files_fixed: fix.then_some(files_fixed),
            success,
        };
        let _ = writeln!(
            report_buf,
            "{}",
            serde_json::to_string_pretty(&report).unwrap_or_else(|_| "{}".to_string())
        );
    } else {
        let fixed_suffix = if fix {
            format!(" ({files_fixed} script(s) fixed.)")
        } else {
            String::new()
        };

        if total_diagnostics == 0 {
            let _ = writeln!(
                report_buf,
                "PapyrusLinterCLI: no problems found in {} script(s).{fixed_suffix}",
                script_paths.len()
            );
        } else {
            let _ = writeln!(
                report_buf,
                "PapyrusLinterCLI: {total_diagnostics} problem(s) found in {files_with_diagnostics} of {} script(s).{fixed_suffix}",
                script_paths.len()
            );
        }
    }

    if let Some(output_path) = output_path {
        if let Err(err) = fs::write(&output_path, &report_buf) {
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

    if success {
        0
    } else {
        1
    }
}

fn initialize_config(dir: &Path, stdout: &mut impl Write, stderr: &mut impl Write) -> u8 {
    match config::initialize_default_config(dir) {
        Ok(path) => {
            let _ = writeln!(stdout, "Created {}", path.display());
            0
        }
        Err(err) => {
            let _ = writeln!(stderr, "error: failed to initialize config: {err}");
            2
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn write_file(path: &Path, contents: &str) {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).expect("failed to create parent dir");
        }
        fs::write(path, contents).expect("failed to write file");
    }

    fn run_captured(args: &[String]) -> (u8, String, String) {
        let mut stdout = Vec::new();
        let mut stderr = Vec::new();
        let code = run(args, &mut stdout, &mut stderr);
        (
            code,
            String::from_utf8(stdout).expect("stdout should be utf8"),
            String::from_utf8(stderr).expect("stderr should be utf8"),
        )
    }

    #[test]
    fn candidate_pair_root_recognizes_every_supported_directory_order() {
        let root = Path::new("project");

        assert_eq!(
            find_candidate_pair_root(&root.join("scripts/source/Example.psc")),
            Some(root.to_path_buf())
        );
        assert_eq!(
            find_candidate_pair_root(&root.join("source/scripts/Example.psc")),
            Some(root.to_path_buf())
        );
    }

    #[test]
    fn candidate_pair_root_is_case_insensitive_and_supports_nested_scripts() {
        let script = Path::new("project/SCRIPTS/Source/User/Example.psc");

        assert_eq!(
            find_candidate_pair_root(script),
            Some(PathBuf::from("project"))
        );
    }

    #[test]
    fn psc_project_root_uses_the_legacy_fallback_without_a_candidate_pair() {
        assert_eq!(
            find_psc_project_root(Path::new("project/custom/source/Example.psc")),
            PathBuf::from("project")
        );
        assert_eq!(
            find_psc_project_root(Path::new("Example.psc")),
            PathBuf::from(".")
        );
    }

    #[test]
    fn prints_usage_and_exits_2_with_no_arguments() {
        let (code, _stdout, stderr) = run_captured(&[]);

        assert_eq!(code, 2);
        assert!(stderr.contains("Usage: PapyrusLinterCLI"));
    }

    #[test]
    fn prints_version_for_version_flag() {
        let (code, stdout, _stderr) = run_captured(&["--version".to_string()]);

        assert_eq!(code, 0);
        assert_eq!(stdout, format!("PapyrusLinterCLI {VERSION}\n"));
    }

    #[test]
    fn prints_version_for_short_version_flag() {
        let (code, stdout, _stderr) = run_captured(&["-V".to_string()]);

        assert_eq!(code, 0);
        assert_eq!(stdout, format!("PapyrusLinterCLI {VERSION}\n"));
    }

    #[test]
    fn prints_usage_for_help_flag() {
        let (code, _stdout, stderr) = run_captured(&["--help".to_string()]);

        assert_eq!(code, 2);
        assert!(stderr.contains("Usage: PapyrusLinterCLI"));
    }

    #[test]
    fn prints_usage_with_too_many_arguments() {
        let (code, _stdout, stderr) = run_captured(&["a".to_string(), "b".to_string()]);

        assert_eq!(code, 2);
        assert!(stderr.contains("Usage: PapyrusLinterCLI"));
    }

    #[test]
    fn init_creates_a_default_config() {
        let dir = tempfile::tempdir().expect("failed to create temp dir");
        let mut stdout = Vec::new();
        let mut stderr = Vec::new();

        let code = initialize_config(dir.path(), &mut stdout, &mut stderr);

        assert_eq!(code, 0);
        assert!(stderr.is_empty());
        assert!(String::from_utf8(stdout)
            .unwrap()
            .contains("papyrus-lint.yaml"));
        let config = config::load_config(dir.path()).expect("config should load");
        assert_eq!(config, papyrus_lints::Config::default());
    }

    #[test]
    fn init_refuses_to_overwrite_an_existing_config() {
        let dir = tempfile::tempdir().expect("failed to create temp dir");
        let path = dir.path().join("papyrus-lint.yml");
        write_file(&path, "semicolon: true\n");
        let mut stdout = Vec::new();
        let mut stderr = Vec::new();

        let code = initialize_config(dir.path(), &mut stdout, &mut stderr);

        assert_eq!(code, 2);
        assert!(stdout.is_empty());
        assert!(String::from_utf8(stderr)
            .unwrap()
            .contains("config already exists"));
        assert_eq!(fs::read_to_string(path).unwrap(), "semicolon: true\n");
    }

    #[test]
    fn errors_when_achlist_is_missing() {
        let dir = tempfile::tempdir().expect("failed to create temp dir");
        let achlist_path = dir.path().join("missing.achlist");

        let (code, _stdout, stderr) = run_captured(&[achlist_path.to_string_lossy().into_owned()]);

        assert_eq!(code, 2);
        assert!(stderr.starts_with("error:"));
    }

    #[test]
    fn reports_no_problems_for_a_clean_project() {
        let dir = tempfile::tempdir().expect("failed to create temp dir");
        write_file(
            &dir.path().join("scripts/source/Example.psc"),
            "ScriptName Example\n",
        );
        write_file(
            &dir.path().join("sources.achlist"),
            r#"["scripts/source/Example.psc"]"#,
        );
        let achlist_path = dir.path().join("sources.achlist");

        let (code, stdout, _stderr) = run_captured(&[achlist_path.to_string_lossy().into_owned()]);

        assert_eq!(code, 0);
        assert!(stdout.contains("no problems found in 1 script"));
    }

    #[test]
    fn reports_diagnostics_and_exits_1_for_a_dirty_project() {
        let dir = tempfile::tempdir().expect("failed to create temp dir");
        write_file(
            &dir.path().join("scripts/source/Example.psc"),
            "ScriptName Example\n\nFunction DoThing()\n\tGame.GetPlayer()\nEndFunction\n",
        );
        write_file(
            &dir.path().join("sources.achlist"),
            r#"["scripts/source/Example.psc"]"#,
        );
        let achlist_path = dir.path().join("sources.achlist");

        let (code, stdout, _stderr) = run_captured(&[achlist_path.to_string_lossy().into_owned()]);

        assert_eq!(code, 1);
        assert!(stdout.contains("[forbidden-functions]"));
        assert!(stdout.contains("problem(s) found in 1 of 1 script(s)"));
    }

    #[test]
    fn does_not_fail_on_warning_level_diagnostics_by_default() {
        let dir = tempfile::tempdir().expect("failed to create temp dir");
        write_file(
            &dir.path().join("scripts/source/Example.psc"),
            "ScriptName Example\n\nInt Property MyValue = 1 Auto\n\nFunction DoThing()\nEndFunction\n",
        );
        write_file(
            &dir.path().join("sources.achlist"),
            r#"["scripts/source/Example.psc"]"#,
        );
        let achlist_path = dir.path().join("sources.achlist");

        let (code, stdout, _stderr) = run_captured(&[achlist_path.to_string_lossy().into_owned()]);

        assert_eq!(code, 0);
        assert!(stdout.contains("[unused-property]"));
        assert!(stdout.contains("1 problem(s) found in 1 of 1 script(s)"));
    }

    #[test]
    fn fails_on_warning_level_diagnostics_when_configured() {
        let dir = tempfile::tempdir().expect("failed to create temp dir");
        write_file(
            &dir.path().join("scripts/source/Example.psc"),
            "ScriptName Example\n\nInt Property MyValue = 1 Auto\n\nFunction DoThing()\nEndFunction\n",
        );
        write_file(
            &dir.path().join("sources.achlist"),
            r#"["scripts/source/Example.psc"]"#,
        );
        write_file(
            &dir.path().join("papyrus-lint.yaml"),
            "fail_on_warning: true\n",
        );
        let achlist_path = dir.path().join("sources.achlist");

        let (code, stdout, _stderr) = run_captured(&[achlist_path.to_string_lossy().into_owned()]);

        assert_eq!(code, 1);
        assert!(stdout.contains("[unused-property]"));
    }

    #[test]
    fn quiet_warnings_hides_warnings_without_changing_the_exit_code() {
        let dir = tempfile::tempdir().expect("failed to create temp dir");
        let script_path = dir.path().join("scripts/source/Example.psc");
        write_file(&script_path, "ScriptName Example   \n");
        write_file(
            &dir.path().join("papyrus-lint.yaml"),
            "fail_on_warning: true\n",
        );

        let (code, stdout, stderr) = run_captured(&[
            "--quiet-warnings".to_string(),
            script_path.to_string_lossy().into_owned(),
        ]);

        assert_eq!(code, 1);
        assert!(stderr.is_empty());
        assert!(!stdout.contains("[warning]"));
        assert!(stdout.contains("no problems found in 1 script"));
    }

    #[test]
    fn quiet_info_hides_info_diagnostics_from_json_without_changing_the_exit_code() {
        let dir = tempfile::tempdir().expect("failed to create temp dir");
        let script_path = dir.path().join("scripts/source/Example.psc");
        write_file(
            &script_path,
            "ScriptName Example\n\nGlobalVariable Property Value Auto\n\nFunction Test()\n    Value.GetValueInt()\nEndFunction\n",
        );
        write_file(
            &dir.path().join("papyrus-lint.yaml"),
            "fail_on_info: true\n",
        );

        let (unfiltered_code, unfiltered_stdout, _) = run_captured(&[
            "--json".to_string(),
            script_path.to_string_lossy().into_owned(),
        ]);
        let unfiltered: serde_json::Value = serde_json::from_str(&unfiltered_stdout).unwrap();
        assert_eq!(unfiltered_code, 1);
        assert!(unfiltered["files"][0]["diagnostics"]
            .as_array()
            .unwrap()
            .iter()
            .any(|diagnostic| diagnostic["level"] == "info"));

        let (code, stdout, stderr) = run_captured(&[
            "--json".to_string(),
            "--quiet-info".to_string(),
            script_path.to_string_lossy().into_owned(),
        ]);

        assert_eq!(code, 1);
        assert!(stderr.is_empty());
        let report: serde_json::Value = serde_json::from_str(&stdout).unwrap();
        assert_eq!(report["success"], false);
        let diagnostics = report["files"][0]["diagnostics"].as_array().unwrap();
        assert_eq!(report["total_diagnostics"], diagnostics.len());
        assert!(diagnostics
            .iter()
            .all(|diagnostic| diagnostic["level"] != "info"));
    }

    #[test]
    fn honors_the_project_yaml_config() {
        let dir = tempfile::tempdir().expect("failed to create temp dir");
        write_file(
            &dir.path().join("scripts/source/Example.psc"),
            "ScriptName Example   \n",
        );
        write_file(
            &dir.path().join("sources.achlist"),
            r#"["scripts/source/Example.psc"]"#,
        );
        write_file(
            &dir.path().join("papyrus-lint.yaml"),
            "rules:\n  trailing_whitespace: false\n",
        );
        let achlist_path = dir.path().join("sources.achlist");

        let (code, stdout, _stderr) = run_captured(&[achlist_path.to_string_lossy().into_owned()]);

        assert_eq!(code, 0);
        assert!(stdout.contains("no problems found"));
    }

    #[test]
    fn reports_an_invalid_project_yaml_config() {
        let dir = tempfile::tempdir().expect("failed to create temp dir");
        write_file(
            &dir.path().join("scripts/source/Example.psc"),
            "ScriptName Example\n",
        );
        write_file(
            &dir.path().join("sources.achlist"),
            r#"["scripts/source/Example.psc"]"#,
        );
        write_file(
            &dir.path().join("papyrus-lint.yaml"),
            "rules: definitely-not-a-rule-set\n",
        );
        let achlist_path = dir.path().join("sources.achlist");

        let (code, stdout, stderr) = run_captured(&[achlist_path.to_string_lossy().into_owned()]);

        assert_eq!(code, 2);
        assert!(stdout.is_empty());
        assert!(stderr.starts_with("error: failed to load lint config:"));
    }

    #[test]
    fn ignores_non_psc_entries_in_the_achlist() {
        let dir = tempfile::tempdir().expect("failed to create temp dir");
        write_file(&dir.path().join("scripts/source/Example.pex"), "");
        write_file(
            &dir.path().join("sources.achlist"),
            r#"["scripts/source/Example.pex"]"#,
        );
        let achlist_path = dir.path().join("sources.achlist");

        let (code, stdout, _stderr) = run_captured(&[achlist_path.to_string_lossy().into_owned()]);

        assert_eq!(code, 0);
        assert!(stdout.contains("no problems found in 0 script"));
    }

    #[test]
    fn recognizes_uppercase_psc_extensions_in_the_achlist() {
        let dir = tempfile::tempdir().expect("failed to create temp dir");
        write_file(
            &dir.path().join("scripts/source/Example.PSC"),
            "ScriptName Example   \n",
        );
        write_file(
            &dir.path().join("sources.achlist"),
            r#"["scripts/source/Example.PSC"]"#,
        );
        let achlist_path = dir.path().join("sources.achlist");

        let (code, stdout, stderr) = run_captured(&[achlist_path.to_string_lossy().into_owned()]);

        assert_eq!(code, 0);
        assert!(stderr.is_empty());
        assert!(stdout.contains("[trailing-whitespace]"));
        assert!(stdout.contains("1 problem(s) found in 1 of 1 script(s)"));
    }

    #[test]
    fn lints_a_single_psc_file_passed_directly() {
        let dir = tempfile::tempdir().expect("failed to create temp dir");
        let script_path = dir.path().join("Example.psc");
        write_file(&script_path, "ScriptName Example   \n");

        let (code, stdout, _stderr) = run_captured(&[script_path.to_string_lossy().into_owned()]);

        assert_eq!(code, 0);
        assert!(stdout.contains("[trailing-whitespace]"));
        assert!(stdout.contains("1 problem(s) found in 1 of 1 script(s)"));
    }

    #[test]
    fn reports_no_problems_for_a_clean_single_psc_file() {
        let dir = tempfile::tempdir().expect("failed to create temp dir");
        let script_path = dir.path().join("Example.psc");
        write_file(&script_path, "ScriptName Example\n");

        let (code, stdout, _stderr) = run_captured(&[script_path.to_string_lossy().into_owned()]);

        assert_eq!(code, 0);
        assert!(stdout.contains("no problems found in 1 script"));
    }

    #[test]
    fn honors_the_project_yaml_config_two_directories_above_a_single_psc_file() {
        // Mirrors the real layout a bare .psc file is found at (e.g. an
        // editor plugin invoking the CLI on a saved file), where the
        // project root sits two directories above the script, at
        // `<root>/scripts/source/Example.psc`.
        let dir = tempfile::tempdir().expect("failed to create temp dir");
        let script_path = dir.path().join("scripts/source/Example.psc");
        write_file(&script_path, "ScriptName Example   \n");
        write_file(
            &dir.path().join("papyrus-lint.yaml"),
            "rules:\n  trailing_whitespace: false\n",
        );

        let (code, stdout, _stderr) = run_captured(&[script_path.to_string_lossy().into_owned()]);

        assert_eq!(code, 0);
        assert!(stdout.contains("no problems found"));
    }

    #[test]
    fn finds_the_project_root_for_a_psc_nested_under_a_namespaced_subfolder() {
        // A Fallout 4-style namespaced script, e.g. `ScriptName User:MyScript`
        // stored at `Scripts/Source/User/MyScript.psc`, sits three
        // directories under the project root rather than the conventional
        // two. A naive "two directories up" rule would land on `Scripts`
        // instead of the real root, missing the project's config and
        // breaking every cross-script lookup — this must still find the
        // real root and pick up the config there.
        let dir = tempfile::tempdir().expect("failed to create temp dir");
        let script_path = dir.path().join("scripts/source/User/MyScript.psc");
        write_file(&script_path, "ScriptName User:MyScript   \n");
        write_file(
            &dir.path().join("papyrus-lint.yaml"),
            "rules:\n  trailing_whitespace: false\n",
        );

        let (code, stdout, _stderr) = run_captured(&[script_path.to_string_lossy().into_owned()]);

        assert_eq!(code, 0);
        assert!(stdout.contains("no problems found"));
    }

    #[test]
    fn finds_the_project_root_from_script_position_when_the_achlist_lives_elsewhere() {
        // Users sometimes drop the .achlist somewhere other than the
        // project root (e.g. next to a game's Data directory) while the
        // actual project, including its papyrus-lint.yaml, lives deeper:
        //
        //   achlist
        //   somefolder/
        //     otherfolder/
        //       papyrus-lint.yaml
        //       scripts/source/AType.psc
        //       source/scripts/BType.psc
        //
        // The achlist's own parent directory (the top-level one) has no
        // config file at all, so the project root must instead be found
        // from the resolved scripts' own position under their
        // scripts/source or source/scripts pair.
        let dir = tempfile::tempdir().expect("failed to create temp dir");
        let project_root = dir.path().join("somefolder/otherfolder");
        write_file(
            &project_root.join("scripts/source/AType.psc"),
            "ScriptName AType   \n",
        );
        write_file(
            &project_root.join("source/scripts/BType.psc"),
            "ScriptName BType   \n",
        );
        write_file(
            &project_root.join("papyrus-lint.yaml"),
            "rules:\n  trailing_whitespace: false\n",
        );
        write_file(
            &dir.path().join("achlist"),
            r#"["somefolder/otherfolder/scripts/source/AType.psc", "somefolder/otherfolder/source/scripts/BType.psc"]"#,
        );
        let achlist_path = dir.path().join("achlist");

        let (code, stdout, _stderr) = run_captured(&[achlist_path.to_string_lossy().into_owned()]);

        // If the project root were (wrongly) taken as the achlist's own
        // parent directory, papyrus-lint.yaml wouldn't be found and the
        // trailing-whitespace lint (disabled by that config) would fire on
        // both scripts instead.
        assert_eq!(code, 0);
        assert!(stdout.contains("no problems found in 2 script"));
    }

    #[test]
    fn same_script_lints_identically_via_achlist_and_directly_when_namespaced() {
        // The same nested script should produce the same diagnostics
        // whether it's resolved from an .achlist or linted directly.
        let dir = tempfile::tempdir().expect("failed to create temp dir");
        write_file(
            &dir.path().join("scripts/source/Greeter.psc"),
            "ScriptName Greeter\n\nFunction Greet(String name)\nEndFunction\n",
        );
        let script_path = dir.path().join("scripts/source/User/Example.psc");
        write_file(
            &script_path,
            "ScriptName Example\n\nGreeter Property Target Auto\n\nFunction Test()\n    Target.Greet(1)\nEndFunction\n",
        );
        write_file(
            &dir.path().join("sources.achlist"),
            r#"["scripts/source/Greeter.psc", "scripts/source/User/Example.psc"]"#,
        );
        let achlist_path = dir.path().join("sources.achlist");

        let (achlist_code, achlist_stdout, _) =
            run_captured(&[achlist_path.to_string_lossy().into_owned()]);
        let (direct_code, direct_stdout, _) =
            run_captured(&[script_path.to_string_lossy().into_owned()]);

        assert_eq!(achlist_code, 1);
        assert_eq!(direct_code, 1);
        assert!(achlist_stdout.contains("[argument-types]"));
        assert!(direct_stdout.contains("[argument-types]"));
    }

    #[test]
    fn decodes_a_cp1252_encoded_script_instead_of_failing_the_whole_run() {
        let dir = tempfile::tempdir().expect("failed to create temp dir");
        let script_path = dir.path().join("scripts/source/Example.psc");
        fs::create_dir_all(script_path.parent().unwrap()).expect("failed to create parent dir");
        // "; caf\xE9" in Windows-1252 (0xE9 is "é"), which is not valid
        // UTF-8 on its own.
        let mut contents = b"ScriptName Example\n\n; caf".to_vec();
        contents.push(0xE9);
        contents.push(b'\n');
        fs::write(&script_path, &contents).expect("failed to write test file");
        write_file(
            &dir.path().join("sources.achlist"),
            r#"["scripts/source/Example.psc"]"#,
        );
        let achlist_path = dir.path().join("sources.achlist");

        let (code, stdout, stderr) = run_captured(&[achlist_path.to_string_lossy().into_owned()]);

        assert_eq!(code, 0, "stderr: {stderr}");
        assert!(stdout.contains("no problems found in 1 script"));
    }

    #[test]
    fn errors_when_the_given_psc_file_is_missing() {
        let dir = tempfile::tempdir().expect("failed to create temp dir");
        let script_path = dir.path().join("Missing.psc");

        let (code, _stdout, stderr) = run_captured(&[script_path.to_string_lossy().into_owned()]);

        assert_eq!(code, 2);
        assert!(stderr.starts_with("error:"));
    }

    #[test]
    fn fix_rewrites_fixable_issues_and_reports_the_rest() {
        let dir = tempfile::tempdir().expect("failed to create temp dir");
        write_file(
            &dir.path().join("scripts/source/Example.psc"),
            "ScriptName Example   \n\nFunction DoThing()\n\tGame.GetPlayer()\nEndFunction\n",
        );
        write_file(
            &dir.path().join("sources.achlist"),
            r#"["scripts/source/Example.psc"]"#,
        );
        let achlist_path = dir.path().join("sources.achlist");

        let (code, stdout, _stderr) = run_captured(&[
            "fix".to_string(),
            achlist_path.to_string_lossy().into_owned(),
        ]);

        assert_eq!(
            fs::read_to_string(dir.path().join("scripts/source/Example.psc")).unwrap(),
            "ScriptName Example\n\nFunction DoThing()\n\tGame.GetPlayer()\nEndFunction\n"
        );
        assert_eq!(code, 1);
        assert!(!stdout.contains("[trailing-whitespace]"));
        assert!(stdout.contains("Game.GetPlayer"));
        assert!(stdout.contains("(1 script(s) fixed.)"));
    }

    #[test]
    fn fix_does_not_rewrite_an_already_clean_file() {
        let dir = tempfile::tempdir().expect("failed to create temp dir");
        let script_path = dir.path().join("Example.psc");
        write_file(&script_path, "ScriptName Example\n");

        let (code, stdout, _stderr) = run_captured(&[
            "fix".to_string(),
            script_path.to_string_lossy().into_owned(),
        ]);

        assert_eq!(code, 0);
        assert_eq!(
            fs::read_to_string(&script_path).unwrap(),
            "ScriptName Example\n"
        );
        assert!(stdout.contains("(0 script(s) fixed.)"));
    }

    #[test]
    fn prints_usage_when_fix_is_given_without_a_path() {
        let (code, _stdout, stderr) = run_captured(&["fix".to_string()]);

        assert_eq!(code, 2);
        assert!(stderr.contains("Usage: PapyrusLinterCLI"));
    }

    #[test]
    fn fix_errors_when_the_achlist_is_missing() {
        let dir = tempfile::tempdir().expect("failed to create temp dir");
        let achlist_path = dir.path().join("missing.achlist");

        let (code, _stdout, stderr) = run_captured(&[
            "fix".to_string(),
            achlist_path.to_string_lossy().into_owned(),
        ]);

        assert_eq!(code, 2);
        assert!(stderr.starts_with("error:"));
    }

    #[test]
    fn resolves_cross_script_argument_types_from_the_project_root() {
        let dir = tempfile::tempdir().expect("failed to create temp dir");
        write_file(
            &dir.path().join("scripts/source/Greeter.psc"),
            "ScriptName Greeter\n\nFunction Greet(String name)\nEndFunction\n",
        );
        write_file(
            &dir.path().join("scripts/source/Example.psc"),
            "ScriptName Example\n\nGreeter Property Target Auto\n\nFunction Test()\n    Target.Greet(1)\nEndFunction\n",
        );
        write_file(
            &dir.path().join("sources.achlist"),
            r#"["scripts/source/Greeter.psc", "scripts/source/Example.psc"]"#,
        );
        let achlist_path = dir.path().join("sources.achlist");

        let (code, stdout, _stderr) = run_captured(&[achlist_path.to_string_lossy().into_owned()]);

        assert_eq!(code, 1);
        assert!(stdout.contains("[argument-types]"));
    }

    #[test]
    fn resolves_cross_script_types_from_every_directory_listed_in_the_achlist() {
        let dir = tempfile::tempdir().expect("failed to create temp dir");
        write_file(
            &dir.path().join("source/dir/one/Greeter.psc"),
            "ScriptName Greeter\n\nFunction Greet(String name)\nEndFunction\n",
        );
        write_file(
            &dir.path().join("source/dir/two/Example.psc"),
            "ScriptName Example\n\nGreeter Property Target Auto\n\nFunction Test()\n    Target.Greet(1)\nEndFunction\n",
        );
        write_file(
            &dir.path().join("sources.achlist"),
            r#"["source/dir/one/Greeter.psc", "source/dir/two/Example.psc"]"#,
        );

        let (code, stdout, stderr) = run_captured(&[dir
            .path()
            .join("sources.achlist")
            .to_string_lossy()
            .into_owned()]);

        assert_eq!(code, 1, "stderr: {stderr}");
        assert!(stdout.contains("[argument-types]"));
    }

    #[test]
    fn goto_state_resolves_a_state_declared_on_a_parent_script_listed_in_the_achlist() {
        // Regression test for https://github.com/Idrinth/papyrus-lint/issues/259:
        // a state declared only on a script's Extends ancestor must not be
        // flagged as missing, even when (as here) the achlist's entries
        // don't sit under either conventional scripts/source layout.
        let dir = tempfile::tempdir().expect("failed to create temp dir");
        write_file(
            &dir.path().join("VahlokStateBase.psc"),
            "Scriptname VahlokStateBase extends ObjectReference\n\nauto State Idle\nEndState\n\nState Busy\nEndState\n",
        );
        write_file(
            &dir.path().join("VahlokStateChild.psc"),
            "Scriptname VahlokStateChild extends VahlokStateBase\n\nState Extra\nEndState\n\nFunction Demo()\n    GoToState(\"Extra\")\n    GoToState(\"Idle\")\n    GoToState(\"Busy\")\n    GoToState(\"NoSuchState\")\nEndFunction\n",
        );
        write_file(
            &dir.path().join("scripts.achlist"),
            r#"["VahlokStateBase.psc", "VahlokStateChild.psc"]"#,
        );

        let (code, stdout, stderr) = run_captured(&[dir
            .path()
            .join("scripts.achlist")
            .to_string_lossy()
            .into_owned()]);

        assert_eq!(code, 0, "stderr: {stderr}, stdout: {stdout}");
        assert!(!stdout.contains("'Extra'"));
        assert!(!stdout.contains("'Idle'"));
        assert!(!stdout.contains("'Busy'"));
        assert!(stdout.contains("[goto-state]"));
        assert!(stdout.contains("'NoSuchState'"));
    }

    #[test]
    fn achlist_resolves_an_unlisted_sibling_script_by_default_for_backward_compatibility() {
        // `strict_achlist_scope` defaults to false, so an achlist-based
        // project already depending on the pre-#311-fix behavior (every
        // listed entry's directory acting as a generic search root) must
        // see no change: `Unlisted.psc` sits right beside the listed
        // `Example.psc`, is never itself mentioned in the achlist, and
        // still resolves.
        let dir = tempfile::tempdir().expect("failed to create temp dir");
        write_file(
            &dir.path().join("mods/one/Example.psc"),
            "ScriptName Example\n\nFunction Test()\n    Unlisted.DoThing()\nEndFunction\n",
        );
        write_file(
            &dir.path().join("mods/one/Unlisted.psc"),
            "ScriptName Unlisted\n\nFunction DoThing()\nEndFunction\n",
        );
        write_file(
            &dir.path().join("scripts.achlist"),
            r#"["mods/one/Example.psc"]"#,
        );

        let (code, stdout, stderr) = run_captured(&[dir
            .path()
            .join("scripts.achlist")
            .to_string_lossy()
            .into_owned()]);

        assert_eq!(code, 0, "stderr: {stderr}");
        assert!(!stdout.contains("[unresolved-script]"), "stdout: {stdout}");
    }

    #[test]
    fn strict_achlist_scope_does_not_leak_into_an_unlisted_sibling_script() {
        // Regression test for https://github.com/Idrinth/papyrus-lint/issues/311:
        // with `strict_achlist_scope: true`, an achlist entry's directory
        // must not become a generic search root, since that would silently
        // make every *other* file in that directory resolvable too, even
        // though it was never listed. Here `Unlisted.psc` sits right beside
        // the listed `Example.psc` but is itself never mentioned in the
        // achlist, so a call against its type must be reported as
        // unresolved once strict scoping is turned on.
        let dir = tempfile::tempdir().expect("failed to create temp dir");
        write_file(
            &dir.path().join("mods/one/Example.psc"),
            "ScriptName Example\n\nFunction Test()\n    Unlisted.DoThing()\nEndFunction\n",
        );
        write_file(
            &dir.path().join("mods/one/Unlisted.psc"),
            "ScriptName Unlisted\n\nFunction DoThing()\nEndFunction\n",
        );
        write_file(
            &dir.path().join("scripts.achlist"),
            r#"["mods/one/Example.psc"]"#,
        );
        write_file(
            &dir.path().join("papyrus-lint.yaml"),
            "strict_achlist_scope: true\n",
        );

        let (code, stdout, stderr) = run_captured(&[dir
            .path()
            .join("scripts.achlist")
            .to_string_lossy()
            .into_owned()]);

        assert_eq!(code, 0, "stderr: {stderr}");
        assert!(stdout.contains("[unresolved-script]"), "stdout: {stdout}");
        assert!(stdout.contains("'Unlisted'"), "stdout: {stdout}");
    }

    #[test]
    fn strict_achlist_scope_still_flags_conflicting_versions_between_two_achlist_entries_sharing_a_file_name(
    ) {
        // Regression test for https://github.com/Idrinth/papyrus-lint/issues/311:
        // with `strict_achlist_scope: true`, two achlist entries can share a
        // file name while living in directories that are no longer scanned
        // as search roots for one another (see the previous test), so the
        // conflicting-script-versions check has to compare the achlist's
        // own listed entries directly rather than relying on a directory
        // scan to notice the collision.
        let dir = tempfile::tempdir().expect("failed to create temp dir");
        write_file(
            &dir.path().join("mods/one/Example.psc"),
            "ScriptName Example\n",
        );
        write_file(
            &dir.path().join("mods/two/Example.psc"),
            "ScriptName ExampleV2\n",
        );
        write_file(
            &dir.path().join("scripts.achlist"),
            r#"["mods/one/Example.psc", "mods/two/Example.psc"]"#,
        );
        write_file(
            &dir.path().join("papyrus-lint.yaml"),
            "strict_achlist_scope: true\n",
        );

        let (code, stdout, stderr) = run_captured(&[dir
            .path()
            .join("scripts.achlist")
            .to_string_lossy()
            .into_owned()]);

        assert_eq!(code, 0, "stderr: {stderr}");
        assert_eq!(
            stdout.matches("[conflicting-script-versions]").count(),
            2,
            "stdout: {stdout}"
        );
    }

    #[test]
    fn strict_achlist_scope_does_not_double_report_a_conflict_also_visible_via_a_conventional_directory(
    ) {
        // Regression test: when two conflicting achlist entries also happen
        // to sit under the project's conventional scripts/source and
        // source/scripts directories, strict mode must report the
        // collision once per file (via conflicting_script_versions_among),
        // not twice (once more via the directory-based
        // conflicting_script_versions, which strict mode must skip
        // entirely to avoid duplicating what it already reports).
        let dir = tempfile::tempdir().expect("failed to create temp dir");
        write_file(
            &dir.path().join("scripts/source/Example.psc"),
            "ScriptName Example\n",
        );
        write_file(
            &dir.path().join("source/scripts/Example.psc"),
            "ScriptName ExampleV2\n",
        );
        write_file(
            &dir.path().join("scripts.achlist"),
            r#"["scripts/source/Example.psc", "source/scripts/Example.psc"]"#,
        );
        write_file(
            &dir.path().join("papyrus-lint.yaml"),
            "strict_achlist_scope: true\n",
        );

        let (code, stdout, stderr) = run_captured(&[dir
            .path()
            .join("scripts.achlist")
            .to_string_lossy()
            .into_owned()]);

        assert_eq!(code, 0, "stderr: {stderr}");
        assert_eq!(
            stdout.matches("[conflicting-script-versions]").count(),
            2,
            "stdout: {stdout}"
        );
    }

    #[test]
    fn resolves_cross_script_argument_types_from_the_projects_configured_script_root() {
        let dir = tempfile::tempdir().expect("failed to create temp dir");
        let shared_dir = tempfile::tempdir().expect("failed to create temp dir");
        write_file(
            &shared_dir.path().join("Greeter.psc"),
            "ScriptName Greeter\n\nFunction Greet(String name)\nEndFunction\n",
        );
        write_file(
            &dir.path().join("scripts/source/Example.psc"),
            "ScriptName Example\n\nGreeter Property Target Auto\n\nFunction Test()\n    Target.Greet(1)\nEndFunction\n",
        );
        write_file(
            &dir.path().join("sources.achlist"),
            r#"["scripts/source/Example.psc"]"#,
        );
        write_file(
            &dir.path().join("papyrus-lint.yaml"),
            &format!(
                "additional_script_roots:\n  - {}\n",
                shared_dir.path().display()
            ),
        );
        let achlist_path = dir.path().join("sources.achlist");

        let (code, stdout, _stderr) = run_captured(&[achlist_path.to_string_lossy().into_owned()]);

        assert_eq!(code, 1);
        assert!(stdout.contains("[argument-types]"));
    }

    #[test]
    fn resolves_cross_script_argument_types_from_a_script_root_flag() {
        let dir = tempfile::tempdir().expect("failed to create temp dir");
        let shared_dir = tempfile::tempdir().expect("failed to create temp dir");
        write_file(
            &shared_dir.path().join("Greeter.psc"),
            "ScriptName Greeter\n\nFunction Greet(String name)\nEndFunction\n",
        );
        write_file(
            &dir.path().join("scripts/source/Example.psc"),
            "ScriptName Example\n\nGreeter Property Target Auto\n\nFunction Test()\n    Target.Greet(1)\nEndFunction\n",
        );
        write_file(
            &dir.path().join("sources.achlist"),
            r#"["scripts/source/Example.psc"]"#,
        );
        let achlist_path = dir.path().join("sources.achlist");

        let (code, stdout, _stderr) = run_captured(&[
            "--script-root".to_string(),
            shared_dir.path().to_string_lossy().into_owned(),
            achlist_path.to_string_lossy().into_owned(),
        ]);

        assert_eq!(code, 1);
        assert!(stdout.contains("[argument-types]"));
    }

    #[test]
    fn script_root_flag_without_a_value_prints_usage() {
        let (code, _stdout, stderr) = run_captured(&["--script-root".to_string()]);

        assert_eq!(code, 2);
        assert!(stderr.contains("Usage: PapyrusLinterCLI"));
    }

    #[test]
    fn config_flag_skips_the_project_roots_additional_script_roots() {
        let dir = tempfile::tempdir().expect("failed to create temp dir");
        let shared_dir = tempfile::tempdir().expect("failed to create temp dir");
        write_file(
            &shared_dir.path().join("Greeter.psc"),
            "ScriptName Greeter\n\nFunction Greet(String name)\nEndFunction\n",
        );
        write_file(
            &dir.path().join("scripts/source/Example.psc"),
            "ScriptName Example\n\nGreeter Property Target Auto\n\nFunction Test()\n    Target.Greet(1)\nEndFunction\n",
        );
        write_file(
            &dir.path().join("sources.achlist"),
            r#"["scripts/source/Example.psc"]"#,
        );
        write_file(
            &dir.path().join("papyrus-lint.yaml"),
            &format!(
                "additional_script_roots:\n  - {}\n",
                shared_dir.path().display()
            ),
        );
        let override_path = dir.path().join("overrides/custom.yaml");
        write_file(&override_path, "");
        let achlist_path = dir.path().join("sources.achlist");

        let (code, stdout, _stderr) = run_captured(&[
            "--config".to_string(),
            override_path.to_string_lossy().into_owned(),
            achlist_path.to_string_lossy().into_owned(),
        ]);

        // Greeter can't be resolved (the project root's own config, which
        // declares the shared_dir root, is bypassed by --config), so the
        // "Argument type check" lint has nothing to flag.
        assert_eq!(code, 0);
        assert!(!stdout.contains("[argument-types]"));
    }

    #[test]
    fn json_flag_prints_a_single_json_report_instead_of_plain_text() {
        let dir = tempfile::tempdir().expect("failed to create temp dir");
        write_file(
            &dir.path().join("scripts/source/Example.psc"),
            "ScriptName Example   \n",
        );
        write_file(
            &dir.path().join("sources.achlist"),
            r#"["scripts/source/Example.psc"]"#,
        );
        let achlist_path = dir.path().join("sources.achlist");

        let (code, stdout, stderr) = run_captured(&[
            "--json".to_string(),
            achlist_path.to_string_lossy().into_owned(),
        ]);

        assert_eq!(code, 0);
        assert_eq!(stderr, "");
        assert!(!stdout.contains("PapyrusLinterCLI:"));

        let report: serde_json::Value =
            serde_json::from_str(&stdout).expect("stdout should be a single JSON document");
        assert_eq!(report["success"], true);
        assert_eq!(report["scripts_checked"], 1);
        assert_eq!(report["files_with_diagnostics"], 1);
        assert_eq!(report["total_diagnostics"], 1);
        assert!(report["files_fixed"].is_null());
        let files = report["files"]
            .as_array()
            .expect("files should be an array");
        assert_eq!(files.len(), 1);
        let diagnostics = files[0]["diagnostics"]
            .as_array()
            .expect("diagnostics should be an array");
        assert_eq!(diagnostics.len(), 1);
        assert_eq!(diagnostics[0]["rule"], "trailing-whitespace");
        assert_eq!(diagnostics[0]["level"], "warning");
        assert_eq!(diagnostics[0]["line"], 1);
    }

    #[test]
    fn json_flag_lists_every_resolved_script_including_clean_ones() {
        let dir = tempfile::tempdir().expect("failed to create temp dir");
        write_file(
            &dir.path().join("scripts/source/Example.psc"),
            "ScriptName Example\n",
        );
        write_file(
            &dir.path().join("sources.achlist"),
            r#"["scripts/source/Example.psc"]"#,
        );
        let achlist_path = dir.path().join("sources.achlist");

        let (code, stdout, _stderr) = run_captured(&[
            "--json".to_string(),
            achlist_path.to_string_lossy().into_owned(),
        ]);

        assert_eq!(code, 0);
        let report: serde_json::Value = serde_json::from_str(&stdout).unwrap();
        assert_eq!(report["success"], true);
        let files = report["files"].as_array().unwrap();
        assert_eq!(files.len(), 1);
        assert_eq!(files[0]["diagnostics"].as_array().unwrap().len(), 0);
    }

    #[test]
    fn json_flag_combines_with_the_fix_subcommand() {
        let dir = tempfile::tempdir().expect("failed to create temp dir");
        write_file(
            &dir.path().join("scripts/source/Example.psc"),
            "ScriptName Example   \n\nFunction DoThing()\n\tGame.GetPlayer()\nEndFunction\n",
        );
        write_file(
            &dir.path().join("sources.achlist"),
            r#"["scripts/source/Example.psc"]"#,
        );
        let achlist_path = dir.path().join("sources.achlist");

        let (code, stdout, _stderr) = run_captured(&[
            "fix".to_string(),
            "--json".to_string(),
            achlist_path.to_string_lossy().into_owned(),
        ]);

        assert_eq!(code, 1);
        assert_eq!(
            fs::read_to_string(dir.path().join("scripts/source/Example.psc")).unwrap(),
            "ScriptName Example\n\nFunction DoThing()\n\tGame.GetPlayer()\nEndFunction\n"
        );
        let report: serde_json::Value = serde_json::from_str(&stdout).unwrap();
        assert_eq!(report["files_fixed"], 1);
        let files = report["files"].as_array().unwrap();
        let diagnostics = files[0]["diagnostics"].as_array().unwrap();
        assert!(diagnostics
            .iter()
            .any(|d| d["message"].as_str().unwrap().contains("Game.GetPlayer")));
    }

    #[test]
    fn config_flag_overrides_project_root_discovery() {
        let dir = tempfile::tempdir().expect("failed to create temp dir");
        write_file(
            &dir.path().join("scripts/source/Example.psc"),
            "ScriptName Example   \n",
        );
        write_file(
            &dir.path().join("sources.achlist"),
            r#"["scripts/source/Example.psc"]"#,
        );
        // A project-root config that would otherwise apply, and an
        // unrelated override file elsewhere that should win instead.
        write_file(
            &dir.path().join("papyrus-lint.yaml"),
            "rules:\n  trailing_whitespace: true\n",
        );
        let override_path = dir.path().join("overrides/custom.yaml");
        write_file(&override_path, "rules:\n  trailing_whitespace: false\n");
        let achlist_path = dir.path().join("sources.achlist");

        let (code, stdout, _stderr) = run_captured(&[
            "--config".to_string(),
            override_path.to_string_lossy().into_owned(),
            achlist_path.to_string_lossy().into_owned(),
        ]);

        assert_eq!(code, 0);
        assert!(stdout.contains("no problems found"));
    }

    #[test]
    fn config_flag_combines_with_fix_and_json_in_any_order() {
        let dir = tempfile::tempdir().expect("failed to create temp dir");
        write_file(
            &dir.path().join("scripts/source/Example.psc"),
            "ScriptName Example   \n",
        );
        write_file(
            &dir.path().join("sources.achlist"),
            r#"["scripts/source/Example.psc"]"#,
        );
        let override_path = dir.path().join("overrides/custom.yaml");
        write_file(&override_path, "rules:\n  trailing_whitespace: false\n");
        let achlist_path = dir.path().join("sources.achlist");

        let (code, stdout, _stderr) = run_captured(&[
            "--json".to_string(),
            "fix".to_string(),
            "--config".to_string(),
            override_path.to_string_lossy().into_owned(),
            achlist_path.to_string_lossy().into_owned(),
        ]);

        assert_eq!(code, 0);
        // trailing_whitespace was disabled by the overriding config, so
        // fix should leave the trailing whitespace in place untouched.
        assert_eq!(
            fs::read_to_string(dir.path().join("scripts/source/Example.psc")).unwrap(),
            "ScriptName Example   \n"
        );
        let report: serde_json::Value = serde_json::from_str(&stdout).unwrap();
        assert_eq!(report["success"], true);
        assert_eq!(report["total_diagnostics"], 0);
    }

    #[test]
    fn config_flag_errors_when_the_override_file_is_missing() {
        let dir = tempfile::tempdir().expect("failed to create temp dir");
        write_file(
            &dir.path().join("scripts/source/Example.psc"),
            "ScriptName Example\n",
        );
        write_file(
            &dir.path().join("sources.achlist"),
            r#"["scripts/source/Example.psc"]"#,
        );
        let achlist_path = dir.path().join("sources.achlist");
        let missing_config = dir.path().join("missing.yaml");

        let (code, _stdout, stderr) = run_captured(&[
            "--config".to_string(),
            missing_config.to_string_lossy().into_owned(),
            achlist_path.to_string_lossy().into_owned(),
        ]);

        assert_eq!(code, 2);
        assert!(stderr.starts_with("error: failed to load lint config:"));
    }

    #[test]
    fn config_flag_without_a_value_prints_usage() {
        let (code, _stdout, stderr) = run_captured(&["--config".to_string()]);

        assert_eq!(code, 2);
        assert!(stderr.contains("Usage: PapyrusLinterCLI"));
    }

    #[test]
    fn json_flag_can_precede_the_fix_subcommand() {
        let dir = tempfile::tempdir().expect("failed to create temp dir");
        let script_path = dir.path().join("Example.psc");
        write_file(&script_path, "ScriptName Example   \n");

        let (code, stdout, stderr) = run_captured(&[
            "--json".to_string(),
            "fix".to_string(),
            script_path.to_string_lossy().into_owned(),
        ]);

        assert_eq!(code, 0);
        assert!(stderr.is_empty());
        assert_eq!(
            fs::read_to_string(&script_path).unwrap(),
            "ScriptName Example\n"
        );
        let report: serde_json::Value = serde_json::from_str(&stdout).unwrap();
        assert_eq!(report["files_fixed"], 1);
        assert_eq!(report["total_diagnostics"], 0);
        assert_eq!(report["success"], true);
    }

    #[test]
    fn output_flag_writes_the_plain_text_report_to_a_file_instead_of_stdout() {
        let dir = tempfile::tempdir().expect("failed to create temp dir");
        write_file(
            &dir.path().join("scripts/source/Example.psc"),
            "ScriptName Example   \n",
        );
        write_file(
            &dir.path().join("sources.achlist"),
            r#"["scripts/source/Example.psc"]"#,
        );
        let achlist_path = dir.path().join("sources.achlist");
        let output_path = dir.path().join("report.txt");

        let (code, stdout, stderr) = run_captured(&[
            "--output".to_string(),
            output_path.to_string_lossy().into_owned(),
            achlist_path.to_string_lossy().into_owned(),
        ]);

        assert_eq!(code, 0);
        assert!(stderr.is_empty());
        assert!(stdout.is_empty());
        let contents = fs::read_to_string(&output_path).expect("output file should exist");
        assert!(contents.contains("[trailing-whitespace]"));
        assert!(contents.contains("1 problem(s) found in 1 of 1 script(s)"));
    }

    #[test]
    fn output_flag_writes_the_json_report_to_a_file_instead_of_stdout() {
        let dir = tempfile::tempdir().expect("failed to create temp dir");
        write_file(
            &dir.path().join("scripts/source/Example.psc"),
            "ScriptName Example   \n",
        );
        write_file(
            &dir.path().join("sources.achlist"),
            r#"["scripts/source/Example.psc"]"#,
        );
        let achlist_path = dir.path().join("sources.achlist");
        let output_path = dir.path().join("report.json");

        let (code, stdout, stderr) = run_captured(&[
            "--json".to_string(),
            "--output".to_string(),
            output_path.to_string_lossy().into_owned(),
            achlist_path.to_string_lossy().into_owned(),
        ]);

        assert_eq!(code, 0);
        assert!(stderr.is_empty());
        assert!(stdout.is_empty());
        let contents = fs::read_to_string(&output_path).expect("output file should exist");
        let report: serde_json::Value =
            serde_json::from_str(&contents).expect("output file should contain a JSON document");
        assert_eq!(report["success"], true);
        assert_eq!(report["total_diagnostics"], 1);
    }

    #[test]
    fn output_flag_combines_with_fix() {
        let dir = tempfile::tempdir().expect("failed to create temp dir");
        write_file(
            &dir.path().join("scripts/source/Example.psc"),
            "ScriptName Example   \n",
        );
        write_file(
            &dir.path().join("sources.achlist"),
            r#"["scripts/source/Example.psc"]"#,
        );
        let achlist_path = dir.path().join("sources.achlist");
        let output_path = dir.path().join("report.txt");

        let (code, stdout, _stderr) = run_captured(&[
            "fix".to_string(),
            "--output".to_string(),
            output_path.to_string_lossy().into_owned(),
            achlist_path.to_string_lossy().into_owned(),
        ]);

        assert_eq!(code, 0);
        assert!(stdout.is_empty());
        assert_eq!(
            fs::read_to_string(dir.path().join("scripts/source/Example.psc")).unwrap(),
            "ScriptName Example\n"
        );
        let contents = fs::read_to_string(&output_path).expect("output file should exist");
        assert!(contents.contains("no problems found in 1 script"));
        assert!(contents.contains("(1 script(s) fixed.)"));
    }

    #[test]
    fn output_flag_errors_when_the_directory_does_not_exist() {
        let dir = tempfile::tempdir().expect("failed to create temp dir");
        write_file(
            &dir.path().join("scripts/source/Example.psc"),
            "ScriptName Example\n",
        );
        write_file(
            &dir.path().join("sources.achlist"),
            r#"["scripts/source/Example.psc"]"#,
        );
        let achlist_path = dir.path().join("sources.achlist");
        let output_path = dir.path().join("missing-dir/report.txt");

        let (code, _stdout, stderr) = run_captured(&[
            "--output".to_string(),
            output_path.to_string_lossy().into_owned(),
            achlist_path.to_string_lossy().into_owned(),
        ]);

        assert_eq!(code, 2);
        assert!(stderr.starts_with("error: failed to write"));
    }

    #[test]
    fn output_flag_without_a_value_prints_usage() {
        let (code, _stdout, stderr) = run_captured(&["--output".to_string()]);

        assert_eq!(code, 2);
        assert!(stderr.contains("Usage: PapyrusLinterCLI"));
    }
}
