//! On-disk representation of a cache entry: where it lives, how it's
//! addressed, and the raw read/write of it. [`crate::ops`] builds the
//! actual `get`/`put` semantics on top of these primitives.
//!
//! Each file is an internal binary document (`.iplatc`): a 4-byte magic,
//! a little-endian format version, and a gzip-compressed bincode payload
//! of [`CacheEntry`]. The layout is not a public interchange format.

use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::time::UNIX_EPOCH;

use flate2::read::GzDecoder;
use flate2::write::GzEncoder;
use flate2::Compression;
use papyrus_lint_globals::Game;
use serde::{Deserialize, Serialize};

use crate::version::is_compatible_version;

const CACHE_DIR_NAME: &str = "ast-cache";
const CACHE_DIR_ENV: &str = "PAPYRUS_LINT_AST_CACHE_DIR";
const CACHE_FILE_EXT: &str = "iplatc";
const MAGIC: &[u8; 4] = b"IPLA";
const FORMAT_VERSION: u32 = 1;

#[derive(Serialize, Deserialize)]
pub(crate) struct CacheEntry {
    pub(crate) modified_unix_secs: u64,
    pub(crate) content_md5: String,
    pub(crate) linter_version: String,
    pub(crate) ast: Option<papyrus_parser::ast::Script>,
    #[serde(default)]
    pub(crate) tokens: Option<Vec<papyrus_parser::token::Token>>,
}

/// The directory selected by `PAPYRUS_LINT_AST_CACHE_DIR`, when set, or the
/// `ast-cache` directory alongside the running executable (the app's install
/// directory). Returns `None` only when neither location can be determined.
pub(crate) fn cache_dir() -> Option<PathBuf> {
    cache_dir_from(
        std::env::var_os(CACHE_DIR_ENV),
        std::env::current_exe().ok(),
    )
}

fn cache_dir_from(
    override_dir: Option<std::ffi::OsString>,
    exe: Option<PathBuf>,
) -> Option<PathBuf> {
    if let Some(dir) = override_dir.filter(|dir| !dir.is_empty()) {
        return Some(PathBuf::from(dir));
    }
    Some(exe?.parent()?.join(CACHE_DIR_NAME))
}

/// The cache file `source_path` is stored under within `dir` for `game`:
/// `{game}-{md5}.iplatc`, an MD5 of the path string so separators and length
/// can't collide with filesystem naming limits. There is no game-less
/// filename; every caller has a target game.
pub(crate) fn cache_file_path_for_game(dir: &Path, game: Game, source_path: &Path) -> PathBuf {
    let digest = md5::compute(source_path.to_string_lossy().as_bytes());
    dir.join(format!("{}-{digest:x}.{CACHE_FILE_EXT}", game.as_str()))
}

pub(crate) fn file_modified_unix_secs(source_path: &Path) -> Option<u64> {
    let modified = std::fs::metadata(source_path).ok()?.modified().ok()?;
    Some(modified.duration_since(UNIX_EPOCH).ok()?.as_secs())
}

pub(crate) fn encode_entry(entry: &CacheEntry) -> Option<Vec<u8>> {
    let payload = bincode::serialize(entry).ok()?;
    let mut compressed = Vec::new();
    {
        let mut encoder = GzEncoder::new(&mut compressed, Compression::fast());
        encoder.write_all(&payload).ok()?;
        encoder.finish().ok()?;
    }
    let mut out = Vec::with_capacity(8 + compressed.len());
    out.extend_from_slice(MAGIC);
    out.extend_from_slice(&FORMAT_VERSION.to_le_bytes());
    out.extend_from_slice(&compressed);
    Some(out)
}

pub(crate) fn decode_entry(raw: &[u8]) -> Option<CacheEntry> {
    if raw.len() < 8 || raw[..4] != *MAGIC {
        return None;
    }
    let version = u32::from_le_bytes(raw[4..8].try_into().ok()?);
    if version != FORMAT_VERSION {
        return None;
    }
    let mut decoder = GzDecoder::new(&raw[8..]);
    let mut payload = Vec::new();
    decoder.read_to_end(&mut payload).ok()?;
    bincode::deserialize(&payload).ok()
}

/// Reads back the cache entry for `game`/`source_path` when the stored
/// mtime and linter version still match, without needing the source text.
/// Used to recover [`CacheEntry::content_md5`] so callers such as
/// `conflicting-script-versions` can compare scripts without opening the
/// `.psc` again.
pub(crate) fn mtime_valid_entry_in_for_game(
    dir: &Path,
    game: Game,
    source_path: &Path,
) -> Option<CacheEntry> {
    let raw = std::fs::read(cache_file_path_for_game(dir, game, source_path)).ok()?;
    let entry = decode_entry(&raw)?;
    if !is_compatible_version(&entry.linter_version)
        || entry.modified_unix_secs != file_modified_unix_secs(source_path)?
    {
        return None;
    }
    Some(entry)
}

/// Reads back the cache entry for `game`/`source_path`/`source`, if one
/// exists and is still fresh (matching content/mtime and at or above
/// [`crate::version::MIN_COMPATIBLE_VERSION`]). Shared by the `ast` and
/// `tokens` accessors in [`crate::ops`], and by each one's `put` so that
/// writing one field preserves whatever still-valid value the other field
/// already held.
pub(crate) fn valid_entry_in_for_game(
    dir: &Path,
    game: Game,
    source_path: &Path,
    source: &str,
) -> Option<CacheEntry> {
    let entry = mtime_valid_entry_in_for_game(dir, game, source_path)?;
    if entry.content_md5 != format!("{:x}", md5::compute(source.as_bytes())) {
        return None;
    }
    Some(entry)
}

pub(crate) fn write_entry_in_for_game(
    dir: &Path,
    game: Game,
    source_path: &Path,
    entry: &CacheEntry,
) {
    let Some(serialized) = encode_entry(entry) else {
        return;
    };
    if std::fs::create_dir_all(dir).is_err() {
        return;
    }
    let _ = std::fs::write(cache_file_path_for_game(dir, game, source_path), serialized);
}

#[cfg(test)]
#[path = "entry_tests.rs"]
mod tests;
