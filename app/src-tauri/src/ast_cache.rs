//! Disk-backed cache of parsed `.psc` ASTs for the desktop app, so reopening
//! an unchanged script (e.g. switching between files in the code viewer, or
//! relinting an achlist) skips re-parsing it. Entries live as one JSON file
//! per source path in an `ast-cache` directory next to the app's own
//! executable, and are invalidated by the source file's last-modified
//! timestamp, an MD5 of its content, and the running linter version -- if
//! any of the three has changed since the entry was written, it's treated
//! as a miss and the caller re-parses.
//!
//! Caching is a pure optimization: any I/O or (de)serialization failure
//! here is swallowed and simply falls through to a fresh parse, never
//! surfaced as a lint error.

use std::path::{Path, PathBuf};
use std::time::UNIX_EPOCH;

use serde::{Deserialize, Serialize};

const CACHE_DIR_NAME: &str = "ast-cache";

#[derive(Serialize, Deserialize)]
struct CacheEntry {
    modified_unix_secs: u64,
    content_md5: String,
    linter_version: String,
    ast: papyrus_parser::ast::Script,
}

/// The `ast-cache` directory alongside the running executable (the app's
/// install directory), or `None` if the executable's own path can't be
/// determined.
fn cache_dir() -> Option<PathBuf> {
    let exe = std::env::current_exe().ok()?;
    Some(exe.parent()?.join(CACHE_DIR_NAME))
}

/// The cache file `source_path` is stored under within `dir`: an MD5 of its
/// absolute path, so path separators and length can't collide with
/// filesystem naming limits.
fn cache_file_path(dir: &Path, source_path: &Path) -> PathBuf {
    let digest = md5::compute(source_path.to_string_lossy().as_bytes());
    dir.join(format!("{digest:x}.json"))
}

fn file_modified_unix_secs(source_path: &Path) -> Option<u64> {
    let modified = std::fs::metadata(source_path).ok()?.modified().ok()?;
    Some(modified.duration_since(UNIX_EPOCH).ok()?.as_secs())
}

fn get_in(dir: &Path, source_path: &Path, source: &str) -> Option<papyrus_parser::ast::Script> {
    let raw = std::fs::read(cache_file_path(dir, source_path)).ok()?;
    let entry: CacheEntry = serde_json::from_slice(&raw).ok()?;

    if entry.linter_version != env!("CARGO_PKG_VERSION")
        || entry.modified_unix_secs != file_modified_unix_secs(source_path)?
        || entry.content_md5 != format!("{:x}", md5::compute(source.as_bytes()))
    {
        return None;
    }

    Some(entry.ast)
}

fn put_in(dir: &Path, source_path: &Path, source: &str, ast: &papyrus_parser::ast::Script) {
    let Some(modified_unix_secs) = file_modified_unix_secs(source_path) else {
        return;
    };
    let entry = CacheEntry {
        modified_unix_secs,
        content_md5: format!("{:x}", md5::compute(source.as_bytes())),
        linter_version: env!("CARGO_PKG_VERSION").to_string(),
        ast: ast.clone(),
    };
    let Ok(serialized) = serde_json::to_vec(&entry) else {
        return;
    };
    if std::fs::create_dir_all(dir).is_err() {
        return;
    }
    let _ = std::fs::write(cache_file_path(dir, source_path), serialized);
}

/// Returns the cached AST for `source_path` if the on-disk cache has a
/// still-valid entry for `source`'s current content, `source_path`'s
/// modification time, and the running linter version. Returns `None` on any
/// cache miss, mismatch, or error -- the caller should parse `source` fresh
/// in that case.
pub fn get(source_path: &Path, source: &str) -> Option<papyrus_parser::ast::Script> {
    get_in(&cache_dir()?, source_path, source)
}

/// Persists `ast`, parsed from `source_path`/`source`, to the on-disk cache
/// for later [`get`] calls. Any failure (e.g. an unwritable install
/// directory) is silently ignored.
pub fn put(source_path: &Path, source: &str, ast: &papyrus_parser::ast::Script) {
    if let Some(dir) = cache_dir() {
        put_in(&dir, source_path, source, ast);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    fn sample_ast() -> papyrus_parser::ast::Script {
        papyrus_parser::parse("ScriptName Example\n").unwrap()
    }

    #[test]
    fn put_then_get_returns_the_cached_ast_when_nothing_changed() {
        let cache_dir = tempdir().unwrap();
        let project_dir = tempdir().unwrap();
        let source_path = project_dir.path().join("Example.psc");
        let source = "ScriptName Example\n";
        std::fs::write(&source_path, source).unwrap();

        let ast = sample_ast();
        put_in(cache_dir.path(), &source_path, source, &ast);

        assert_eq!(get_in(cache_dir.path(), &source_path, source), Some(ast));
    }

    #[test]
    fn get_is_a_miss_for_an_uncached_path() {
        let cache_dir = tempdir().unwrap();
        let project_dir = tempdir().unwrap();
        let source_path = project_dir.path().join("Example.psc");
        std::fs::write(&source_path, "ScriptName Example\n").unwrap();

        assert_eq!(
            get_in(cache_dir.path(), &source_path, "ScriptName Example\n"),
            None
        );
    }

    #[test]
    fn get_is_a_miss_when_the_content_changed_even_if_the_mtime_did_not() {
        let cache_dir = tempdir().unwrap();
        let project_dir = tempdir().unwrap();
        let source_path = project_dir.path().join("Example.psc");
        let original = "ScriptName Example\n";
        std::fs::write(&source_path, original).unwrap();

        put_in(cache_dir.path(), &source_path, original, &sample_ast());

        let changed = "ScriptName Renamed\n";
        assert_eq!(get_in(cache_dir.path(), &source_path, changed), None);
    }

    #[test]
    fn get_is_a_miss_when_the_file_was_modified_after_caching() {
        let cache_dir = tempdir().unwrap();
        let project_dir = tempdir().unwrap();
        let source_path = project_dir.path().join("Example.psc");
        let source = "ScriptName Example\n";
        std::fs::write(&source_path, source).unwrap();

        put_in(cache_dir.path(), &source_path, source, &sample_ast());

        let filetime_now = std::time::SystemTime::now() + std::time::Duration::from_secs(120);
        std::fs::write(&source_path, source).unwrap();
        let file = std::fs::File::open(&source_path).unwrap();
        file.set_modified(filetime_now).unwrap();

        assert_eq!(get_in(cache_dir.path(), &source_path, source), None);
    }

    #[test]
    fn get_is_a_miss_when_the_cached_linter_version_differs() {
        let cache_dir = tempdir().unwrap();
        let project_dir = tempdir().unwrap();
        let source_path = project_dir.path().join("Example.psc");
        let source = "ScriptName Example\n";
        std::fs::write(&source_path, source).unwrap();

        let entry = CacheEntry {
            modified_unix_secs: file_modified_unix_secs(&source_path).unwrap(),
            content_md5: format!("{:x}", md5::compute(source.as_bytes())),
            linter_version: "0.0.0-not-the-running-version".to_string(),
            ast: sample_ast(),
        };
        std::fs::create_dir_all(cache_dir.path()).unwrap();
        std::fs::write(
            cache_file_path(cache_dir.path(), &source_path),
            serde_json::to_vec(&entry).unwrap(),
        )
        .unwrap();

        assert_eq!(get_in(cache_dir.path(), &source_path, source), None);
    }

    #[test]
    fn get_is_a_miss_on_malformed_cache_contents() {
        let cache_dir = tempdir().unwrap();
        let project_dir = tempdir().unwrap();
        let source_path = project_dir.path().join("Example.psc");
        std::fs::write(&source_path, "ScriptName Example\n").unwrap();

        std::fs::create_dir_all(cache_dir.path()).unwrap();
        std::fs::write(cache_file_path(cache_dir.path(), &source_path), b"not json").unwrap();

        assert_eq!(
            get_in(cache_dir.path(), &source_path, "ScriptName Example\n"),
            None
        );
    }

    #[test]
    fn put_creates_the_cache_directory_if_missing() {
        let cache_dir = tempdir().unwrap();
        let nested_cache_dir = cache_dir.path().join("nested");
        let project_dir = tempdir().unwrap();
        let source_path = project_dir.path().join("Example.psc");
        let source = "ScriptName Example\n";
        std::fs::write(&source_path, source).unwrap();

        put_in(&nested_cache_dir, &source_path, source, &sample_ast());

        assert!(nested_cache_dir.is_dir());
        assert!(get_in(&nested_cache_dir, &source_path, source).is_some());
    }

    #[test]
    fn different_source_paths_do_not_collide_in_the_cache() {
        let cache_dir = tempdir().unwrap();
        let project_dir = tempdir().unwrap();
        let path_a = project_dir.path().join("A.psc");
        let path_b = project_dir.path().join("B.psc");
        std::fs::write(&path_a, "ScriptName A\n").unwrap();
        std::fs::write(&path_b, "ScriptName B\n").unwrap();

        let ast_a = papyrus_parser::parse("ScriptName A\n").unwrap();
        let ast_b = papyrus_parser::parse("ScriptName B\n").unwrap();
        put_in(cache_dir.path(), &path_a, "ScriptName A\n", &ast_a);
        put_in(cache_dir.path(), &path_b, "ScriptName B\n", &ast_b);

        assert_eq!(
            get_in(cache_dir.path(), &path_a, "ScriptName A\n"),
            Some(ast_a)
        );
        assert_eq!(
            get_in(cache_dir.path(), &path_b, "ScriptName B\n"),
            Some(ast_b)
        );
    }

    #[test]
    fn public_get_and_put_do_not_panic() {
        let project_dir = tempdir().unwrap();
        let source_path = project_dir.path().join("Example.psc");
        let source = "ScriptName Example\n";
        std::fs::write(&source_path, source).unwrap();

        let ast = sample_ast();
        put(&source_path, source, &ast);
        let _ = get(&source_path, source);
    }
}
