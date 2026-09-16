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
//! has none (see [`papyrus_lint_core::config`]) — and prints the
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
//! [`papyrus_lint_core::config::Preset`] and `docs/presets/`), selecting
//! which baseline `papyrus-lint.yaml` it generates. Defaults to `strict`,
//! identical to the engine's built-in default, so plain `init` is
//! unaffected by this flag existing at all. Any other name is looked up as
//! a user preset: a `<name>.yaml`/`.yml` file (matched case-insensitively)
//! under a `presets` directory next to the running executable (the CLI
//! binary itself, or the desktop app's binary when it delegates to CLI
//! mode) — see [`papyrus_lint_core::config::USER_PRESETS_DIR_NAME`]. An
//! executable-adjacent base config file (see
//! [`papyrus_lint_core::config::initialize_default_config`]) still layers
//! on top of whichever preset is selected the same way it layers over the
//! built-in default. A `--preset` value that matches neither a built-in nor
//! a file in the `presets` directory is reported as an error once `init`
//! actually runs (a missing `--preset` value, or an argument that isn't
//! `--preset`/`--preset=<name>` at all, is still a usage error reported
//! immediately).
//!
//! `preset add <name> <path-to-papyrus-lint.yaml>` adds a user preset,
//! selectable afterward the same way as a built-in one via `--preset
//! <name>` (see [`papyrus_lint_core::config::add_user_preset`]): it copies
//! the file at `<path-to-papyrus-lint.yaml>` into the executable-adjacent
//! `presets` directory (see
//! [`papyrus_lint_core::config::USER_PRESETS_DIR_NAME`]) as `<name>.yaml`,
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
//! [`papyrus_lint_core::config::load_lookup_script_roots_from_path`] /
//! [`papyrus_lint_core::config::load_strict_achlist_scope_from_path`]) are
//! still read from `<path>` itself, the same as every other lint setting.
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

use std::collections::HashMap;
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};

use papyrus_lint_core::content_hash;
use papyrus_lint_core::diff::unified_diff;
use papyrus_lint_core::function_table::{FunctionTable, SharedFunctionTable};
use papyrus_lint_core::script_locator::find_psc_files_recursively;
use papyrus_lint_core::source_encoding::{read_psc_source_with_encoding, write_psc_source};
use papyrus_lint_core::{achlist, ast_cache, compile_diagnostics, compiler, config};

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
        let result = config::add_user_preset(&name, &source_path, overwrite);
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

    let is_psc_file = input_path
        .extension()
        .and_then(|ext| ext.to_str())
        .is_some_and(|ext| ext.eq_ignore_ascii_case("psc"));
    let is_directory = !is_psc_file && input_path.is_dir();

    let script_paths: Vec<PathBuf> = if is_psc_file {
        vec![input_path.clone()]
    } else if is_directory {
        find_psc_files_recursively(&input_path)
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
    // entries, or a scanned directory's own recursively-found entries, are
    // tried the same way first, so a project whose .achlist/scanned
    // directory doesn't live in the project root still resolves correctly;
    // only if none of the resolved scripts sit under such a pair do we fall
    // back to the achlist's own parent directory (the conventional layout)
    // or, for a scanned directory, the directory itself.
    let project_root = if is_psc_file {
        find_psc_project_root(&input_path)
    } else {
        script_paths
            .iter()
            .find_map(|path| find_candidate_pair_root(path))
            .unwrap_or_else(|| {
                if is_directory {
                    input_path.clone()
                } else {
                    input_path
                        .ancestors()
                        .nth(1)
                        .filter(|dir| !dir.as_os_str().is_empty())
                        .map(Path::to_path_buf)
                        .unwrap_or_else(|| PathBuf::from("."))
                }
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

    // `strict_achlist_scope` (off by default) picks between two ways of
    // letting an achlist's entries resolve each other across arbitrary,
    // non-conventional source directories:
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
    //
    // Unlike `additional_script_roots` above, this is read from whichever
    // config file is actually in effect — the project root's own, or the
    // file named by `--config` — rather than being forced off whenever
    // `--config` is used: it isn't a project-root-only setting, so a
    // `--config` file that sets it is honored the same way it is for every
    // other key in that file (see #362).
    let strict_achlist_scope = match config_path.as_deref().map_or_else(
        || config::load_strict_achlist_scope(&project_root),
        config::load_strict_achlist_scope_from_path,
    ) {
        Ok(value) => value,
        Err(err) => {
            let _ = writeln!(stderr, "error: failed to load lint config: {err}");
            return 2;
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

    // Read from the project root's own config the same way `doctor` reports
    // on them (see `run_doctor`), regardless of `--config` — `compile_check`
    // and `compiler_path` aren't part of the lint settings a `--config`
    // override replaces. `compiler_path` is only resolved when `compile_check`
    // is actually enabled, since it's otherwise unused.
    let compile_check = match config::load_compile_check(&project_root) {
        Ok(value) => value,
        Err(err) => {
            let _ = writeln!(stderr, "error: failed to load lint config: {err}");
            return 2;
        }
    };
    let compiler_path = if compile_check {
        match config::resolve_compiler_path(&project_root) {
            Ok(path) => path.unwrap_or_default(),
            Err(err) => {
                let _ = writeln!(stderr, "error: failed to load lint config: {err}");
                return 2;
            }
        }
    } else {
        String::new()
    };
    let compiler_path = compiler_path.trim().to_string();

    // Analysis-only fallback directories: read from whichever config file
    // is actually in effect, the same as `strict_achlist_scope`. They are
    // never mixed into `additional_script_roots`, so they don't get linted
    // and `conflicting_script_versions` never scans them.
    let lookup_script_roots = match config_path.as_deref().map_or_else(
        || config::load_lookup_script_roots(&project_root),
        config::load_lookup_script_roots_from_path,
    ) {
        Ok(roots) => roots,
        Err(err) => {
            let _ = writeln!(stderr, "error: failed to load lint config: {err}");
            return 2;
        }
    };

    let mut function_table =
        FunctionTable::new_with_additional_roots(project_root, additional_script_roots)
            .with_lookup_roots(lookup_script_roots);
    if strict_achlist_scope {
        function_table = function_table.with_known_scripts(&script_paths);
    }

    // Grouped by (lowercased) file name up front so checking for a
    // same-named achlist entry elsewhere in the list, below, stays
    // proportional to how many scripts actually share a name rather than to
    // the achlist's full size. Only needed in strict-scope mode: the
    // off-by-default `script_index` built below already covers this (and
    // more) via the directories just added to `additional_script_roots`.
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

    // Built once up front, rather than re-scanning `function_table`'s search
    // directories for every conflict check or newly resolved script, since a
    // modlist-sized achlist can list hundreds of scripts across as many
    // directories (see #311). Empty (and unused) in strict-scope mode, where
    // `scripts_by_name` above covers this instead.
    let script_index = Arc::new(if !strict_achlist_scope {
        papyrus_lint_core::script_locator::build_script_index(
            function_table.root(),
            function_table.additional_roots(),
        )
    } else {
        HashMap::new()
    });
    if !strict_achlist_scope {
        function_table = function_table.with_script_index(Arc::clone(&script_index));
    }

    // `--output <path>` redirects the report to a file, which is never a
    // terminal, so `auto` never colorizes in that case regardless of
    // whether `stdout` itself is one. `NO_COLOR` (https://no-color.org/)
    // is honored the same way most CLIs do: any non-empty or empty value
    // disables auto-coloring.
    let use_color = match color_choice {
        ColorChoice::Always => true,
        ColorChoice::Never => false,
        ColorChoice::Auto => {
            output_path.is_none() && stdout_is_terminal && std::env::var_os("NO_COLOR").is_none()
        }
    };

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
    let function_table_root = function_table.root().to_path_buf();
    let function_table_additional_roots = function_table.additional_roots().to_vec();
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
    let function_table = Mutex::new(function_table);
    let progress_completed = AtomicUsize::new(0);
    let stdout_dyn: &mut (dyn Write + Send) = stdout;
    let progress_stdout: Mutex<&mut (dyn Write + Send)> = Mutex::new(stdout_dyn);

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

                let mut file_diff: Option<String> = None;
                let mut plain_text: Vec<u8> = Vec::new();
                let mut fixed_this_file = false;
                let source = if fix {
                    // The "unused-import" fix, unlike every other fixable
                    // rule, can only resolve which imports are actually
                    // unused through this project's own cross-script
                    // resolver -- the same `SharedFunctionTable` the lint
                    // pass below uses -- so the fix step needs one too, via
                    // `papyrus_lints`' `_with_external_arguments` repair
                    // family (see that module's own docs).
                    let mut shared = SharedFunctionTable(&function_table);
                    let repaired = match tag_filter.as_deref() {
                        Some(tag) => papyrus_lints::repair_filtered_by_tag_with_external_arguments(
                            &source,
                            &lint_config,
                            &mut shared,
                            Some(tag),
                        ),
                        None => papyrus_lints::repair_filtered_with_external_arguments(
                            &source,
                            &lint_config,
                            &mut shared,
                            rule_filter,
                        ),
                    };
                    let repaired = match target_line {
                    Some(line) => papyrus_lints::restrict_to_line(&source, &repaired, line)
                        .ok_or_else(|| {
                            format!(
                                "error: --line can't be applied to {} because a fix changes the file's line count (e.g. property-sorting); use --type to restrict to a line-preserving fix, or omit --line",
                                script_path.display()
                            )
                        })?,
                    None => repaired,
                };
                    if repaired != source {
                        if dry_run {
                            let diff_text = unified_diff(&reported_path, &source, &repaired);
                            if !json {
                                let _ = write!(plain_text, "{diff_text}");
                            }
                            file_diff = Some(diff_text);
                        } else {
                            write_psc_source(script_path, &repaired, encoding).map_err(|err| {
                                format!("error: failed to write {}: {err}", script_path.display())
                            })?;
                        }
                        fixed_this_file = true;
                    }
                    repaired
                } else {
                    source
                };

                ast_cache::ensure_primed(script_path, &source);
                // Computed up front and merged in via
                // `lint_with_external_arguments_and_extra_diagnostics` below,
                // rather than appended to that call's own result afterward,
                // so a `@disable`/`@disable-file` directive naming one of
                // these path-dependent diagnostics is honored *and* counted
                // as used by the `unused-disable` lint instead of being
                // incorrectly flagged as unused (see
                // `papyrus_lints::lint_with_external_arguments_and_extra_diagnostics`'s
                // own docs).
                let mut project_diagnostics = Vec::new();
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
                            if let Some(same_named) =
                                scripts_by_name.get(&name.to_ascii_lowercase())
                            {
                                project_diagnostics.extend(
                                papyrus_lint_core::script_locator::conflicting_script_versions_among(
                                    script_path,
                                    same_named,
                                ),
                            );
                            }
                        }
                    } else {
                        project_diagnostics.extend(
                            papyrus_lint_core::script_locator::conflicting_script_versions_in_index(
                                script_path,
                                &script_index,
                            ),
                        );
                    }
                }
                if lint_config.rules.stale_compiled_output {
                    project_diagnostics.extend(papyrus_lint_core::stale_pex::check(script_path));
                }
                if lint_config.rules.script_filename_mismatch {
                    project_diagnostics.extend(papyrus_lint_core::script_filename_mismatch::check(
                        script_path,
                        &source,
                    ));
                }
                let mut diagnostics = {
                    let mut shared = SharedFunctionTable(&function_table);
                    papyrus_lints::lint_with_external_arguments_and_extra_diagnostics(
                        &source,
                        &lint_config,
                        &mut shared,
                        project_diagnostics,
                    )
                };
                // Mirrors the desktop app's `lint_with_compile_check`: a
                // `compiler_path` that can't be run at all (missing/misconfigured)
                // is silently left out rather than failing the whole lint run.
                if compile_check && !compiler_path.is_empty() {
                    if let Ok(outcome) = compiler::check_psc_file(
                        Path::new(&compiler_path),
                        script_path,
                        &function_table_additional_roots,
                    ) {
                        if !outcome.success {
                            diagnostics.extend(compile_diagnostics::parse_compile_errors(&outcome));
                        }
                    }
                }
                if let Some(tag) = tag_filter.as_deref() {
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

                // Quiet flags only affect presentation. A hidden diagnostic still
                // participates in the configured failure threshold and exit code.
                let file_should_fail = diagnostics
                    .iter()
                    .any(|diagnostic| lint_config.should_fail_on(diagnostic));
                diagnostics.retain(|diagnostic| {
                    !((quiet_warnings && diagnostic.level() == "warning")
                        || (quiet_info && diagnostic.level() == "info"))
                });

                for diagnostic in &diagnostics {
                    if !json {
                        let _ = writeln!(
                            plain_text,
                            "{}",
                            format_diagnostic_line(&reported_path, diagnostic, use_color)
                        );
                    }
                }

                let mut json_file = None;
                let mut ai_file = None;
                if json {
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
                    if output_format == OutputFormat::Ai && !json_diagnostics.is_empty() {
                        let rule_counts = rule_counts(&json_diagnostics);
                        let severity_counts = severity_counts(&json_diagnostics);
                        let ai_source = if hash_source {
                            AiSource::Hash {
                                algorithm: "md5",
                                hash: content_hash::md5_hex(&source),
                            }
                        } else {
                            AiSource::Content {
                                content: source.clone(),
                            }
                        };
                        ai_file = Some(AiFileReport {
                            path: reported_path.clone(),
                            severity_counts,
                            rule_counts,
                            diagnostics: json_diagnostics,
                            source: ai_source,
                        });
                    } else if output_format == OutputFormat::Json {
                        json_file = Some(JsonFileReport {
                            path: reported_path,
                            diagnostics: json_diagnostics,
                            diff: file_diff,
                        });
                    }
                }

                let has_diagnostics = !diagnostics.is_empty();
                let diagnostic_count = diagnostics.len();

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
                    json_file,
                    ai_file,
                    should_fail: file_should_fail,
                    has_diagnostics,
                    diagnostic_count,
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
        let _ = writeln!(
            report_buf,
            "{}",
            serde_json::to_string_pretty(&report).unwrap_or_else(|_| "{}".to_string())
        );
    } else if output_format == OutputFormat::Ai {
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
            schema: "https://papyrus-lint.idrinth.de/schema/papyrus-lint-ai-export.v3.schema.json",
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
mod tests {
    use super::*;
    use crate::test_support::*;

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
    fn lints_every_psc_found_recursively_under_a_dropped_directory() {
        // Mirrors a mod like Requiem, whose scripts are spread across
        // arbitrarily nested subfolders rather than a flat scripts/source
        // directory, and ships no .achlist at all.
        let dir = tempfile::tempdir().expect("failed to create temp dir");
        write_file(
            &dir.path().join("scripts/source/Top.psc"),
            "ScriptName Top   \n",
        );
        write_file(
            &dir.path().join("scripts/source/Requiem/Sub/Nested.psc"),
            "ScriptName Nested   \n",
        );
        let target = dir.path().join("scripts/source");

        let (code, stdout, stderr) = run_captured(&[target.to_string_lossy().into_owned()]);

        assert_eq!(code, 0, "stderr: {stderr}");
        assert!(stdout.contains("2 script(s)"));
        assert!(stdout.contains("Top.psc"));
        assert!(stdout.contains("Nested.psc"));
    }

    #[test]
    fn directory_scan_reports_no_problems_for_an_empty_directory() {
        let dir = tempfile::tempdir().expect("failed to create temp dir");

        let (code, stdout, _stderr) = run_captured(&[dir.path().to_string_lossy().into_owned()]);

        assert_eq!(code, 0);
        assert!(stdout.contains("no problems found in 0 script"));
    }

    #[test]
    fn directory_scan_resolves_cross_script_calls_across_nested_subfolders() {
        let dir = tempfile::tempdir().expect("failed to create temp dir");
        write_file(
            &dir.path().join("scripts/source/Greeter.psc"),
            "ScriptName Greeter\n\nFunction Greet(String name)\nEndFunction\n",
        );
        write_file(
            &dir.path().join("scripts/source/Requiem/Example.psc"),
            "ScriptName Example\n\nGreeter Property Target Auto\n\nFunction Test()\n    Target.Greet(1)\nEndFunction\n",
        );
        let target = dir.path().join("scripts/source");

        let (code, stdout, _stderr) = run_captured(&[target.to_string_lossy().into_owned()]);

        assert_eq!(code, 1);
        assert!(stdout.contains("[argument-types]"));
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
    fn fix_preserves_a_cp1252_encoded_files_encoding() {
        let dir = tempfile::tempdir().expect("failed to create temp dir");
        let script_path = dir.path().join("scripts/source/Example.psc");
        fs::create_dir_all(script_path.parent().unwrap()).expect("failed to create parent dir");
        // "ScriptName Example   \n\n; caf\xE9\n" with trailing whitespace on
        // the first line to fix, and 0xE9 ("é" in Windows-1252) making the
        // file as a whole invalid UTF-8.
        let mut contents = b"ScriptName Example   \n\n; caf".to_vec();
        contents.push(0xE9);
        contents.push(b'\n');
        fs::write(&script_path, &contents).expect("failed to write test file");
        write_file(
            &dir.path().join("sources.achlist"),
            r#"["scripts/source/Example.psc"]"#,
        );
        let achlist_path = dir.path().join("sources.achlist");

        let (code, stdout, stderr) = run_captured(&[
            "fix".to_string(),
            achlist_path.to_string_lossy().into_owned(),
        ]);

        assert!(stdout.contains("(1 script(s) fixed.)"), "stderr: {stderr}");
        let mut expected = b"ScriptName Example\n\n; caf".to_vec();
        expected.push(0xE9);
        expected.push(b'\n');
        assert_eq!(
            fs::read(&script_path).expect("failed to read back fixed file"),
            expected,
            "fixing must preserve the file's original Windows-1252 encoding"
        );
        assert_eq!(code, 0);
    }

    #[test]
    fn fix_type_filter_applies_only_the_named_rule() {
        let dir = tempfile::tempdir().expect("failed to create temp dir");
        let script_path = dir.path().join("Example.psc");
        write_file(
            &script_path,
            "ScriptName Example\n\nFunction Add(Int left,Int right)\nEndFunction   \n",
        );

        let (code, _stdout, stderr) = run_captured(&[
            "fix".to_string(),
            "--type=comma_spacing".to_string(),
            script_path.to_string_lossy().into_owned(),
        ]);

        assert!(stderr.is_empty());
        assert_eq!(code, 0);
        assert_eq!(
            fs::read_to_string(&script_path).unwrap(),
            "ScriptName Example\n\nFunction Add(Int left, Int right)\nEndFunction   \n"
        );
    }

    #[test]
    fn fix_type_filter_accepts_the_hyphenated_rule_id() {
        let dir = tempfile::tempdir().expect("failed to create temp dir");
        let script_path = dir.path().join("Example.psc");
        write_file(&script_path, "ScriptName Example   \n");

        let (code, _stdout, stderr) = run_captured(&[
            "fix".to_string(),
            "--type".to_string(),
            "trailing-whitespace".to_string(),
            script_path.to_string_lossy().into_owned(),
        ]);

        assert!(stderr.is_empty());
        assert_eq!(code, 0);
        assert_eq!(
            fs::read_to_string(&script_path).unwrap(),
            "ScriptName Example\n"
        );
    }

    #[test]
    fn fix_line_filter_only_touches_the_named_line() {
        let dir = tempfile::tempdir().expect("failed to create temp dir");
        let script_path = dir.path().join("Example.psc");
        write_file(
            &script_path,
            "ScriptName Example   \n\nFunction DoThing()   \nEndFunction\n",
        );

        let (code, _stdout, stderr) = run_captured(&[
            "fix".to_string(),
            "--line=3".to_string(),
            script_path.to_string_lossy().into_owned(),
        ]);

        assert!(stderr.is_empty());
        assert_eq!(code, 0);
        assert_eq!(
            fs::read_to_string(&script_path).unwrap(),
            "ScriptName Example   \n\nFunction DoThing()\nEndFunction\n"
        );
    }

    #[test]
    fn fix_line_and_type_filters_combine() {
        let dir = tempfile::tempdir().expect("failed to create temp dir");
        let script_path = dir.path().join("Example.psc");
        write_file(
            &script_path,
            "ScriptName Example   \n\nFunction Add(Int left,Int right)   \nEndFunction\n",
        );

        let (code, _stdout, stderr) = run_captured(&[
            "fix".to_string(),
            "--line=3".to_string(),
            "--type=comma_spacing".to_string(),
            script_path.to_string_lossy().into_owned(),
        ]);

        assert!(stderr.is_empty());
        assert_eq!(code, 0);
        assert_eq!(
            fs::read_to_string(&script_path).unwrap(),
            "ScriptName Example   \n\nFunction Add(Int left, Int right)   \nEndFunction\n"
        );
    }

    #[test]
    fn tag_filter_restricts_reported_diagnostics_to_the_matching_kind() {
        let dir = tempfile::tempdir().expect("failed to create temp dir");
        let script_path = dir.path().join("Example.psc");
        write_file(
            &script_path,
            "ScriptName Example   \n\nFunction DoThing()\n\tGame.GetPlayer()\nEndFunction\n",
        );

        let (code, stdout, stderr) = run_captured(&[
            "--tag=style".to_string(),
            script_path.to_string_lossy().into_owned(),
        ]);

        assert!(stderr.is_empty());
        assert_eq!(code, 0);
        assert!(stdout.contains("[trailing-whitespace]"));
        assert!(!stdout.contains("[forbidden-functions]"));
    }

    #[test]
    fn tag_filter_matches_case_insensitively() {
        let dir = tempfile::tempdir().expect("failed to create temp dir");
        let script_path = dir.path().join("Example.psc");
        write_file(&script_path, "ScriptName Example   \n");

        let (code, stdout, stderr) = run_captured(&[
            "--tag=STYLE".to_string(),
            script_path.to_string_lossy().into_owned(),
        ]);

        assert!(stderr.is_empty());
        assert_eq!(code, 0);
        assert!(stdout.contains("[trailing-whitespace]"));
    }

    #[test]
    fn tag_filter_works_without_fix() {
        let dir = tempfile::tempdir().expect("failed to create temp dir");
        let script_path = dir.path().join("Example.psc");
        write_file(
            &script_path,
            "ScriptName Example\n\nFunction DoThing()\n\tGame.GetPlayer()\nEndFunction\n",
        );

        let (code, _stdout, stderr) = run_captured(&[
            "--tag=performance".to_string(),
            script_path.to_string_lossy().into_owned(),
        ]);

        assert!(stderr.is_empty());
        assert_eq!(code, 1);
    }

    #[test]
    fn fix_tag_filter_applies_only_fixes_in_that_kind() {
        let dir = tempfile::tempdir().expect("failed to create temp dir");
        let script_path = dir.path().join("Example.psc");
        write_file(
            &script_path,
            "ScriptName Example\n\nFunction DoThing(GlobalVariable akGlobal)\n    akGlobal.GetValueInt()  \nEndFunction\n",
        );

        let (code, _stdout, stderr) = run_captured(&[
            "fix".to_string(),
            "--tag=performance".to_string(),
            script_path.to_string_lossy().into_owned(),
        ]);

        assert!(stderr.is_empty());
        assert_eq!(code, 0);
        let fixed = fs::read_to_string(&script_path).unwrap();
        assert!(!fixed.contains("GetValueInt"));
        // trailing-whitespace is tagged "style", not "performance", so it's
        // left in place by --tag=performance.
        assert!(fixed.contains("  \n"));
    }

    // Builds an achlist with enough scripts, several of them cross-referencing
    // a shared base script, that a multi-threaded run actually exercises
    // more than one worker thread and more than one `SharedFunctionTable`
    // lookup collision -- then checks a `--threads 1` (fully sequential) run
    // and the default multi-threaded run agree byte-for-byte, since threading
    // is only ever meant to change how fast a run finishes, never what it
    // reports or in what order.
    #[test]
    fn threaded_and_sequential_runs_report_identical_results() {
        let dir = tempfile::tempdir().expect("failed to create temp dir");
        write_file(
            &dir.path().join("scripts/source/Base.psc"),
            "ScriptName Base\n\nFunction DoThing(int arg1)\nEndFunction\n",
        );
        let mut entries = vec!["\"scripts/source/Base.psc\"".to_string()];
        for i in 0..12 {
            let name = format!("Child{i}");
            write_file(
                &dir.path().join(format!("scripts/source/{name}.psc")),
                &format!(
                    "ScriptName {name} Extends Base\n\nFunction UseIt()\n    DoThing(\"wrong type\")   \nEndFunction\n"
                ),
            );
            entries.push(format!("\"scripts/source/{name}.psc\""));
        }
        write_file(
            &dir.path().join("sources.achlist"),
            &format!("[{}]", entries.join(", ")),
        );
        let achlist_path = dir.path().join("sources.achlist");

        let (sequential_code, sequential_stdout, _) = run_captured(&[
            "--threads=1".to_string(),
            "--short-paths".to_string(),
            "--json".to_string(),
            achlist_path.to_string_lossy().into_owned(),
        ]);
        let (parallel_code, parallel_stdout, _) = run_captured(&[
            "--threads=8".to_string(),
            "--short-paths".to_string(),
            "--json".to_string(),
            achlist_path.to_string_lossy().into_owned(),
        ]);

        assert_eq!(sequential_code, parallel_code);
        assert_eq!(sequential_stdout, parallel_stdout);
        // Sanity check that this fixture actually triggers diagnostics
        // (the argument-type mismatch on every child script), rather than
        // both runs trivially agreeing on an empty report.
        assert!(sequential_stdout.contains("argument-types"));
    }

    #[test]
    fn line_filter_errors_when_a_fix_changes_the_line_count() {
        let dir = tempfile::tempdir().expect("failed to create temp dir");
        let script_path = dir.path().join("scripts/source/Example.psc");
        write_file(
            &script_path,
            "ScriptName Example\n\nInt Property Zulu = 1 Auto\nActor Property Alpha Auto\n",
        );
        write_file(
            &dir.path().join("papyrus-lint.yaml"),
            "rules:\n  property_sorting: true\n",
        );

        let (code, _stdout, stderr) = run_captured(&[
            "fix".to_string(),
            "--line=3".to_string(),
            script_path.to_string_lossy().into_owned(),
        ]);

        assert_eq!(code, 2);
        assert!(stderr.contains("changes the file's line count"));
        assert_eq!(
            fs::read_to_string(&script_path).unwrap(),
            "ScriptName Example\n\nInt Property Zulu = 1 Auto\nActor Property Alpha Auto\n"
        );
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
    fn fix_dry_run_prints_a_diff_without_writing_the_file() {
        let dir = tempfile::tempdir().expect("failed to create temp dir");
        let script_path = dir.path().join("Example.psc");
        let original =
            "ScriptName Example   \n\nFunction DoThing()\n\tGame.GetPlayer()\nEndFunction\n";
        write_file(&script_path, original);

        let (code, stdout, stderr) = run_captured(&[
            "fix".to_string(),
            "--dry-run".to_string(),
            script_path.to_string_lossy().into_owned(),
        ]);

        assert_eq!(code, 1, "stderr: {stderr}");
        assert_eq!(
            fs::read_to_string(&script_path).unwrap(),
            original,
            "--dry-run must never write to the file"
        );
        let expected_path = script_path.to_string_lossy();
        assert!(stdout.contains(&format!("--- {expected_path}\n")));
        assert!(stdout.contains(&format!("+++ {expected_path}\n")));
        assert!(stdout.contains("-ScriptName Example   \n"));
        assert!(stdout.contains("+ScriptName Example\n"));
        assert!(stdout.contains("(1 script(s) would be fixed.)"));
        assert!(stdout.contains("Game.GetPlayer"));
    }

    #[test]
    fn fix_dry_run_prints_nothing_extra_for_an_already_clean_file() {
        let dir = tempfile::tempdir().expect("failed to create temp dir");
        let script_path = dir.path().join("Example.psc");
        write_file(&script_path, "ScriptName Example\n");

        let (code, stdout, stderr) = run_captured(&[
            "fix".to_string(),
            "--dry-run".to_string(),
            script_path.to_string_lossy().into_owned(),
        ]);

        assert_eq!(code, 0, "stderr: {stderr}");
        assert_eq!(
            fs::read_to_string(&script_path).unwrap(),
            "ScriptName Example\n"
        );
        assert!(!stdout.contains("---"));
        assert!(stdout.contains("(0 script(s) would be fixed.)"));
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
    fn fix_removes_an_unused_import_resolved_through_the_project_root() {
        let dir = tempfile::tempdir().expect("failed to create temp dir");
        write_file(
            &dir.path().join("scripts/source/Helpers.psc"),
            "ScriptName Helpers\n\nGlobal Function Assist()\nEndFunction\n",
        );
        let script_path = dir.path().join("scripts/source/Example.psc");
        write_file(
            &script_path,
            "ScriptName Example\n\nImport Helpers\n\nFunction Test()\nEndFunction\n",
        );
        write_file(
            &dir.path().join("sources.achlist"),
            r#"["scripts/source/Helpers.psc", "scripts/source/Example.psc"]"#,
        );
        let achlist_path = dir.path().join("sources.achlist");

        let (code, stdout, stderr) = run_captured(&[
            "fix".to_string(),
            achlist_path.to_string_lossy().into_owned(),
        ]);

        assert_eq!(code, 0, "stdout: {stdout}, stderr: {stderr}");
        assert_eq!(
            fs::read_to_string(&script_path).unwrap(),
            "ScriptName Example\n\n\nFunction Test()\nEndFunction\n"
        );
        assert!(stdout.contains("(1 script(s) fixed.)"));
    }

    #[test]
    fn fix_type_filter_for_unused_import_only_touches_that_rule() {
        let dir = tempfile::tempdir().expect("failed to create temp dir");
        write_file(
            &dir.path().join("scripts/source/Helpers.psc"),
            "ScriptName Helpers\n\nGlobal Function Assist()\nEndFunction\n",
        );
        let script_path = dir.path().join("scripts/source/Example.psc");
        write_file(
            &script_path,
            "ScriptName Example\n\nImport Helpers\n\nFunction Test()\n    Call(1,2)\nEndFunction\n",
        );
        write_file(
            &dir.path().join("sources.achlist"),
            r#"["scripts/source/Helpers.psc", "scripts/source/Example.psc"]"#,
        );
        let achlist_path = dir.path().join("sources.achlist");

        let (code, _stdout, stderr) = run_captured(&[
            "fix".to_string(),
            "--type=unused-import".to_string(),
            achlist_path.to_string_lossy().into_owned(),
        ]);

        assert_eq!(code, 0, "stderr: {stderr}");
        assert_eq!(
            fs::read_to_string(&script_path).unwrap(),
            "ScriptName Example\n\n\nFunction Test()\n    Call(1,2)\nEndFunction\n"
        );
    }

    #[test]
    fn fix_line_filter_errors_when_removing_an_unused_import_changes_the_line_count() {
        let dir = tempfile::tempdir().expect("failed to create temp dir");
        write_file(
            &dir.path().join("scripts/source/Helpers.psc"),
            "ScriptName Helpers\n\nGlobal Function Assist()\nEndFunction\n",
        );
        let script_path = dir.path().join("scripts/source/Example.psc");
        write_file(
            &script_path,
            "ScriptName Example\n\nImport Helpers\n\nFunction Test()\nEndFunction\n",
        );
        write_file(
            &dir.path().join("sources.achlist"),
            r#"["scripts/source/Helpers.psc", "scripts/source/Example.psc"]"#,
        );
        let achlist_path = dir.path().join("sources.achlist");

        let (code, _stdout, stderr) = run_captured(&[
            "fix".to_string(),
            "--line=3".to_string(),
            achlist_path.to_string_lossy().into_owned(),
        ]);

        assert_eq!(code, 2);
        assert!(stderr.contains("changes the file's line count"));
        assert_eq!(
            fs::read_to_string(&script_path).unwrap(),
            "ScriptName Example\n\nImport Helpers\n\nFunction Test()\nEndFunction\n"
        );
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
    fn flags_a_call_through_a_script_name_to_a_function_not_declared_global() {
        let dir = tempfile::tempdir().expect("failed to create temp dir");
        write_file(
            &dir.path().join("scripts/source/MyScriptOne.psc"),
            "ScriptName MyScriptOne Extends Form\n\nFunction IMNotStatic()\nEndFunction\n",
        );
        write_file(
            &dir.path().join("scripts/source/MyScriptTwo.psc"),
            "ScriptName MyScriptTwo Extends Form\n\nFunction Mine()\n    MyScriptOne.IMNotStatic()\nEndFunction\n",
        );
        write_file(
            &dir.path().join("sources.achlist"),
            r#"["scripts/source/MyScriptOne.psc", "scripts/source/MyScriptTwo.psc"]"#,
        );
        let achlist_path = dir.path().join("sources.achlist");

        let (code, stdout, stderr) = run_captured(&[achlist_path.to_string_lossy().into_owned()]);

        assert_eq!(code, 1, "stderr: {stderr}");
        assert!(
            stdout.contains("[non-global-function-call]"),
            "stdout: {stdout}"
        );
        assert!(stdout.contains("'IMNotStatic' is not declared Global on 'MyScriptOne'"));
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
            "ScriptName Unlisted\n\nFunction DoThing() Global\nEndFunction\n",
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
            "ScriptName Unlisted\n\nFunction DoThing() Global\nEndFunction\n",
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
    fn strict_achlist_scope_is_honored_from_an_explicit_config_path() {
        // Regression test for https://github.com/Idrinth/papyrus-lint/issues/362:
        // `--config <path>` must still pick up `strict_achlist_scope` from
        // the file it names, rather than always resolving as if it were
        // off (which made the flag appear entirely inert whenever
        // `--config` was used, and left an achlist entry's directory
        // reachable as a search root when it should not have been).
        let dir = tempfile::tempdir().expect("failed to create temp dir");
        write_file(
            &dir.path().join("mods/one/Example.psc"),
            "ScriptName Example\n\nFunction Test()\n    Unlisted.DoThing()\nEndFunction\n",
        );
        write_file(
            &dir.path().join("mods/one/Unlisted.psc"),
            "ScriptName Unlisted\n\nFunction DoThing() Global\nEndFunction\n",
        );
        write_file(
            &dir.path().join("scripts.achlist"),
            r#"["mods/one/Example.psc"]"#,
        );
        let config_path = dir.path().join("custom-config.yaml");
        write_file(&config_path, "strict_achlist_scope: true\n");

        let (code, stdout, stderr) = run_captured(&[
            "--config".to_string(),
            config_path.to_string_lossy().into_owned(),
            dir.path()
                .join("scripts.achlist")
                .to_string_lossy()
                .into_owned(),
        ]);

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
            "ScriptName Example\n; a different version\n",
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
            "ScriptName Example\n; a different version\n",
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
    fn flags_a_script_newer_than_its_compiled_pex() {
        let dir = tempfile::tempdir().expect("failed to create temp dir");
        let script_path = dir.path().join("scripts/source/Example.psc");
        let pex_path = dir.path().join("scripts/Example.pex");
        write_file(&script_path, "ScriptName Example\n");
        write_file(&pex_path, "");

        let now = std::time::SystemTime::now();
        let pex_file = fs::File::open(&pex_path).expect("failed to open pex file");
        pex_file
            .set_modified(now - std::time::Duration::from_secs(60))
            .expect("failed to set pex mtime");
        let script_file = fs::File::open(&script_path).expect("failed to open script file");
        script_file
            .set_modified(now)
            .expect("failed to set script mtime");

        let (code, stdout, stderr) = run_captured(&[script_path.to_string_lossy().into_owned()]);

        assert_eq!(code, 0, "stderr: {stderr}");
        assert!(
            stdout.contains("[stale-compiled-output]"),
            "stdout: {stdout}"
        );
        assert!(stdout.contains("[info]"), "stdout: {stdout}");
    }

    #[test]
    fn does_not_flag_a_script_older_than_its_compiled_pex() {
        let dir = tempfile::tempdir().expect("failed to create temp dir");
        let script_path = dir.path().join("scripts/source/Example.psc");
        let pex_path = dir.path().join("scripts/Example.pex");
        write_file(&script_path, "ScriptName Example\n");
        write_file(&pex_path, "");

        let now = std::time::SystemTime::now();
        let script_file = fs::File::open(&script_path).expect("failed to open script file");
        script_file
            .set_modified(now - std::time::Duration::from_secs(60))
            .expect("failed to set script mtime");
        let pex_file = fs::File::open(&pex_path).expect("failed to open pex file");
        pex_file.set_modified(now).expect("failed to set pex mtime");

        let (code, stdout, stderr) = run_captured(&[script_path.to_string_lossy().into_owned()]);

        assert_eq!(code, 0, "stderr: {stderr}");
        assert!(
            !stdout.contains("[stale-compiled-output]"),
            "stdout: {stdout}"
        );
    }

    #[test]
    fn stale_compiled_output_can_be_disabled() {
        let dir = tempfile::tempdir().expect("failed to create temp dir");
        let script_path = dir.path().join("scripts/source/Example.psc");
        let pex_path = dir.path().join("scripts/Example.pex");
        write_file(&script_path, "ScriptName Example\n");
        write_file(&pex_path, "");
        write_file(
            &dir.path().join("papyrus-lint.yaml"),
            "rules:\n  stale_compiled_output: false\n",
        );

        let now = std::time::SystemTime::now();
        let pex_file = fs::File::open(&pex_path).expect("failed to open pex file");
        pex_file
            .set_modified(now - std::time::Duration::from_secs(60))
            .expect("failed to set pex mtime");
        let script_file = fs::File::open(&script_path).expect("failed to open script file");
        script_file
            .set_modified(now)
            .expect("failed to set script mtime");

        let (code, stdout, stderr) = run_captured(&[script_path.to_string_lossy().into_owned()]);

        assert_eq!(code, 0, "stderr: {stderr}");
        assert!(
            !stdout.contains("[stale-compiled-output]"),
            "stdout: {stdout}"
        );
    }

    #[test]
    fn flags_a_script_name_that_does_not_match_its_file_name() {
        let dir = tempfile::tempdir().expect("failed to create temp dir");
        let script_path = dir.path().join("scripts/source/Other.psc");
        write_file(&script_path, "ScriptName Example\n");

        let (code, stdout, stderr) = run_captured(&[script_path.to_string_lossy().into_owned()]);

        assert_eq!(code, 1, "stderr: {stderr}");
        assert!(
            stdout.contains("[script-filename-mismatch]"),
            "stdout: {stdout}"
        );
        assert!(stdout.contains("[error]"), "stdout: {stdout}");
    }

    #[test]
    fn does_not_flag_a_script_name_matching_its_file_name() {
        let dir = tempfile::tempdir().expect("failed to create temp dir");
        let script_path = dir.path().join("scripts/source/Example.psc");
        write_file(&script_path, "ScriptName Example\n");

        let (code, stdout, stderr) = run_captured(&[script_path.to_string_lossy().into_owned()]);

        assert_eq!(code, 0, "stderr: {stderr}");
        assert!(
            !stdout.contains("[script-filename-mismatch]"),
            "stdout: {stdout}"
        );
    }

    #[test]
    fn script_filename_mismatch_can_be_disabled() {
        let dir = tempfile::tempdir().expect("failed to create temp dir");
        let script_path = dir.path().join("scripts/source/Other.psc");
        write_file(&script_path, "ScriptName Example\n");
        write_file(
            &dir.path().join("papyrus-lint.yaml"),
            "rules:\n  script_filename_mismatch: false\n",
        );

        let (code, stdout, stderr) = run_captured(&[script_path.to_string_lossy().into_owned()]);

        assert_eq!(code, 0, "stderr: {stderr}");
        assert!(
            !stdout.contains("[script-filename-mismatch]"),
            "stdout: {stdout}"
        );
    }

    #[test]
    fn script_filename_mismatch_can_be_suppressed_with_a_disable_comment() {
        let dir = tempfile::tempdir().expect("failed to create temp dir");
        let script_path = dir.path().join("scripts/source/Other.psc");
        write_file(
            &script_path,
            "ScriptName Example ; @disable script-filename-mismatch\n",
        );

        let (code, stdout, stderr) = run_captured(&[script_path.to_string_lossy().into_owned()]);

        assert_eq!(code, 0, "stderr: {stderr}");
        assert!(
            !stdout.contains("[script-filename-mismatch]"),
            "stdout: {stdout}"
        );
    }

    #[test]
    fn script_filename_mismatch_can_be_suppressed_with_a_disable_file_comment() {
        let dir = tempfile::tempdir().expect("failed to create temp dir");
        let script_path = dir.path().join("scripts/source/Other.psc");
        write_file(
            &script_path,
            "ScriptName Example\n; @disable-file script-filename-mismatch\n",
        );

        let (code, stdout, stderr) = run_captured(&[script_path.to_string_lossy().into_owned()]);

        assert_eq!(code, 0, "stderr: {stderr}");
        assert!(
            !stdout.contains("[script-filename-mismatch]"),
            "stdout: {stdout}"
        );
    }

    // Regression tests for
    // https://github.com/Idrinth/papyrus-lint/issues/772: `stale-compiled-output`,
    // `conflicting-script-versions`, and `script-filename-mismatch` are
    // computed after `papyrus_lints::lint_with_external_arguments` used to
    // run its own unused-directive validation, so an `@disable`/
    // `@disable-file` directive that correctly suppressed one of them was
    // still reported as an `unused-disable`, even though the diagnostic it
    // named was in fact suppressed.

    #[test]
    fn stale_compiled_output_disable_comment_is_not_reported_as_unused() {
        let dir = tempfile::tempdir().expect("failed to create temp dir");
        let script_path = dir.path().join("scripts/source/Example.psc");
        let pex_path = dir.path().join("scripts/Example.pex");
        write_file(
            &script_path,
            "ScriptName Example ; @disable stale-compiled-output\n",
        );
        write_file(&pex_path, "");
        write_file(
            &dir.path().join("papyrus-lint.yaml"),
            "rules:\n  unused_disable: true\n",
        );

        let now = std::time::SystemTime::now();
        let pex_file = fs::File::open(&pex_path).expect("failed to open pex file");
        pex_file
            .set_modified(now - std::time::Duration::from_secs(60))
            .expect("failed to set pex mtime");
        let script_file = fs::File::open(&script_path).expect("failed to open script file");
        script_file
            .set_modified(now)
            .expect("failed to set script mtime");

        let (code, stdout, stderr) = run_captured(&[script_path.to_string_lossy().into_owned()]);

        assert_eq!(code, 0, "stderr: {stderr}");
        assert!(
            !stdout.contains("[stale-compiled-output]"),
            "stdout: {stdout}"
        );
        assert!(!stdout.contains("[unused-disable]"), "stdout: {stdout}");
    }

    #[test]
    fn stale_compiled_output_disable_file_comment_is_not_reported_as_unused() {
        let dir = tempfile::tempdir().expect("failed to create temp dir");
        let script_path = dir.path().join("scripts/source/Example.psc");
        let pex_path = dir.path().join("scripts/Example.pex");
        write_file(
            &script_path,
            "ScriptName Example\n; @disable-file stale-compiled-output\n",
        );
        write_file(&pex_path, "");
        write_file(
            &dir.path().join("papyrus-lint.yaml"),
            "rules:\n  unused_disable: true\n",
        );

        let now = std::time::SystemTime::now();
        let pex_file = fs::File::open(&pex_path).expect("failed to open pex file");
        pex_file
            .set_modified(now - std::time::Duration::from_secs(60))
            .expect("failed to set pex mtime");
        let script_file = fs::File::open(&script_path).expect("failed to open script file");
        script_file
            .set_modified(now)
            .expect("failed to set script mtime");

        let (code, stdout, stderr) = run_captured(&[script_path.to_string_lossy().into_owned()]);

        assert_eq!(code, 0, "stderr: {stderr}");
        assert!(
            !stdout.contains("[stale-compiled-output]"),
            "stdout: {stdout}"
        );
        assert!(!stdout.contains("[unused-disable]"), "stdout: {stdout}");
    }

    #[test]
    fn conflicting_script_versions_disable_comment_is_not_reported_as_unused() {
        let dir = tempfile::tempdir().expect("failed to create temp dir");
        let script_path = dir.path().join("scripts/source/Example.psc");
        write_file(
            &script_path,
            "ScriptName Example ; @disable conflicting-script-versions\n",
        );
        write_file(
            &dir.path().join("source/scripts/Example.psc"),
            "ScriptName Example\n; a different version\n",
        );
        write_file(
            &dir.path().join("papyrus-lint.yaml"),
            "rules:\n  unused_disable: true\n",
        );

        let (code, stdout, stderr) = run_captured(&[script_path.to_string_lossy().into_owned()]);

        assert_eq!(code, 0, "stderr: {stderr}");
        assert!(
            !stdout.contains("[conflicting-script-versions]"),
            "stdout: {stdout}"
        );
        assert!(!stdout.contains("[unused-disable]"), "stdout: {stdout}");
    }

    #[test]
    fn conflicting_script_versions_disable_file_comment_is_not_reported_as_unused() {
        let dir = tempfile::tempdir().expect("failed to create temp dir");
        let script_path = dir.path().join("scripts/source/Example.psc");
        write_file(
            &script_path,
            "ScriptName Example\n; @disable-file conflicting-script-versions\n",
        );
        write_file(
            &dir.path().join("source/scripts/Example.psc"),
            "ScriptName Example\n; a different version\n",
        );
        write_file(
            &dir.path().join("papyrus-lint.yaml"),
            "rules:\n  unused_disable: true\n",
        );

        let (code, stdout, stderr) = run_captured(&[script_path.to_string_lossy().into_owned()]);

        assert_eq!(code, 0, "stderr: {stderr}");
        assert!(
            !stdout.contains("[conflicting-script-versions]"),
            "stdout: {stdout}"
        );
        assert!(!stdout.contains("[unused-disable]"), "stdout: {stdout}");
    }

    #[test]
    fn script_filename_mismatch_disable_comment_is_not_reported_as_unused() {
        let dir = tempfile::tempdir().expect("failed to create temp dir");
        let script_path = dir.path().join("scripts/source/Other.psc");
        write_file(
            &script_path,
            "ScriptName Example ; @disable script-filename-mismatch\n",
        );
        write_file(
            &dir.path().join("papyrus-lint.yaml"),
            "rules:\n  unused_disable: true\n",
        );

        let (code, stdout, stderr) = run_captured(&[script_path.to_string_lossy().into_owned()]);

        assert_eq!(code, 0, "stderr: {stderr}");
        assert!(
            !stdout.contains("[script-filename-mismatch]"),
            "stdout: {stdout}"
        );
        assert!(!stdout.contains("[unused-disable]"), "stdout: {stdout}");
    }

    #[test]
    fn script_filename_mismatch_disable_file_comment_is_not_reported_as_unused() {
        let dir = tempfile::tempdir().expect("failed to create temp dir");
        let script_path = dir.path().join("scripts/source/Other.psc");
        write_file(
            &script_path,
            "ScriptName Example\n; @disable-file script-filename-mismatch\n",
        );
        write_file(
            &dir.path().join("papyrus-lint.yaml"),
            "rules:\n  unused_disable: true\n",
        );

        let (code, stdout, stderr) = run_captured(&[script_path.to_string_lossy().into_owned()]);

        assert_eq!(code, 0, "stderr: {stderr}");
        assert!(
            !stdout.contains("[script-filename-mismatch]"),
            "stdout: {stdout}"
        );
        assert!(!stdout.contains("[unused-disable]"), "stdout: {stdout}");
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
    fn resolves_cross_script_argument_types_from_lookup_script_roots_without_linting_them() {
        let dir = tempfile::tempdir().expect("failed to create temp dir");
        let vanilla_dir = tempfile::tempdir().expect("failed to create temp dir");
        write_file(
            &vanilla_dir.path().join("Greeter.psc"),
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
                "lookup_script_roots:\n  - {}\n",
                vanilla_dir.path().display()
            ),
        );
        let achlist_path = dir.path().join("sources.achlist");

        let (code, stdout, _stderr) = run_captured(&[achlist_path.to_string_lossy().into_owned()]);

        assert_eq!(code, 1);
        assert!(stdout.contains("[argument-types]"));
        assert!(
            !stdout.contains("Greeter.psc"),
            "lookup-root scripts must not be linted"
        );
    }

    #[test]
    fn lookup_script_roots_do_not_report_conflicting_script_versions() {
        let dir = tempfile::tempdir().expect("failed to create temp dir");
        write_file(
            &dir.path().join("scripts/source/Actor.psc"),
            "ScriptName Actor\n",
        );
        let vanilla_dir = tempfile::tempdir().expect("failed to create temp dir");
        write_file(
            &vanilla_dir.path().join("Actor.psc"),
            "ScriptName Actor extends Form\n",
        );
        write_file(
            &dir.path().join("sources.achlist"),
            r#"["scripts/source/Actor.psc"]"#,
        );
        write_file(
            &dir.path().join("papyrus-lint.yaml"),
            &format!(
                "lookup_script_roots:\n  - {}\n",
                vanilla_dir.path().display()
            ),
        );
        let achlist_path = dir.path().join("sources.achlist");

        let (code, stdout, _stderr) = run_captured(&[achlist_path.to_string_lossy().into_owned()]);

        assert_eq!(code, 0);
        assert!(
            !stdout.contains("conflicting-script-versions"),
            "lookup roots must not participate in collision checks"
        );
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
    fn a_psc_path_that_is_a_directory_reports_a_read_error() {
        let dir = tempfile::tempdir().expect("failed to create temp dir");
        let misleading_path = dir.path().join("NotAFile.psc");
        fs::create_dir(&misleading_path).expect("failed to create directory");

        let (code, stdout, stderr) =
            run_captured(&[misleading_path.to_string_lossy().into_owned()]);

        assert_eq!(code, 2);
        assert!(stdout.is_empty());
        assert!(stderr.contains("error: failed to read"));
        assert!(stderr.contains("NotAFile.psc"));
    }

    #[test]
    #[cfg(unix)]
    fn compile_check_merges_compiler_reported_errors_into_the_lint_report() {
        let dir = tempfile::tempdir().expect("failed to create temp dir");
        let source_dir = dir.path().join("scripts/source");
        let script = source_dir.join("Example.psc");
        write_file(&script, "ScriptName Example\n");
        let compiler_path = write_stub_compiler(
            dir.path(),
            "#!/bin/sh\necho \"Example.psc(3,4): custom compiler error\" >&2\nexit 1\n",
        );
        write_file(
            &dir.path().join("papyrus-lint.yaml"),
            &format!(
                "compile_check: true\ncompiler_path: {}\n",
                compiler_path.display()
            ),
        );

        let (code, stdout, _stderr) =
            run_captured(&["--json".to_string(), script.to_string_lossy().into_owned()]);

        assert_eq!(code, 1);
        assert!(stdout.contains("\"rule\": \"compiler-error\""));
        assert!(stdout.contains("custom compiler error"));
    }

    #[test]
    #[cfg(unix)]
    fn compile_check_disabled_by_default_ignores_a_failing_compiler() {
        let dir = tempfile::tempdir().expect("failed to create temp dir");
        let source_dir = dir.path().join("scripts/source");
        let script = source_dir.join("Example.psc");
        write_file(&script, "ScriptName Example\n");
        let compiler_path = write_stub_compiler(
            dir.path(),
            "#!/bin/sh\necho \"Example.psc(3,4): custom compiler error\" >&2\nexit 1\n",
        );
        // No `compile_check: true`, only a configured `compiler_path` — the
        // compiler must never be invoked at all when the setting is off.
        write_file(
            &dir.path().join("papyrus-lint.yaml"),
            &format!("compiler_path: {}\n", compiler_path.display()),
        );

        let (code, stdout, _stderr) =
            run_captured(&["--json".to_string(), script.to_string_lossy().into_owned()]);

        assert_eq!(code, 0);
        assert!(!stdout.contains("compiler-error"));
    }

    #[test]
    #[cfg(unix)]
    fn compile_check_is_ignored_when_no_compiler_path_can_be_resolved() {
        let dir = tempfile::tempdir().expect("failed to create temp dir");
        let source_dir = dir.path().join("scripts/source");
        let script = source_dir.join("Example.psc");
        write_file(&script, "ScriptName Example\n");
        write_file(
            &dir.path().join("papyrus-lint.yaml"),
            "compile_check: true\n",
        );

        let (code, stdout, _stderr) =
            run_captured(&["--json".to_string(), script.to_string_lossy().into_owned()]);

        assert_eq!(code, 0);
        assert!(!stdout.contains("compiler-error"));
    }

    #[test]
    fn direct_psc_detection_is_case_insensitive() {
        let dir = tempfile::tempdir().expect("failed to create temp dir");
        let script = dir.path().join("Example.PSC");
        write_file(&script, "ScriptName Example   \n");

        let (code, stdout, stderr) = run_captured(&[script.to_string_lossy().into_owned()]);

        assert_eq!(code, 0, "stderr: {stderr}");
        assert!(stderr.is_empty());
        assert!(stdout.contains("[trailing-whitespace]"));
        assert!(stdout.contains("1 problem(s) found in 1 of 1 script(s)"));
    }
}
