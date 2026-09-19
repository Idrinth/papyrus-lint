//! Library backing the `PapyrusLinterCLI` command-line interface.
//!
//! ```text
//! PapyrusLinterCLI [--json | --format <plain|json|ai>] [--hash-source] [--quiet-warnings] [--quiet-info] [--tag <kind>] <path-to-achlist-or-psc-or-directory>
//! PapyrusLinterCLI [--json | --format <plain|json|ai>] [--hash-source] [--quiet-warnings] [--quiet-info] [--config <path>] [--tag <kind>] --blob <source>
//! PapyrusLinterCLI [--json | --format <plain|json|ai>] [--hash-source] [--quiet-warnings] [--quiet-info] fix [--type <rule-id> | --tag <kind>] [--line <n>] <path-to-achlist-or-psc-or-directory>
//! PapyrusLinterCLI init [--preset <strict|standard|careful|custom-name>]
//! PapyrusLinterCLI preset add <name> <path-to-papyrus-lint.yaml> [--yes]
//! PapyrusLinterCLI preset list
//! PapyrusLinterCLI doctor [--json] [--config <path>] [--script-root <path>]... <path-to-achlist-or-psc-or-directory>
//! ```
//!
//! Resolves every `.psc` entry listed in the given `.achlist` file (see
//! [`papyrus_lint_core::achlist`]) — or, if given a single `.psc` file
//! directly, treats that file as the achlist's sole entry, or, if given a
//! directory, recursively scans it (and every subdirectory beneath it, at
//! any depth) for `.psc` files instead (see
//! [`papyrus_lint_core::script_locator::find_psc_files_recursively`]) —
//! lints each against the project's `papyrus-lint.yaml`/`.yml`
//! configuration, falling back to [`papyrus_lints::Config::default`] if it
//! has none (see [`papyrus_lint_config`]) — and prints the
//! diagnostics found, one per line. The directory-scan mode is for a
//! project with no `.achlist` at all whose scripts are spread across
//! arbitrarily nested subfolders (e.g. Requiem's own layout) rather than
//! flatly under `scripts/source`. The project root the config (and the
//! function table below) is looked up under is found by walking up from a
//! resolved `.psc` file's own position for a `scripts/source`/
//! `source/scripts` directory pair (matching
//! [`papyrus_lint_core::script_locator::CANDIDATE_DIRS`], case-insensitively)
//! and taking the directory above that pair, e.g. `Data` for
//! `Data\\Scripts\\Source\\abc.psc` — which also finds the right root for a
//! script nested further still, e.g. a namespaced
//! `Data\\Scripts\\Source\\User\\abc.psc`. For a bare `.psc` file given directly,
//! that walk starts from the file itself; if no such pair is found in the
//! path at all (e.g. a project laid out some other way, like Requiem's own,
//! arbitrarily nested layout), it instead looks for the nearest ancestor
//! directory that already has a `papyrus-lint.yaml`/`.yml`
//! ([`papyrus_lint_core::project_root::find_psc_project_root`]'s
//! `find_config_file_root` step), so that project's real config still gets
//! picked up rather than silently falling back to the built-in defaults —
//! only falling back to the previous fixed two-directories-up guess if
//! neither finds anything. For an `.achlist` or a
//! directory, the same walk is tried against each resolved `.psc` entry
//! first, so a project whose `.achlist`/scanned directory sits somewhere
//! other than the project root (e.g. a user drops it next to a game's
//! `Data` directory while the actual project, and its `papyrus-lint.yaml`,
//! live in a subfolder alongside the `scripts/source`/`source/scripts`
//! tree) still finds the right root; falling back to the achlist's own
//! parent directory (the previous, simpler rule), or to the scanned
//! directory itself, only if none of the resolved entries match that
//! layout. This is what lets editor plugins that invoke the CLI on a single
//! saved file (see `SublimeLinter-contrib-papyrus-lint/linter.py`) still
//! pick up the project's config regardless of how the project organizes its
//! scripts under `scripts/source`. Calls to functions declared on other
//! scripts under the project root are resolved the same way the desktop app
//! resolves them (see [`papyrus_lint_core::function_table`]), so the CLI's
//! "Argument type check"/"Return type check" results match the app's.
//!
//! With the `fix` subcommand, every automatic fix (see
//! [`papyrus_lints::repair`]) is applied to each resolved script first,
//! rewriting it on disk if it changed, before the (now possibly smaller)
//! set of remaining diagnostics is reported the same way.
//!
//! `fix` also accepts `--type <rule-id>` (matched case-insensitively, `_`
//! and `-` interchangeable, e.g. `trailing_whitespace` or
//! `trailing-whitespace`) to apply only that one automatic fix (see
//! [`papyrus_lints::FIXABLE_RULE_IDS`]) instead of every enabled one, and
//! `--line <n>` to further restrict whichever fix(es) run to just that
//! 1-indexed line, leaving every other line exactly as it was. Both flags
//! are only valid with `fix` and can be combined, e.g. `fix --line=12
//! --type=trailing_whitespace path/to/Example.psc`. `--line` errors out if
//! a fix it would apply changes the file's line count (e.g.
//! `property-sorting` relocating a property's declaration), since a single
//! original line number no longer identifies the same line in the result.
//!
//! `fix` also accepts `--dry-run`, which computes the same fix(es) (honoring
//! `--type`/`--tag`/`--line` the same way) but never writes them to disk:
//! instead, for each script that would change, a standard unified diff
//! (matching `diff -u`'s hunk format, three lines of context) between the
//! original and would-be-fixed source is printed as part of the report,
//! headed by `--- <path>`/`+++ <path>` lines using that script's already-
//! resolved display path. The diagnostics reported afterward still reflect
//! the would-be-fixed source, the same as a real `fix` run, so `--dry-run`
//! shows both what would change and what would still be left afterward.
//! With `--json`, each file's diff (when non-empty) is carried in its own
//! `diff` field instead of being interleaved with the plain-text report; the
//! top-level report also carries a `dry_run` boolean. `--dry-run` is a
//! usage error without `fix`, the same as `--type`/`--line`.
//!
//! `--tag <kind>` (matched case-insensitively against the kind keyword(s)
//! published by [`papyrus_lints::tags`], e.g. `style`, `performance`,
//! `correctness`, `maintainability`) restricts a run to just one class of
//! rules instead of a single rule id, the tag-based counterpart to
//! `--type`. Without `fix`, it limits the reported diagnostics to rules
//! tagged with that kind; with `fix`, it also limits which automatic fixes
//! run to that same kind (see [`papyrus_lints::repair_filtered_by_tag`]).
//! Unlike `--type`/`--line`, `--tag` doesn't require `fix`. It can't be
//! combined with `--type`, since the two select overlapping things (one
//! rule vs. one kind of rule); an unrecognized tag is a usage error.
//!
//! `init` accepts its own `--preset <name>` flag (`strict`, `standard`, or
//! `careful`, matched case-insensitively; see
//! [`papyrus_lint_config::presets::Preset`] and `configuration/presets/`), selecting
//! which baseline `papyrus-lint.yaml` it generates. Defaults to `strict`,
//! identical to the engine's built-in default, so plain `init` is
//! unaffected by this flag existing at all. Any other name is looked up as
//! a user preset: a `<name>.yaml`/`.yml` file (matched case-insensitively)
//! under a `presets` directory next to the running executable (the CLI
//! binary itself, or the desktop app's binary when it delegates to CLI
//! mode) — see [`papyrus_lint_config::presets::USER_PRESETS_DIR_NAME`]. An
//! executable-adjacent base config file (see
//! [`papyrus_lint_config::presets::initialize_default_config`]) still layers
//! on top of whichever preset is selected the same way it layers over the
//! built-in default. A `--preset` value that matches neither a built-in nor
//! a file in the `presets` directory is reported as an error once `init`
//! actually runs (a missing `--preset` value, or an argument that isn't
//! `--preset`/`--preset=<name>` at all, is still a usage error reported
//! immediately).
//!
//! `preset add <name> <path-to-papyrus-lint.yaml>` adds a user preset,
//! selectable afterward the same way as a built-in one via `--preset
//! <name>` (see [`papyrus_lint_config::presets::add_user_preset`]): it copies
//! the file at `<path-to-papyrus-lint.yaml>` into the executable-adjacent
//! `presets` directory (see
//! [`papyrus_lint_config::presets::USER_PRESETS_DIR_NAME`]) as `<name>.yaml`,
//! creating that directory first if it doesn't exist yet. `<name>` can't be
//! blank or match a built-in preset name (`strict`, `standard`, `careful`)
//! case-insensitively, since such a name could never actually be selected
//! (a built-in always resolves first). If a preset named `<name>` already
//! exists, this refuses to overwrite it and reports an error naming the
//! existing file, unless `--yes` is also given — the confirmation an
//! overwrite requires, since there's no interactive prompt.
//!
//! `preset list` prints the name of every preset selectable via `--preset
//! <name>`, one per line: the three built-ins (`strict`, `standard`,
//! `careful`) first, then any user preset found under the
//! executable-adjacent `presets` directory (see
//! [`papyrus_lint_config::presets::list_user_preset_names`]), in
//! alphabetical order.
//!
//! `doctor` validates a project's setup — the paths its configuration
//! assumes or names — without linting any script: that the given
//! achlist/`.psc`/directory path itself exists (and, for an achlist, that
//! every listed entry exists on disk); that a discovered (or
//! `--config`-overridden) `papyrus-lint.yaml`/`.yml` actually parses; that
//! at least one of `scripts/source`/`source/scripts` exists under the
//! resolved project root; that each configured `additional_script_roots`
//! entry (and any `--script-root` given alongside `doctor`) resolves to an
//! existing directory; that each configured `lookup_script_roots` entry
//! (analysis-only fallback directories, never linted) resolves to an
//! existing directory; and that a configured, or auto-detected,
//! `compiler_path` points at an existing file — warning instead if
//! `compile_check` is enabled but no compiler path could be resolved at
//! all. Each check is printed as its own `[ok]`/`[warning]`/`[error]
//! <message>` line, or, with `--json`, as part of a single JSON document
//! (see [`DoctorReport`]) instead. Exits `0` if every check passed, `1` if
//! any reported a `warning` or `error`, or `2` on a usage error.
//!
//! `--blob <source>` (see [`run_blob`]) lints `<source>` itself as raw
//! Papyrus source text — e.g. an editor buffer that isn't (yet, or ever)
//! saved to disk — in place of the usual achlist/`.psc`/directory path, and
//! reports it under the literal path `<blob>`. There's no real project
//! behind it, so this skips project root discovery, cross-script argument/
//! return type resolution, the `conflicting_script_versions`/
//! `stale_compiled_output`/`script_filename_mismatch` project lints, and
//! `compile_check` entirely — a call into another script is treated the
//! same as one into an unknown script. `--config <path>` still selects an
//! explicit configuration file; without it, the engine's default
//! configuration applies, since there's no project root to discover one
//! from. Combinable with `--json`/`--format`/`--hash-source`/
//! `--quiet-warnings`/`--quiet-info`/`--tag`/`--color`/`--output`, in any
//! argument order, but a usage error alongside a path argument, `fix`,
//! `--type`, `--line`, `--dry-run`, `--script-root`, `--progress`, or
//! `--threads` — none of which mean anything without a real file to
//! resolve or write back to.
//!
//! With the `--json` flag (combinable with `fix`, in either argument
//! order), the diagnostics report is printed to stdout as a single JSON
//! document (see [`JsonReport`]) instead of the plain-text format, so
//! editor plugins and other tooling can consume it without scraping text.
//! `--quiet-warnings` and `--quiet-info` omit diagnostics of the corresponding
//! severity from either report format without changing the process exit code.
//!
//! `--format ai` (see [`AiFileReport`]) produces the same self-contained
//! export the desktop app's "Export for AI" button downloads: each
//! resolved script's diagnostics alongside its `source` — an object naming
//! which of `content` (the script's full text, the default) or `hash` (its
//! MD5 digest instead, selected via `--hash-source`) it carries. `--hash-
//! source` lets a report be handed to an external AI assistant without
//! exposing proprietary script text, while a viewer can still tell files
//! apart, or notice a file changed between exports, from the hash alone;
//! it's a usage error without `--format ai`.
//!
//! With `--short-paths` (combinable with `fix`/`--json` in any order), each
//! script's path in the report (plain text or JSON) has the project root's
//! path stripped from its beginning, the same way the desktop app shortens
//! paths in its own results list; a path that isn't under the project root
//! is left unchanged. This only affects the printed report, not the paths
//! used in usage/error text.
//!
//! With `--config <path>` (combinable with `fix`/`--json` in any order),
//! lint configuration is loaded directly from `<path>` instead of being
//! discovered from the project root, letting a caller (e.g. an editor
//! plugin with its own configured override) point at a config file with
//! any name, anywhere on disk. This also skips the project root's own
//! `additional_script_roots` (see below), since that config file is no
//! longer being read at all — but `lookup_script_roots` and
//! `strict_achlist_scope` (see
//! [`papyrus_lint_config::load_lookup_script_roots_from_path`] /
//! [`papyrus_lint_config::load_strict_achlist_scope_from_path`]) are
//! still read from `<path>` itself, the same as every other lint setting.
//!
//! With one or more `--script-root <path>` flags (combinable with
//! `fix`/`--json`/`--config` in any order), each given directory (resolved
//! relative to the project root unless already absolute) is searched
//! alongside `scripts/source`/`source/scripts` and the project's configured
//! `additional_script_roots` (see [`papyrus_lint_config::load_script_roots`])
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
//! With `--progress` (combinable with `fix`/`--json`/`--config`/
//! `--script-root`/`--short-paths` in any order), a live `<files
//! linted>/<total files to lint>` progress bar is written to stdout as each
//! script finishes linting. This only makes sense when the report itself
//! isn't also going to stdout, so `--progress` requires `--output <path>`
//! to be given alongside it — it's a usage error (exit code `2`) otherwise,
//! in every other mode (plain stdout or `--json` to stdout).
//!
//! With `--color <auto|always|never>` (default `auto`, combinable with
//! every flag above), the plain-text report's diagnostic locations, rule
//! tags, and `[error]`/`[warning]`/`[info]` level tags are colorized with
//! ANSI escapes. `auto` colorizes only when the caller reports stdout as a
//! terminal (see [`run`]'s `stdout_is_terminal` parameter, which both binary
//! entry points set from `std::io::Stdout::is_terminal`), `--output` isn't
//! used (a file isn't a terminal), and the environment allows it under the
//! [NO_COLOR](https://no-color.org/) / [CLICOLOR](https://bixense.com/clicolors/)
//! conventions (`NO_COLOR` or `CLICOLOR=0` disable auto color;
//! `CLICOLOR_FORCE` enables it even when stdout is not a TTY); `always`/
//! `never` override that detection outright. `--json`
//! output is never colorized, since it's meant for tooling rather than a
//! terminal.
//!
//! With `--threads <n>` (combinable with every flag above), up to `<n>`
//! scripts are read, fixed, and linted concurrently instead of one at a
//! time, via [`papyrus_lint_core::parallel::map_in_parallel`] — the
//! reported diagnostics/JSON/AI output is unaffected, since results are
//! always reassembled in the scripts' original order regardless of which
//! order the worker threads actually finish them in. Defaults to the
//! machine's available parallelism ([`papyrus_lint_core::parallel::default_thread_count`]);
//! `--threads 1` forces the previous fully sequential behavior. `<n>` must
//! be a positive integer.
//!
//! This crate is used both by the standalone `PapyrusLinterCLI` binary
//! (`src/main.rs`) and by the desktop app (`app/src-tauri`), which runs it in
//! place of launching its GUI whenever it's given command-line arguments.

mod args;
mod blob;
mod doctor;
mod init;
mod output;
mod project;
mod report;
mod run_fix;
mod run_lint;
mod run_lint_command;
mod run_scan;

pub use output::{JsonDiagnostic, JsonFileReport, JsonReport};

use args::{parse_cli, write_args_error, ParsedCli, ParsedCommand};
use blob::run_blob;
use doctor::run_doctor;
use init::{run_init, run_preset_add, run_preset_list};
use run_lint_command::run_lint_command;

use std::io::Write;

/// Command-line help printed on a usage error (`--help`, missing path, etc.).
/// Synopsis lives here; the example invocations are the checked-in
/// `docs/papyrus-cli-usage.txt` so that file and this help stay in lockstep.
/// Contact URLs are generated at compile time from `shared/links.yaml`,
/// filtered by the `contact` tag rather than named individually.
pub const USAGE: &str = concat!(
    "Usage: PapyrusLinterCLI [--json | --format <plain|json|ai>] [--hash-source] [--quiet-warnings] [--quiet-info] [--short-paths] [--config <path>] [--script-root <path>]... [--output <path>] [--progress] [--threads <n>] [--tag <kind>] <path-to-achlist-or-psc-or-directory>\n       ",
    "PapyrusLinterCLI [--json | --format <plain|json|ai>] [--hash-source] [--quiet-warnings] [--quiet-info] [--config <path>] [--output <path>] [--color <when>] [--tag <kind>] --blob <source>\n       ",
    "PapyrusLinterCLI [--json | --format <plain|json|ai>] [--hash-source] [--quiet-warnings] [--quiet-info] [--short-paths] [--config <path>] [--script-root <path>]... [--output <path>] [--progress] [--threads <n>] fix [--type <rule-id> | --tag <kind>] [--line <n>] [--dry-run] <path-to-achlist-or-psc-or-directory>\n\n",
    "PapyrusLinterCLI init [--preset <strict|standard|careful|custom-name>]\n\n",
    "PapyrusLinterCLI preset add <name> <path-to-papyrus-lint.yaml> [--yes]\n\n",
    "PapyrusLinterCLI preset list\n\n",
    "PapyrusLinterCLI doctor [--json] [--config <path>] [--script-root <path>]... <path-to-achlist-or-psc-or-directory>\n\n",
    "Examples:\n",
    include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../../docs/papyrus-cli-usage.txt"
    )),
    "\n\nExit status: 0 if no problems were found (or none met the configured\n",
    "fail_on_warning/fail_on_info threshold), 1 if any did, 2 on a usage or\n",
    "I/O error.\n\n",
    "Contact:\n",
    include_str!(concat!(env!("OUT_DIR"), "/contact_links.txt"))
);

/// The crate's version, as set in `crates/papyrus-lint-cli/Cargo.toml`
/// (kept in sync with the desktop app's version at release time). Printed
/// by `--version`/`-V`.
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

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
///
/// `stdout_is_terminal` reports whether `stdout` is directly connected to an
/// interactive terminal, used to decide `--color auto`'s coloring (see
/// [`ColorChoice`]) — the caller (a binary's `main`) determines this via
/// `std::io::Stdout::is_terminal` rather than `run` checking it itself,
/// since `stdout: &mut impl Write` accepts any writer (including the
/// in-memory buffers this crate's own tests write to), not just a real
/// `Stdout` capable of answering that question on its own.
pub fn run(
    args: &[String],
    stdout: &mut (impl Write + Send),
    stderr: &mut impl Write,
    stdout_is_terminal: bool,
) -> u8 {
    match parse_cli(args) {
        Err(err) => {
            write_args_error(err, stderr);
            2
        }
        Ok(ParsedCli::Init(preset)) => run_init(preset, stdout, stderr),
        Ok(ParsedCli::PresetAdd {
            name,
            source_path,
            overwrite,
        }) => run_preset_add(name, source_path, overwrite, stdout, stderr),
        Ok(ParsedCli::PresetList) => run_preset_list(stdout),
        Ok(ParsedCli::Doctor(raw)) => run_doctor(raw, stdout),
        Ok(ParsedCli::Run(ParsedCommand::Version)) => {
            let _ = writeln!(stdout, "PapyrusLinterCLI {VERSION}");
            0
        }
        Ok(ParsedCli::Run(ParsedCommand::Blob(blob))) => run_blob(
            &blob.source,
            blob.config_path.as_deref(),
            blob.output_format,
            blob.hash_source,
            blob.quiet_warnings,
            blob.quiet_info,
            blob.tag_filter.as_deref(),
            blob.color_choice,
            blob.output_path.as_deref(),
            stdout_is_terminal,
            stdout,
            stderr,
        ),
        Ok(ParsedCli::Run(ParsedCommand::Lint(lint))) => {
            run_lint_command(lint, stdout, stderr, stdout_is_terminal)
        }
    }
}

#[cfg(test)]
mod test_support;

#[cfg(test)]
mod run_tests;

#[cfg(test)]
mod usage_tests {
    #[test]
    fn usage_embeds_checked_in_cli_examples() {
        assert!(crate::USAGE.starts_with("Usage: PapyrusLinterCLI"));
        assert!(crate::USAGE.contains("Examples:"));
        assert!(crate::USAGE.contains("PapyrusLinterCLI path/to/project.achlist"));
        assert!(crate::USAGE.contains("PapyrusLinterCLI --blob"));
        assert!(crate::USAGE.contains("PapyrusLinterCLI doctor path/to/project.achlist"));
    }

    #[test]
    fn usage_embeds_contact_tagged_links_from_shared_yaml() {
        assert!(crate::USAGE.contains("https://discord.gg/idrinth"));
        assert!(crate::USAGE.contains("https://tally.so/r/aQL1dB"));
        assert!(
            !crate::USAGE.contains("marketplace.visualstudio.com"),
            "CLI help should only include contact-tagged links"
        );
    }
}
