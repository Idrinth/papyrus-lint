//! Disk-backed cache of parsed `.psc` ASTs, shared by the desktop app and
//! the CLI, so reopening an unchanged script (e.g. switching between files
//! in the code viewer, relinting an achlist, or resolving the same
//! cross-script lookup across separate CLI invocations) skips re-parsing
//! it. Entries live as one JSON file per source path in the directory named
//! by `PAPYRUS_LINT_AST_CACHE_DIR`, or, when that isn't set, an `ast-cache`
//! directory next to the running executable -- the desktop app's own binary,
//! or `PapyrusLinterCLI`'s, whichever process is doing the parsing -- and are
//! invalidated by the source file's last-modified timestamp, an MD5 of its
//! content, and the linter version that wrote the entry -- if any of the
//! three is no longer valid, it's treated as a miss and the caller re-parses.
//!
//! Vanilla Skyrim and SKSE scripts shipped in `shared/skyrim-scripts.zip` and
//! `shared/skyrim-extender-scripts.zip` are also compiled into the binary as
//! a content-addressed AST/token blob (see [`bundled`]). [`get`]/
//! [`get_tokens`]/[`ensure_primed`] consult that blob first, keyed only by an
//! MD5 of the decoded source, so a stock `Actor.psc` or `SKSE.psc` hits on the
//! first analysis even when the file was just extracted to a new path
//! (Docker, `--script-root`, the user's Skyrim install). A bundled hit does
//! not take the disk-cache lock below, so parallel lint workers resolving the
//! same base type do not serialize on each other for that lookup. A modified
//! copy of a bundled script has a different digest and falls through to the
//! on-disk cache / a fresh parse as before.
//!
//! The version check is a minimum-compatible-version check against
//! [`version::MIN_COMPATIBLE_VERSION`], not an exact match against the
//! running linter's own version: an entry written by any release at or
//! after `MIN_COMPATIBLE_VERSION` is accepted, so an ordinary app update
//! doesn't discard an otherwise still-valid cache. Bump
//! `MIN_COMPATIBLE_VERSION` only when a release actually changes the
//! on-disk `CacheEntry` layout or the `papyrus_parser::ast::Script` shape
//! it embeds in a way that would break reading older entries. The bundled
//! blob is rebuilt by `build.rs` from the zip whenever this crate (or
//! `papyrus-parser`) rebuilds, so it does not participate in that floor.
//!
//! Caching is a pure optimization: any I/O or (de)serialization failure
//! here is swallowed and simply falls through to a fresh parse, never
//! surfaced as a lint error.
//!
//! Every public accessor below serializes on a single process-wide
//! [`CACHE_LOCK`] when it has to touch the on-disk cache, since two scripts
//! linted at once (the CLI's `papyrus_lint_core::parallel`-based worker
//! pool, or the desktop app's own already-concurrent per-file Tauri
//! commands) can both resolve the same cross-script dependency at the same
//! moment, and [`std::fs::write`] isn't atomic: two unsynchronized writers
//! to the very same cache file could interleave into invalid JSON. A
//! corrupt read already falls back to a fresh parse (see above), so that
//! alone was never unsound, but it did mean a hot shared script (e.g. a
//! common base class) could pay for a redundant reparse on every such
//! collision. Holding the lock across an entire disk-cache accessor call —
//! including its own file I/O and (de)serialization, but never the
//! caller's actual parse/lint work, and never a bundled-cache hit — keeps
//! that cost to the disk access itself rather than serializing the
//! CPU-bound work around it.
//!
//! Each entry also carries the lexer's token stream
//! (`papyrus_parser::tokenize()`'s output) alongside the AST, via
//! [`get_tokens`]/[`put_tokens`], sharing the same freshness metadata as
//! the AST accessors -- a `put`/`put_tokens` call preserves whatever
//! still-valid value the other field already held instead of clobbering it.
//! `put_tokens` is also called directly (alongside `put`) wherever
//! `parse_psc_file` or `FunctionTable::ensure_loaded` freshly parse a
//! script, independent of the priming path below.
//!
//! Since `papyrus_lints::lint()`/`repair()` parse/tokenize their `source`
//! argument internally and never see `source_path`, they can't consult this
//! cache directly. `get`/`get_tokens` close that gap as a side effect of a
//! hit: each also primes `papyrus_parser`'s own in-memory memoization (see
//! `papyrus_parser::prime_cache`/`prime_tokenize_cache`) with the same
//! value, so anything that parses/tokenizes that exact source text later in
//! the same process reuses it instead of redoing the work. [`ensure_primed`]
//! wraps both accessors for the "about to lint/repair a script whose disk
//! cache might already be current" case: a hit for either primes the
//! matching in-memory cache as above; a miss for either parses/tokenizes
//! `source` once itself and writes a fresh disk entry for next time.
//!
//! The implementation is split across a few modules: [`entry`] is the raw
//! on-disk representation (paths, freshness metadata, read/write),
//! [`version`] is the compatibility check against
//! [`version::MIN_COMPATIBLE_VERSION`], [`bundled`] is the content-addressed
//! bundled-script blob, and [`ops`] is the `get`/`put`/`ensure_primed`
//! logic built on top of the on-disk primitives, parameterized over a
//! cache directory so it can be tested without touching the real one. This
//! file wraps [`ops`] with [`CACHE_LOCK`] and the real cache directory to
//! form the crate's public API, consulting [`bundled`] first.

use std::path::Path;
use std::sync::Mutex;

mod bundled;
mod bundled_blob;
mod entry;
mod ops;
mod version;

/// Guards every public on-disk accessor below against concurrent access
/// from multiple lint workers at once — see the module docs above for why
/// this is needed despite each entry living in its own file. Bundled-cache
/// hits never take this lock.
static CACHE_LOCK: Mutex<()> = Mutex::new(());

/// Returns the cached AST for `source_path` if the bundled-script cache knows
/// `source`, or if the on-disk cache has a still-valid entry
/// for `source`'s current content, `source_path`'s modification time, and
/// a linter version at or above [`version::MIN_COMPATIBLE_VERSION`].
/// Returns `None` on any cache miss, mismatch, or error -- the caller
/// should parse `source` fresh in that case. See [`ops::get_in`] for the
/// in-memory priming a disk hit also does (a bundled hit primes the same
/// way).
pub fn get(source_path: &Path, source: &str) -> Option<papyrus_parser::ast::Script> {
    if let Some(ast) = bundled::ast_for(source) {
        return Some(ast);
    }
    let _guard = CACHE_LOCK
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    ops::get_in(&entry::cache_dir()?, source_path, source)
}

/// Persists `ast`, parsed from `source_path`/`source`, to the on-disk cache
/// for later [`get`] calls. Any failure (e.g. an unwritable install
/// directory) is silently ignored.
pub fn put(source_path: &Path, source: &str, ast: &papyrus_parser::ast::Script) {
    let _guard = CACHE_LOCK
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    if let Some(dir) = entry::cache_dir() {
        ops::put_in(&dir, source_path, source, ast, version::stamped_version());
    }
}

/// Returns the cached tokens for `source_path` if the bundled-script cache
/// knows `source`, or if the on-disk cache has a still-valid entry for
/// `source`'s current content, `source_path`'s modification time, and a linter
/// version at or above
/// [`version::MIN_COMPATIBLE_VERSION`]. Returns `None` on any cache miss,
/// mismatch, or error -- the caller should tokenize `source` fresh in that
/// case. See [`ops::get_tokens_in`] for the in-memory priming a disk hit
/// also does (a bundled hit primes the same way).
pub fn get_tokens(source_path: &Path, source: &str) -> Option<Vec<papyrus_parser::token::Token>> {
    if let Some(tokens) = bundled::tokens_for(source) {
        return Some(tokens);
    }
    let _guard = CACHE_LOCK
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    ops::get_tokens_in(&entry::cache_dir()?, source_path, source)
}

/// Persists `tokens`, lexed from `source_path`/`source`, to the on-disk
/// cache for later [`get_tokens`] calls. Any failure (e.g. an unwritable
/// install directory) is silently ignored.
pub fn put_tokens(source_path: &Path, source: &str, tokens: &[papyrus_parser::token::Token]) {
    let _guard = CACHE_LOCK
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    if let Some(dir) = entry::cache_dir() {
        ops::put_tokens_in(
            &dir,
            source_path,
            source,
            tokens,
            version::stamped_version(),
        );
    }
}

/// Makes sure `papyrus_parser`'s in-memory memoization has both an AST and
/// a token stream ready for `source` before something that parses/
/// tokenizes `source` itself -- typically `papyrus_lints::lint()`/
/// `repair()`, called with only the raw source text, never `source_path` --
/// runs. A bundled-script hit primes both without touching the disk cache (or
/// its lock). A disk cache hit for either already primes
/// the matching in-memory cache as a side effect; a miss for either
/// parses/tokenizes `source` once here instead (which populates the
/// in-memory cache the same way a hit would) and writes the result to the
/// disk cache for next time. Used by the desktop app's `lint_psc_file`/
/// `repair_psc_file` commands and the CLI's own per-script lint loop, so
/// relinting an unchanged script -- across separate desktop app commands
/// or CLI invocations -- skips both re-parsing and re-tokenizing it there
/// too, not just in `get`/`get_tokens`'s other existing callers. See
/// [`ops::ensure_primed_in`].
pub fn ensure_primed(source_path: &Path, source: &str) {
    if bundled::prime(source) {
        return;
    }
    let _guard = CACHE_LOCK
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    let Some(dir) = entry::cache_dir() else {
        return;
    };
    ops::ensure_primed_in(&dir, source_path, source, version::stamped_version());
}

#[cfg(test)]
#[path = "lib_tests.rs"]
mod tests;
