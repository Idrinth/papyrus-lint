//! Library backing the `PapyrusLinterCLI` command-line interface.
//!
//! ```text
//! PapyrusLinterCLI [--json | --format <plain|json|ai>] [--hash-source] [--quiet-warnings] [--quiet-info] [--tag <kind>] <path-to-achlist-or-psc-or-directory>
//! PapyrusLinterCLI [--json | --format <plain|json|ai>] [--hash-source] [--quiet-warnings] [--quiet-info] [--config <path>] [--tag <kind>] --blob <source>
//! PapyrusLinterCLI [--json | --format <plain|json|ai>] [--hash-source] [--quiet-warnings] [--quiet-info] fix [--type <rule-id> | --tag <kind>] [--line <n>] <path-to-achlist-or-psc-or-directory>
//! PapyrusLinterCLI init [--preset <strict|standard|careful|custom-name>]
//! PapyrusLinterCLI preset add <name> <path-to-papyrus-lint.yaml> [--yes]
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
//! `Data\Scripts\Source\abc.psc` — which also finds the right root for a
//! script nested further still, e.g. a namespaced
//! `Data\Scripts\Source\User\abc.psc`. For a bare `.psc` file given directly,
//! that walk starts from the file itself, falling back to two directories up
//! if no such pair is found in the path at all. For an `.achlist` or a
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
//! [`papyrus_lint_config::presets::Preset`] and `docs/presets/`), selecting
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
//! used (a file isn't a terminal), and the `NO_COLOR` environment variable
//! isn't set; `always`/`never` override that detection outright. `--json`
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
mod run_fix;
mod run_lint;
mod run_scan;

pub use output::{JsonDiagnostic, JsonFileReport, JsonReport};

use args::{parse_run_args, ArgsError, ParsedCommand};
use blob::run_blob;
use doctor::run_doctor;
use init::{
    initialize_config, parse_init_preset, parse_preset_add_args, report_add_user_preset,
    InitPresetError, PresetAddArgsError,
};
use output::*;
use project::*;
use run_fix::fix_file;
use run_lint::{lint_file, LintContext};
use run_scan::scan_project;

use std::fs;
use std::io::Write;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Mutex;

use papyrus_lint_config::presets;
use papyrus_lint_core::source_encoding::read_psc_source_with_encoding;

pub const USAGE: &str =
    "Usage: PapyrusLinterCLI [--json | --format <plain|json|ai>] [--hash-source] [--quiet-warnings] [--quiet-info] [--short-paths] [--config <path>] [--script-root <path>]... [--output <path>] [--progress] [--threads <n>] [--tag <kind>] <path-to-achlist-or-psc-or-directory>\n       \
PapyrusLinterCLI [--json | --format <plain|json|ai>] [--hash-source] [--quiet-warnings] [--quiet-info] [--config <path>] [--output <path>] [--color <when>] [--tag <kind>] --blob <source>\n       \
PapyrusLinterCLI [--json | --format <plain|json|ai>] [--hash-source] [--quiet-warnings] [--quiet-info] [--short-paths] [--config <path>] [--script-root <path>]... [--output <path>] [--progress] [--threads <n>] fix [--type <rule-id> | --tag <kind>] [--line <n>] [--dry-run] <path-to-achlist-or-psc-or-directory>\n\n\
PapyrusLinterCLI init [--preset <strict|standard|careful|custom-name>]\n\n\
PapyrusLinterCLI preset add <name> <path-to-papyrus-lint.yaml> [--yes]\n\n\
PapyrusLinterCLI doctor [--json] [--config <path>] [--script-root <path>]... <path-to-achlist-or-psc-or-directory>\n\n\
Lints every .psc script listed in the given .achlist file, a single\n\
.psc file given directly, or every .psc file found recursively under a\n\
given directory (any depth of subfolders), using the project's\n\
papyrus-lint.yaml/.yml configuration (looked up next to the .achlist\n\
file or scanned directory, or two directories up from a bare .psc file,\n\
e.g. Data for Data\\Scripts\\Source\\abc.psc; falling back to defaults\n\
if it has none).\n\n\
With the `fix` subcommand, applies every automatic fix (see README.md)\n\
to those scripts first, rewriting each one on disk if it changed, then\n\
reports whatever diagnostics remain the same way. With --dry-run, no\n\
file is written; a standard diff of what would have changed is printed\n\
instead.\n\n\
With the `init` subcommand, creates a papyrus-lint.yaml in the current\n\
working directory without overwriting an existing config, from the\n\
selected --preset (strict, standard, or careful; defaults to strict,\n\
identical to today's built-in default; any other name is looked up as\n\
<name>.yaml/.yml in a presets directory next to the executable).\n\n\
With the `preset add` subcommand, adds a user preset named <name> by\n\
copying <path-to-papyrus-lint.yaml> into a presets directory next to the\n\
executable, so it becomes selectable via --preset <name> just like a\n\
built-in preset. Refuses a blank name or one matching a built-in preset\n\
(strict, standard, careful). Refuses to overwrite an existing preset\n\
of the same name unless --yes is also given.\n\n\
With the `doctor` subcommand, validates a project's configuration and the\n\
paths it assumes or names (conventional script directories, configured\n\
additional_script_roots/lookup_script_roots/compiler_path, and an achlist's own listed entries)\n\
without linting any script, accepting the same --config/--script-root\n\
flags as a normal run and printing one [ok]/[warning]/[error] line per\n\
check (or a single JSON document with --json).\n\n\
With --blob <source>, lints <source> itself as raw Papyrus source text\n\
instead of resolving an achlist/.psc/directory path, e.g. for a script\n\
buffer that isn't (yet) saved to disk. Reported as the literal path\n\
<blob>. Can't be combined with a path argument, `fix`, --type, --line,\n\
--dry-run, --script-root, --progress, or --threads.\n\n\
Options:\n\
  -h, --help              Show this help message\n\
  -V, --version           Print the PapyrusLinterCLI version\n\
  --json                  Print the report as JSON (alias for --format json)\n\
  --format <format>       Print as plain text, JSON, or a self-contained AI export\n\
                          with source, diagnostics, triggered-rule details, and\n\
                          tool/version metadata—everything an AI needs to assist\n\
                          (plain, json, or ai)\n\
  --hash-source           --format ai only: report each file's source as an\n\
                          md5 hash instead of its full content, e.g. to avoid\n\
                          exposing proprietary script text to an external AI.\n\
                          A usage error without --format ai.\n\
  --quiet-warnings        Hide warning-level diagnostics from the report\n\
  --quiet-info            Hide info-level diagnostics from the report\n\
  --short-paths           Strip the project root from each script's path in\n\
                          the report, the same way the desktop app shortens\n\
                          paths in its results list\n\
  --config <path>         Load lint configuration from this file instead of\n\
                          discovering papyrus-lint.yaml/.yml from the project root\n\
                          (also disables the project root's additional_script_roots;\n\
                          use --script-root to add any back explicitly.\n\
                          lookup_script_roots is still read from this file)\n\
  --script-root <path>    An extra directory (relative to the project root,\n\
                          or absolute) to search for .psc files, besides\n\
                          scripts/source, source/scripts, and the project's\n\
                          configured additional_script_roots. Repeatable.\n\
  --output <path>         Write the report (plain text or JSON, per --json) to\n\
                          this file instead of stdout.\n\
  --progress              Print a live files-linted/total-files progress bar\n\
                          to stdout as each script finishes. Requires --output\n\
                          (a usage error otherwise, since the report itself\n\
                          would otherwise also be writing to stdout).\n\
  --color <when>          Colorize the plain-text report: auto (default),\n\
                          always, or never. auto colors only when stdout is a\n\
                          terminal, --output isn't used, and NO_COLOR is unset.\n\
  --threads <n>           Read/fix/lint up to <n> scripts concurrently instead\n\
                          of one at a time. Defaults to the machine's available\n\
                          parallelism; --threads 1 forces sequential processing.\n\
                          Output is always reassembled in the same order\n\
                          regardless of thread count.\n\
  --type <rule-id>        fix only: apply only this rule's automatic fix\n\
                          (e.g. trailing-whitespace or trailing_whitespace)\n\
                          instead of every enabled one.\n\
  --line <n>              fix only: apply the selected fix(es) only to this\n\
                          1-indexed line, leaving every other line untouched.\n\
                          Combinable with --type. Errors if the fix would\n\
                          change the file's line count (e.g. property-sorting\n\
                          or unused-import).\n\
  --dry-run               fix only: don't write any changes to disk; print a\n\
                          standard diff of what would change instead.\n\
  --tag <kind>            Restrict to rules tagged with this kind (e.g. style,\n\
                          performance, correctness, maintainability), matched\n\
                          case-insensitively. Without fix, limits the reported\n\
                          diagnostics; with fix, also limits which automatic\n\
                          fixes run. Valid with or without fix. Can't be\n\
                          combined with --type.\n\
  --blob <source>         Lint <source> directly as raw Papyrus source text\n\
                          instead of a path, reported as <blob>. Can't be\n\
                          combined with a path argument, fix, --type, --line,\n\
                          --dry-run, --script-root, --progress, or --threads.\n\
  --preset <name>         init only: the baseline papyrus-lint.yaml to\n\
                          generate (strict, standard, or careful; see\n\
                          README.md). Defaults to strict, identical to the\n\
                          built-in default. Any other name is looked up as\n\
                          <name>.yaml/.yml in a presets directory next to\n\
                          the executable.\n\
  --yes                   preset add only: confirm overwriting an existing\n\
                          preset of the same name. Without it, an existing\n\
                          preset is left untouched and an error is reported.\n\n\
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
    if args.first().map(String::as_str) == Some("init") {
        let preset = match parse_init_preset(&args[1..]) {
            Ok(preset) => preset,
            Err(InitPresetError::Usage) => {
                let _ = write!(stderr, "{USAGE}");
                return 2;
            }
        };

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
        return initialize_config(&current_dir, preset, stdout, stderr);
    }

    if args.first().map(String::as_str) == Some("preset") {
        if args.get(1).map(String::as_str) != Some("add") {
            let _ = write!(stderr, "{USAGE}");
            return 2;
        }
        let (name, source_path, overwrite) = match parse_preset_add_args(&args[2..]) {
            Ok(parsed) => parsed,
            Err(PresetAddArgsError::Usage) => {
                let _ = write!(stderr, "{USAGE}");
                return 2;
            }
        };
        let result = presets::add_user_preset(&name, &source_path, overwrite);
        return report_add_user_preset(&name, result, stdout, stderr);
    }

    if args.first().map(String::as_str) == Some("doctor") {
        return run_doctor(&args[1..], stdout, stderr);
    }

    let parsed = match parse_run_args(args) {
        Ok(parsed) => parsed,
        Err(ArgsError::Usage) => {
            let _ = write!(stderr, "{USAGE}");
            return 2;
        }
        Err(ArgsError::JsonAndFormatConflict) => {
            let _ = writeln!(stderr, "error: --json and --format can't be combined");
            return 2;
        }
        Err(ArgsError::InvalidFormat(value)) => {
            let _ = writeln!(
                stderr,
                "error: --format must be 'plain', 'json', or 'ai', got '{value}'"
            );
            return 2;
        }
        Err(ArgsError::HashSourceRequiresAi) => {
            let _ = writeln!(stderr, "error: --hash-source requires --format ai");
            return 2;
        }
        Err(ArgsError::InvalidColor(value)) => {
            let _ = writeln!(
                stderr,
                "error: --color must be 'auto', 'always', or 'never', got '{value}'"
            );
            return 2;
        }
        Err(ArgsError::BlobWithPathArgument) => {
            let _ = writeln!(
                stderr,
                "error: --blob can't be combined with a path argument (or `fix`)"
            );
            return 2;
        }
        Err(ArgsError::BlobWithFixFlags) => {
            let _ = writeln!(
                stderr,
                "error: --blob can't be combined with fix/--type/--line/--dry-run"
            );
            return 2;
        }
        Err(ArgsError::BlobWithScriptRootProgressThreads) => {
            let _ = writeln!(
                stderr,
                "error: --blob can't be combined with --script-root/--progress/--threads"
            );
            return 2;
        }
        Err(ArgsError::UnknownTag(value)) => {
            let _ = writeln!(stderr, "error: unknown tag '{value}'");
            return 2;
        }
        Err(ArgsError::ProgressRequiresOutput) => {
            let _ = writeln!(stderr, "error: --progress requires --output <path>");
            return 2;
        }
        Err(ArgsError::TypeAndTagConflict) => {
            let _ = writeln!(stderr, "error: --type and --tag can't be combined");
            return 2;
        }
        Err(ArgsError::UnknownRule(value)) => {
            let _ = writeln!(stderr, "error: unknown rule '{value}'");
            return 2;
        }
        Err(ArgsError::RuleHasNoFix(value)) => {
            let _ = writeln!(stderr, "error: rule '{value}' has no automatic fix");
            return 2;
        }
        Err(ArgsError::InvalidLine(value)) => {
            let _ = writeln!(
                stderr,
                "error: --line must be a positive integer, got '{value}'"
            );
            return 2;
        }
        Err(ArgsError::InvalidThreads(value)) => {
            let _ = writeln!(
                stderr,
                "error: --threads must be a positive integer, got '{value}'"
            );
            return 2;
        }
    };

    let lint = match parsed {
        ParsedCommand::Version => {
            let _ = writeln!(stdout, "PapyrusLinterCLI {VERSION}");
            return 0;
        }
        ParsedCommand::Blob(blob) => {
            return run_blob(
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
            );
        }
        ParsedCommand::Lint(lint) => lint,
    };

    let fix = lint.fix;
    let input_path = lint.input_path;
    let output_format = lint.output_format;
    let json = output_format != OutputFormat::Plain;
    let quiet_warnings = lint.quiet_warnings;
    let quiet_info = lint.quiet_info;
    let short_paths = lint.short_paths;
    let progress = lint.progress;
    let dry_run = lint.dry_run;
    let hash_source = lint.hash_source;
    let config_path = lint.config_path;
    let output_path = lint.output_path;
    let cli_script_roots = lint.cli_script_roots;
    let tag_filter = lint.tag_filter;
    let rule_filter = lint.rule_filter;
    let target_line = lint.target_line;
    let color_choice = lint.color_choice;
    let thread_count = lint.thread_count;

    let scan = match scan_project(&input_path, config_path.as_deref(), cli_script_roots) {
        Ok(scan) => scan,
        Err(message) => {
            let _ = writeln!(stderr, "{message}");
            return 2;
        }
    };
    let script_paths = scan.script_paths;
    let lint_config = scan.lint_config;

    // `--output <path>` redirects the report to a file, which is never a
    // terminal, so `auto` never colorizes in that case regardless of
    // whether `stdout` itself is one. `NO_COLOR` (https://no-color.org/)
    // is honored the same way most CLIs do: any non-empty or empty value
    // disables auto-coloring.
    let use_color = resolve_color(color_choice, output_path.as_deref(), stdout_is_terminal);

    let mut total_diagnostics = 0usize;
    let mut files_with_diagnostics = 0usize;
    let mut files_fixed = 0usize;
    let mut should_fail = false;
    let mut json_files: Vec<JsonFileReport> = Vec::new();
    let mut ai_files: Vec<AiFileReport> = Vec::new();
    // Buffered so `--output <path>` can redirect the whole report to a file
    // instead of stdout, without duplicating the printing logic below.
    let mut report_buf: Vec<u8> = Vec::new();

    let total_scripts = script_paths.len();
    let function_table_root = scan.function_table.root().to_path_buf();
    let function_table_additional_roots = scan.function_table.additional_roots().to_vec();
    // Every script is otherwise independent, so this table's own cache
    // (of other scripts' cross-referenced signatures) is the only thing
    // `--threads` workers below actually share -- through
    // `SharedFunctionTable`, which locks it only for the duration of a
    // single lookup rather than a whole script's lint pass. Progress
    // ("--progress") is likewise reported through a shared counter rather
    // than each worker's own position in `script_paths`, since completion
    // order no longer matches input order once more than one thread is
    // involved -- the final report still is, via `map_in_parallel`'s
    // ordering guarantee.
    let function_table = Mutex::new(scan.function_table);
    let progress_completed = AtomicUsize::new(0);
    let stdout_dyn: &mut (dyn Write + Send) = stdout;
    let progress_stdout: Mutex<&mut (dyn Write + Send)> = Mutex::new(stdout_dyn);

    // Shared read-only across every script (and, with `--threads`, across
    // every worker thread linting one) -- see `run_lint::LintContext`.
    let lint_context = LintContext {
        lint_config: &lint_config,
        function_table: &function_table,
        function_table_additional_roots: &function_table_additional_roots,
        scripts_by_name: &scan.scripts_by_name,
        script_index: &scan.script_index,
        strict_achlist_scope: scan.strict_achlist_scope,
        compile_check: scan.compile_check,
        compiler_path: &scan.compiler_path,
        tag_filter: tag_filter.as_deref(),
        quiet_warnings,
        quiet_info,
        json,
        output_format,
        hash_source,
        use_color,
    };

    let file_results: Vec<Result<FileOutcome, String>> =
        papyrus_lint_core::parallel::map_in_parallel(
            (0..total_scripts).collect(),
            thread_count,
            |file_index| -> Result<FileOutcome, String> {
                let script_path = &script_paths[file_index];
                let (source, encoding) =
                    read_psc_source_with_encoding(script_path).map_err(|err| {
                        format!("error: failed to read {}: {err}", script_path.display())
                    })?;

                let reported_path = display_path(script_path, &function_table_root, short_paths);

                // `fix` mutates (or, under `--dry-run`, previews) the
                // source first; `run_lint::lint_file` then lints whatever
                // source comes out of that (the original source, if `fix`
                // didn't run or changed nothing).
                let (source, fixed_this_file, file_diff, mut plain_text) = if fix {
                    let outcome = fix_file(
                        script_path,
                        &reported_path,
                        source,
                        encoding,
                        &lint_config,
                        &function_table,
                        tag_filter.as_deref(),
                        rule_filter,
                        target_line,
                        dry_run,
                        json,
                    )?;
                    (
                        outcome.source,
                        outcome.fixed,
                        outcome.diff,
                        outcome.plain_text,
                    )
                } else {
                    (source, false, None, Vec::new())
                };

                let lint_outcome = lint_file(
                    &lint_context,
                    script_path,
                    reported_path,
                    &source,
                    file_diff,
                );
                plain_text.extend_from_slice(&lint_outcome.plain_text);

                if progress {
                    let completed = progress_completed.fetch_add(1, Ordering::SeqCst) + 1;
                    let mut stdout = progress_stdout
                        .lock()
                        .unwrap_or_else(|poisoned| poisoned.into_inner());
                    let _ = write!(stdout, "\rLinting: {completed}/{total_scripts} files");
                    let _ = stdout.flush();
                }

                Ok(FileOutcome {
                    plain_text,
                    json_file: lint_outcome.json_file,
                    ai_file: lint_outcome.ai_file,
                    should_fail: lint_outcome.should_fail,
                    has_diagnostics: lint_outcome.has_diagnostics,
                    diagnostic_count: lint_outcome.diagnostic_count,
                    fixed: fixed_this_file,
                })
            },
        );

    if let Some(message) = file_results.iter().find_map(|result| result.as_ref().err()) {
        let _ = writeln!(stderr, "{message}");
        return 2;
    }

    let stdout = progress_stdout
        .into_inner()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    for result in file_results {
        let outcome = result.expect("checked for errors above");
        report_buf.extend_from_slice(&outcome.plain_text);
        if let Some(json_file) = outcome.json_file {
            json_files.push(json_file);
        }
        if let Some(ai_file) = outcome.ai_file {
            ai_files.push(ai_file);
        }
        should_fail = should_fail || outcome.should_fail;
        if outcome.has_diagnostics {
            files_with_diagnostics += 1;
            total_diagnostics += outcome.diagnostic_count;
        }
        if outcome.fixed {
            files_fixed += 1;
        }
    }
    if progress {
        let _ = writeln!(stdout);
    }

    let success = !should_fail;

    if output_format == OutputFormat::Json {
        let report = JsonReport {
            files: json_files,
            scripts_checked: script_paths.len(),
            files_with_diagnostics,
            total_diagnostics,
            files_fixed: fix.then_some(files_fixed),
            dry_run,
            success,
        };
        write_json_report(&mut report_buf, &report);
    } else if output_format == OutputFormat::Ai {
        let report = build_ai_report(&lint_config, ai_files, total_diagnostics);
        write_json_report(&mut report_buf, &report);
    } else {
        let fixed_suffix = if fix && dry_run {
            format!(" ({files_fixed} script(s) would be fixed.)")
        } else if fix {
            format!(" ({files_fixed} script(s) fixed.)")
        } else {
            String::new()
        };

        // Green when clean, yellow when problems were found but none crossed
        // the configured failure threshold, red when the run will exit 1.
        let summary_color = if total_diagnostics == 0 {
            ANSI_GREEN
        } else if success {
            ANSI_YELLOW
        } else {
            ANSI_RED
        };

        let summary = if total_diagnostics == 0 {
            format!(
                "PapyrusLinterCLI: no problems found in {} script(s).{fixed_suffix}",
                script_paths.len()
            )
        } else {
            format!(
                "PapyrusLinterCLI: {total_diagnostics} problem(s) found in {files_with_diagnostics} of {} script(s).{fixed_suffix}",
                script_paths.len()
            )
        };
        let _ = writeln!(
            report_buf,
            "{}",
            colorize(&summary, summary_color, use_color)
        );
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

#[cfg(test)]
mod test_support;

#[cfg(test)]
mod run_tests;
