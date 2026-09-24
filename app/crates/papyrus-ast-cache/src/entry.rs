//! On-disk representation of a cache entry: where it lives, how it's
//! addressed, and the raw read/write of it. [`crate::ops`] builds the
//! actual `get`/`put` semantics on top of these primitives.

use std::path::{Path, PathBuf};
use std::time::UNIX_EPOCH;

use papyrus_lint_globals::Game;
use serde::{Deserialize, Serialize};

use crate::version::is_compatible_version;

const CACHE_DIR_NAME: &str = "ast-cache";
const CACHE_DIR_ENV: &str = "PAPYRUS_LINT_AST_CACHE_DIR";

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
/// `{game}-{md5}.json`, an MD5 of the path string so separators and length
/// can't collide with filesystem naming limits. There is no game-less
/// filename; every caller has a target game.
pub(crate) fn cache_file_path_for_game(dir: &Path, game: Game, source_path: &Path) -> PathBuf {
    let digest = md5::compute(source_path.to_string_lossy().as_bytes());
    dir.join(format!("{}-{digest:x}.json", game.as_str()))
}

pub(crate) fn file_modified_unix_secs(source_path: &Path) -> Option<u64> {
    let modified = std::fs::metadata(source_path).ok()?.modified().ok()?;
    Some(modified.duration_since(UNIX_EPOCH).ok()?.as_secs())
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
    let raw = std::fs::read(cache_file_path_for_game(dir, game, source_path)).ok()?;
    deserialize_fresh_entry(raw, source_path, source)
}

fn deserialize_fresh_entry(raw: Vec<u8>, source_path: &Path, source: &str) -> Option<CacheEntry> {
    let entry: CacheEntry = serde_json::from_slice(&raw).ok()?;

    if !is_compatible_version(&entry.linter_version)
        || entry.modified_unix_secs != file_modified_unix_secs(source_path)?
        || entry.content_md5 != format!("{:x}", md5::compute(source.as_bytes()))
    {
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
    let Ok(serialized) = serde_json::to_vec(entry) else {
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
