//! Content-addressed AST/token cache of the Skyrim and SKSE scripts in
//! `shared/scripts/skyrim-scripts.zip` and `shared/scripts/skyrim-extender-scripts.zip`,
//! compiled into the binary by `build.rs`.
//!
//! Lookups are keyed by an MD5 of the decoded source text *or* by the
//! script's lowercased `ScriptName`. An MD5 hit means a known `Actor.psc`
//! (or `SKSE.psc`, …) matches regardless of extract path or mtime — the
//! user's Skyrim install, Docker's unpacked zip, and `--script-root` copies
//! of the same bytes all share one entry. A name hit is the fallback when
//! no matching `.psc` is on disk at all, so `FunctionTable` can still walk
//! `Actor extends ObjectReference extends Form` without game data. A
//! modified copy (SKSE patch, user edit) has a different digest and falls
//! through to the on-disk cache / a fresh parse for the MD5 path; a
//! project file of the same name still wins over the name index.
//!
//! The blob is gzip-compressed `include_bytes!` data generated at build
//! time. First lookup decompresses it once into a process-wide buffer
//! and builds an MD5 → offset and name → offset index; later lookups
//! deserialize just the requested script. There is no lock on this path:
//! the data is read-only after init, so parallel lint workers resolving
//! the same base type (`Actor`, `ObjectReference`, `Form`, …) do not
//! serialize on the disk cache's process-wide lock the way two disk-cache
//! writers would.

use std::collections::HashMap;
use std::io::Read;
use std::sync::OnceLock;

use flate2::read::GzDecoder;

use crate::bundled_blob::{self, IndexEntry};

static COMPRESSED: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/skyrim-ast-cache.bin.gz"));

struct BundledCache {
    index: HashMap<[u8; 16], IndexEntry>,
    by_name: HashMap<String, IndexEntry>,
    raw: Vec<u8>,
    payload_start: usize,
}

impl BundledCache {
    fn payload(&self) -> &[u8] {
        self.raw.get(self.payload_start..).unwrap_or(&[])
    }
}

fn cache() -> Option<&'static BundledCache> {
    static CACHE: OnceLock<Option<BundledCache>> = OnceLock::new();
    CACHE
        .get_or_init(|| {
            let mut decoder = GzDecoder::new(COMPRESSED);
            let mut raw = Vec::new();
            decoder.read_to_end(&mut raw).ok()?;
            let parsed = bundled_blob::parse_blob(&raw)?;
            Some(BundledCache {
                index: parsed.by_md5,
                by_name: parsed.by_name,
                raw,
                payload_start: parsed.payload_start,
            })
        })
        .as_ref()
}

fn lookup(source: &str) -> Option<(&'static BundledCache, IndexEntry)> {
    let digest = md5::compute(source.as_bytes());
    let cache = cache()?;
    let entry = *cache.index.get(&digest.0)?;
    Some((cache, entry))
}

fn lookup_name(name: &str) -> Option<(&'static BundledCache, IndexEntry)> {
    let cache = cache()?;
    let entry = *cache.by_name.get(&name.to_ascii_lowercase())?;
    Some((cache, entry))
}

/// Cached AST for `source` when it matches a bundled script.
/// Also primes `papyrus_parser`'s in-memory memoization, matching
/// [`crate::ops::get_in`].
pub(crate) fn ast_for(source: &str) -> Option<papyrus_parser::ast::Script> {
    let (cache, entry) = lookup(source)?;
    let ast = bundled_blob::decode_ast(cache.payload(), &entry)?;
    papyrus_parser::prime_cache(source, ast.clone());
    Some(ast)
}

/// Cached tokens for `source` when it matches a bundled script.
/// Also primes `papyrus_parser`'s in-memory memoization, matching
/// [`crate::ops::get_tokens_in`].
pub(crate) fn tokens_for(source: &str) -> Option<Vec<papyrus_parser::token::Token>> {
    let (cache, entry) = lookup(source)?;
    let tokens = bundled_blob::decode_tokens(cache.payload(), &entry)?;
    papyrus_parser::prime_tokenize_cache(source, tokens.clone());
    Some(tokens)
}

/// Cached AST for a bundled script looked up by `ScriptName`, matched
/// case-insensitively. Used when no matching `.psc` is on disk.
pub(crate) fn ast_for_name(name: &str) -> Option<papyrus_parser::ast::Script> {
    let (cache, entry) = lookup_name(name)?;
    bundled_blob::decode_ast(cache.payload(), &entry)
}

/// Cached tokens for a bundled script looked up by `ScriptName`.
#[cfg(test)]
pub(crate) fn tokens_for_name(name: &str) -> Option<Vec<papyrus_parser::token::Token>> {
    let (cache, entry) = lookup_name(name)?;
    bundled_blob::decode_tokens(cache.payload(), &entry)
}

/// Whether the bundled cache has a script whose `ScriptName` matches
/// `name` (case-insensitive). Does not deserialize the AST.
pub(crate) fn contains_name(name: &str) -> bool {
    cache()
        .map(|cache| cache.by_name.contains_key(&name.to_ascii_lowercase()))
        .unwrap_or(false)
}

/// Primes both in-memory caches from the bundled blob when `source` is a
/// known bundled script. Returns `true` when both an AST and a token
/// stream were present, so [`crate::ensure_primed`] can skip the disk
/// cache (and its process-wide lock) entirely.
pub(crate) fn prime(source: &str) -> bool {
    let Some((cache, entry)) = lookup(source) else {
        return false;
    };
    let Some(ast) = bundled_blob::decode_ast(cache.payload(), &entry) else {
        return false;
    };
    let Some(tokens) = bundled_blob::decode_tokens(cache.payload(), &entry) else {
        return false;
    };
    papyrus_parser::prime_cache(source, ast);
    papyrus_parser::prime_tokenize_cache(source, tokens);
    true
}

#[cfg(test)]
pub(crate) fn entry_count() -> usize {
    cache().map(|cache| cache.index.len()).unwrap_or(0)
}

#[cfg(test)]
#[path = "bundled_tests.rs"]
mod tests;
