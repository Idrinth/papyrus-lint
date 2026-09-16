//! On-disk representation of a cache entry: where it lives, how it's
//! addressed, and the raw read/write of it. [`crate::ops`] builds the
//! actual `get`/`put` semantics on top of these primitives.

use std::path::{Path, PathBuf};
use std::time::UNIX_EPOCH;

use serde::{Deserialize, Serialize};

use crate::version::is_compatible_version;

const CACHE_DIR_NAME: &str = "ast-cache";

#[derive(Serialize, Deserialize)]
pub(crate) struct CacheEntry {
    pub(crate) modified_unix_secs: u64,
    pub(crate) content_md5: String,
    pub(crate) linter_version: String,
    pub(crate) ast: Option<papyrus_parser::ast::Script>,
    #[serde(default)]
    pub(crate) tokens: Option<Vec<papyrus_parser::token::Token>>,
}

/// The `ast-cache` directory alongside the running executable (the app's
/// install directory), or `None` if the executable's own path can't be
/// determined.
pub(crate) fn cache_dir() -> Option<PathBuf> {
    let exe = std::env::current_exe().ok()?;
    Some(exe.parent()?.join(CACHE_DIR_NAME))
}

/// The cache file `source_path` is stored under within `dir`: an MD5 of its
/// absolute path, so path separators and length can't collide with
/// filesystem naming limits.
pub(crate) fn cache_file_path(dir: &Path, source_path: &Path) -> PathBuf {
    let digest = md5::compute(source_path.to_string_lossy().as_bytes());
    dir.join(format!("{digest:x}.json"))
}

pub(crate) fn file_modified_unix_secs(source_path: &Path) -> Option<u64> {
    let modified = std::fs::metadata(source_path).ok()?.modified().ok()?;
    Some(modified.duration_since(UNIX_EPOCH).ok()?.as_secs())
}

/// Reads back the cache entry for `source_path`/`source`, if one exists and
/// is still fresh (matching content/mtime and at or above
/// [`crate::version::MIN_COMPATIBLE_VERSION`]). Shared by the `ast` and
/// `tokens` accessors in [`crate::ops`], and by each one's `put` so that
/// writing one field preserves whatever still-valid value the other field
/// already held.
pub(crate) fn valid_entry_in(dir: &Path, source_path: &Path, source: &str) -> Option<CacheEntry> {
    let raw = std::fs::read(cache_file_path(dir, source_path)).ok()?;
    let entry: CacheEntry = serde_json::from_slice(&raw).ok()?;

    if !is_compatible_version(&entry.linter_version)
        || entry.modified_unix_secs != file_modified_unix_secs(source_path)?
        || entry.content_md5 != format!("{:x}", md5::compute(source.as_bytes()))
    {
        return None;
    }

    Some(entry)
}

pub(crate) fn write_entry_in(dir: &Path, source_path: &Path, entry: &CacheEntry) {
    let Ok(serialized) = serde_json::to_vec(entry) else {
        return;
    };
    if std::fs::create_dir_all(dir).is_err() {
        return;
    }
    let _ = std::fs::write(cache_file_path(dir, source_path), serialized);
}
