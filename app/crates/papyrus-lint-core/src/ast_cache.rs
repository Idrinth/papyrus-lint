//! Disk-backed cache of parsed `.psc` ASTs, shared by the desktop app and
//! the CLI, so reopening an unchanged script (e.g. switching between files
//! in the code viewer, relinting an achlist, or resolving the same
//! cross-script lookup across separate CLI invocations) skips re-parsing
//! it. Entries live as one JSON file per source path in an `ast-cache`
//! directory next to the running executable -- the desktop app's own
//! binary, or `PapyrusLinterCLI`'s, whichever process is doing the parsing
//! -- and are invalidated by the source file's last-modified timestamp, an
//! MD5 of its content, and the linter version that wrote the entry -- if
//! any of the three is no longer valid, it's treated as a miss and the
//! caller re-parses.
//!
//! The version check is a minimum-compatible-version check against
//! [`MIN_COMPATIBLE_VERSION`], not an exact match against the running
//! linter's own version: an entry written by any release at or after
//! `MIN_COMPATIBLE_VERSION` is accepted, so an ordinary app update doesn't
//! discard an otherwise still-valid cache. Bump `MIN_COMPATIBLE_VERSION`
//! only when a release actually changes the on-disk `CacheEntry` layout or
//! the `papyrus_parser::ast::Script` shape it embeds in a way that would
//! break reading older entries.
//!
//! Caching is a pure optimization: any I/O or (de)serialization failure
//! here is swallowed and simply falls through to a fresh parse, never
//! surfaced as a lint error.
//!
//! Each entry also carries the lexer's token stream
//! (`papyrus_parser::tokenize()`'s output) alongside the AST, via
//! [`get_tokens`]/[`put_tokens`], sharing the same freshness metadata as
//! the AST accessors -- a `put`/`put_tokens` call preserves whatever
//! still-valid value the other field already held instead of clobbering it.
//! `put_tokens` is called wherever a script is freshly parsed (see
//! `get_tokens`'s doc comment for the call sites), so entries accumulate a
//! cached token stream too; nothing reads one back via `get_tokens` yet.

use std::path::{Path, PathBuf};
use std::time::UNIX_EPOCH;

use serde::{Deserialize, Serialize};

const CACHE_DIR_NAME: &str = "ast-cache";

/// The oldest linter release whose AST cache entries the running binary
/// still accepts. See the module docs above for when to bump this.
const MIN_COMPATIBLE_VERSION: &str = "1.28.0";

#[derive(Serialize, Deserialize)]
struct CacheEntry {
    modified_unix_secs: u64,
    content_md5: String,
    linter_version: String,
    ast: Option<papyrus_parser::ast::Script>,
    #[serde(default)]
    tokens: Option<Vec<papyrus_parser::token::Token>>,
}

/// Parses a `major.minor.patch` version string into a comparable tuple.
/// Returns `None` for anything that doesn't parse that way, so a malformed
/// or unexpected version string is treated as incompatible rather than
/// panicking.
fn parse_version(version: &str) -> Option<(u64, u64, u64)> {
    let mut parts = version.split('.');
    let major = parts.next()?.parse().ok()?;
    let minor = parts.next()?.parse().ok()?;
    let patch = parts.next()?.parse().ok()?;
    if parts.next().is_some() {
        return None;
    }
    Some((major, minor, patch))
}

/// Whether a cache entry written by `version` is still readable by this
/// binary, i.e. `version >= MIN_COMPATIBLE_VERSION`. Either version failing
/// to parse is treated as incompatible.
fn is_compatible_version(version: &str) -> bool {
    let Some(min) = parse_version(MIN_COMPATIBLE_VERSION) else {
        return false;
    };
    parse_version(version).is_some_and(|v| v >= min)
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

/// Reads back the cache entry for `source_path`/`source`, if one exists and
/// is still fresh (matching content/mtime and at or above
/// [`MIN_COMPATIBLE_VERSION`]). Shared by the `ast` and `tokens` accessors
/// below, and by each one's `put` so that writing one field preserves
/// whatever still-valid value the other field already held.
fn valid_entry_in(dir: &Path, source_path: &Path, source: &str) -> Option<CacheEntry> {
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

fn get_in(dir: &Path, source_path: &Path, source: &str) -> Option<papyrus_parser::ast::Script> {
    valid_entry_in(dir, source_path, source)?.ast
}

fn get_tokens_in(
    dir: &Path,
    source_path: &Path,
    source: &str,
) -> Option<Vec<papyrus_parser::token::Token>> {
    valid_entry_in(dir, source_path, source)?.tokens
}

fn write_entry_in(dir: &Path, source_path: &Path, entry: &CacheEntry) {
    let Ok(serialized) = serde_json::to_vec(entry) else {
        return;
    };
    if std::fs::create_dir_all(dir).is_err() {
        return;
    }
    let _ = std::fs::write(cache_file_path(dir, source_path), serialized);
}

fn put_in(
    dir: &Path,
    source_path: &Path,
    source: &str,
    ast: &papyrus_parser::ast::Script,
    linter_version: &str,
) {
    let Some(modified_unix_secs) = file_modified_unix_secs(source_path) else {
        return;
    };
    let tokens = valid_entry_in(dir, source_path, source).and_then(|entry| entry.tokens);
    let entry = CacheEntry {
        modified_unix_secs,
        content_md5: format!("{:x}", md5::compute(source.as_bytes())),
        linter_version: linter_version.to_string(),
        ast: Some(ast.clone()),
        tokens,
    };
    write_entry_in(dir, source_path, &entry);
}

fn put_tokens_in(
    dir: &Path,
    source_path: &Path,
    source: &str,
    tokens: &[papyrus_parser::token::Token],
    linter_version: &str,
) {
    let Some(modified_unix_secs) = file_modified_unix_secs(source_path) else {
        return;
    };
    let ast = valid_entry_in(dir, source_path, source).and_then(|entry| entry.ast);
    let entry = CacheEntry {
        modified_unix_secs,
        content_md5: format!("{:x}", md5::compute(source.as_bytes())),
        linter_version: linter_version.to_string(),
        ast,
        tokens: Some(tokens.to_vec()),
    };
    write_entry_in(dir, source_path, &entry);
}

/// Returns the cached AST for `source_path` if the on-disk cache has a
/// still-valid entry for `source`'s current content, `source_path`'s
/// modification time, and a linter version at or above
/// [`MIN_COMPATIBLE_VERSION`]. Returns `None` on any cache miss, mismatch, or
/// error -- the caller should parse `source` fresh in that case.
pub fn get(source_path: &Path, source: &str) -> Option<papyrus_parser::ast::Script> {
    get_in(&cache_dir()?, source_path, source)
}

/// Persists `ast`, parsed from `source_path`/`source`, to the on-disk cache
/// for later [`get`] calls. Any failure (e.g. an unwritable install
/// directory) is silently ignored.
pub fn put(source_path: &Path, source: &str, ast: &papyrus_parser::ast::Script) {
    if let Some(dir) = cache_dir() {
        put_in(&dir, source_path, source, ast, env!("CARGO_PKG_VERSION"));
    }
}

/// Returns the cached tokens for `source_path` if the on-disk cache has a
/// still-valid entry for `source`'s current content, `source_path`'s
/// modification time, and a linter version at or above
/// [`MIN_COMPATIBLE_VERSION`]. Returns `None` on any cache miss, mismatch, or
/// error -- the caller should tokenize `source` fresh in that case.
///
/// Not yet called anywhere: [`put_tokens`] is now populated alongside every
/// fresh parse (see `parse_psc_file` in `app/src-tauri/src/lib.rs` and
/// `FunctionTable::ensure_loaded`), but nothing yet reads a cached token
/// stream back out -- lint rules that need tokens still tokenize `source`
/// directly, relying on `papyrus_parser`'s own in-memory memoization rather
/// than this disk cache.
pub fn get_tokens(source_path: &Path, source: &str) -> Option<Vec<papyrus_parser::token::Token>> {
    get_tokens_in(&cache_dir()?, source_path, source)
}

/// Persists `tokens`, lexed from `source_path`/`source`, to the on-disk
/// cache for later [`get_tokens`] calls. Any failure (e.g. an unwritable
/// install directory) is silently ignored.
pub fn put_tokens(source_path: &Path, source: &str, tokens: &[papyrus_parser::token::Token]) {
    if let Some(dir) = cache_dir() {
        put_tokens_in(&dir, source_path, source, tokens, env!("CARGO_PKG_VERSION"));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    /// A version at the minimum compatible threshold, used by tests that
    /// don't care about version compatibility itself.
    const COMPATIBLE_VERSION: &str = MIN_COMPATIBLE_VERSION;

    fn sample_ast() -> papyrus_parser::ast::Script {
        papyrus_parser::parse("ScriptName Example\n").unwrap()
    }

    fn sample_tokens() -> Vec<papyrus_parser::token::Token> {
        papyrus_parser::tokenize("ScriptName Example\n").unwrap()
    }

    #[test]
    fn put_then_get_returns_the_cached_ast_when_nothing_changed() {
        let cache_dir = tempdir().unwrap();
        let project_dir = tempdir().unwrap();
        let source_path = project_dir.path().join("Example.psc");
        let source = "ScriptName Example\n";
        std::fs::write(&source_path, source).unwrap();

        let ast = sample_ast();
        put_in(
            cache_dir.path(),
            &source_path,
            source,
            &ast,
            COMPATIBLE_VERSION,
        );

        assert_eq!(get_in(cache_dir.path(), &source_path, source), Some(ast));
    }

    #[test]
    fn get_is_a_hit_when_the_cached_version_is_newer_than_the_minimum_compatible_version() {
        let cache_dir = tempdir().unwrap();
        let project_dir = tempdir().unwrap();
        let source_path = project_dir.path().join("Example.psc");
        let source = "ScriptName Example\n";
        std::fs::write(&source_path, source).unwrap();

        let ast = sample_ast();
        put_in(cache_dir.path(), &source_path, source, &ast, "9.9.9");

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

        put_in(
            cache_dir.path(),
            &source_path,
            original,
            &sample_ast(),
            COMPATIBLE_VERSION,
        );

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

        put_in(
            cache_dir.path(),
            &source_path,
            source,
            &sample_ast(),
            COMPATIBLE_VERSION,
        );

        let filetime_now = std::time::SystemTime::now() + std::time::Duration::from_secs(120);
        std::fs::write(&source_path, source).unwrap();
        let file = std::fs::File::open(&source_path).unwrap();
        file.set_modified(filetime_now).unwrap();

        assert_eq!(get_in(cache_dir.path(), &source_path, source), None);
    }

    #[test]
    fn get_is_a_miss_when_the_cached_version_is_older_than_the_minimum_compatible_version() {
        let cache_dir = tempdir().unwrap();
        let project_dir = tempdir().unwrap();
        let source_path = project_dir.path().join("Example.psc");
        let source = "ScriptName Example\n";
        std::fs::write(&source_path, source).unwrap();

        let entry = CacheEntry {
            modified_unix_secs: file_modified_unix_secs(&source_path).unwrap(),
            content_md5: format!("{:x}", md5::compute(source.as_bytes())),
            linter_version: "1.10.1".to_string(),
            ast: Some(sample_ast()),
            tokens: None,
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
    fn get_is_a_miss_when_the_cached_version_does_not_parse() {
        let cache_dir = tempdir().unwrap();
        let project_dir = tempdir().unwrap();
        let source_path = project_dir.path().join("Example.psc");
        let source = "ScriptName Example\n";
        std::fs::write(&source_path, source).unwrap();

        let entry = CacheEntry {
            modified_unix_secs: file_modified_unix_secs(&source_path).unwrap(),
            content_md5: format!("{:x}", md5::compute(source.as_bytes())),
            linter_version: "not-a-version".to_string(),
            ast: Some(sample_ast()),
            tokens: None,
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
    fn parse_version_rejects_malformed_strings() {
        assert_eq!(parse_version("1.11.0"), Some((1, 11, 0)));
        assert_eq!(parse_version("1.11"), None);
        assert_eq!(parse_version("1.11.x"), None);
        assert_eq!(parse_version(""), None);
    }

    #[test]
    fn parse_version_requires_exactly_three_numeric_components() {
        assert_eq!(parse_version("1.13.0.1"), None);
        assert_eq!(parse_version("1.13.0-beta"), None);
        assert_eq!(parse_version("v1.13.0"), None);
        assert_eq!(parse_version("18446744073709551616.0.0"), None);
    }

    #[test]
    fn is_compatible_version_accepts_the_minimum_and_anything_newer() {
        assert!(is_compatible_version(MIN_COMPATIBLE_VERSION));
        assert!(is_compatible_version("1.28.1"));
        assert!(is_compatible_version("2.0.0"));
        assert!(!is_compatible_version("1.27.99"));
        assert!(!is_compatible_version("not-a-version"));
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

        put_in(
            &nested_cache_dir,
            &source_path,
            source,
            &sample_ast(),
            COMPATIBLE_VERSION,
        );

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
        put_in(
            cache_dir.path(),
            &path_a,
            "ScriptName A\n",
            &ast_a,
            COMPATIBLE_VERSION,
        );
        put_in(
            cache_dir.path(),
            &path_b,
            "ScriptName B\n",
            &ast_b,
            COMPATIBLE_VERSION,
        );

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
    fn same_named_scripts_in_different_projects_do_not_share_a_cache_entry() {
        let cache_dir = tempdir().unwrap();
        let projects_dir = tempdir().unwrap();
        let project_a = projects_dir.path().join("ProjectA");
        let project_b = projects_dir.path().join("ProjectB");
        std::fs::create_dir_all(&project_a).unwrap();
        std::fs::create_dir_all(&project_b).unwrap();

        let path_a = project_a.join("Shared.psc");
        let path_b = project_b.join("Shared.psc");
        let source_a = "ScriptName Shared\nInt Property ProjectId = 1 Auto\n";
        let source_b = "ScriptName Shared\nInt Property ProjectId = 2 Auto\n";
        std::fs::write(&path_a, source_a).unwrap();
        std::fs::write(&path_b, source_b).unwrap();

        let ast_a = papyrus_parser::parse(source_a).unwrap();
        let ast_b = papyrus_parser::parse(source_b).unwrap();
        put_in(
            cache_dir.path(),
            &path_a,
            source_a,
            &ast_a,
            COMPATIBLE_VERSION,
        );
        put_in(
            cache_dir.path(),
            &path_b,
            source_b,
            &ast_b,
            COMPATIBLE_VERSION,
        );

        assert_ne!(
            cache_file_path(cache_dir.path(), &path_a),
            cache_file_path(cache_dir.path(), &path_b)
        );
        assert_eq!(get_in(cache_dir.path(), &path_a, source_a), Some(ast_a));
        assert_eq!(get_in(cache_dir.path(), &path_b, source_b), Some(ast_b));
    }

    #[test]
    fn put_replaces_a_corrupt_entry_with_a_readable_cache_entry() {
        let cache_dir = tempdir().unwrap();
        let project_dir = tempdir().unwrap();
        let source_path = project_dir.path().join("Example.psc");
        let source = "ScriptName Example\n";
        std::fs::write(&source_path, source).unwrap();

        std::fs::write(
            cache_file_path(cache_dir.path(), &source_path),
            b"a previous process left an incomplete cache entry",
        )
        .unwrap();
        assert_eq!(get_in(cache_dir.path(), &source_path, source), None);

        let ast = sample_ast();
        put_in(
            cache_dir.path(),
            &source_path,
            source,
            &ast,
            COMPATIBLE_VERSION,
        );

        assert_eq!(get_in(cache_dir.path(), &source_path, source), Some(ast));
    }

    #[test]
    fn cache_file_path_is_stable_and_does_not_expose_the_source_filename() {
        let cache_dir = Path::new("/tmp/cache");
        let source_path = Path::new("/projects/private/MyScript.psc");

        let first = cache_file_path(cache_dir, source_path);
        let second = cache_file_path(cache_dir, source_path);

        assert_eq!(first, second);
        assert_eq!(first.parent(), Some(cache_dir));
        assert_eq!(
            first.extension().and_then(|value| value.to_str()),
            Some("json")
        );
        assert!(!first.to_string_lossy().contains("MyScript"));
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

    #[test]
    fn put_is_a_noop_when_the_source_file_does_not_exist() {
        let cache_dir = tempdir().unwrap();
        let missing_source = cache_dir.path().join("Missing.psc");

        put_in(
            cache_dir.path(),
            &missing_source,
            "ScriptName Missing\n",
            &sample_ast(),
            COMPATIBLE_VERSION,
        );

        assert!(!cache_file_path(cache_dir.path(), &missing_source).exists());
    }

    #[test]
    fn get_is_a_miss_when_the_source_file_was_deleted() {
        let cache_dir = tempdir().unwrap();
        let project_dir = tempdir().unwrap();
        let source_path = project_dir.path().join("Example.psc");
        let source = "ScriptName Example\n";
        std::fs::write(&source_path, source).unwrap();
        put_in(
            cache_dir.path(),
            &source_path,
            source,
            &sample_ast(),
            COMPATIBLE_VERSION,
        );
        std::fs::remove_file(&source_path).unwrap();

        assert_eq!(get_in(cache_dir.path(), &source_path, source), None);
    }

    #[test]
    fn put_tokens_then_get_tokens_returns_the_cached_tokens_when_nothing_changed() {
        let cache_dir = tempdir().unwrap();
        let project_dir = tempdir().unwrap();
        let source_path = project_dir.path().join("Example.psc");
        let source = "ScriptName Example\n";
        std::fs::write(&source_path, source).unwrap();

        let tokens = sample_tokens();
        put_tokens_in(
            cache_dir.path(),
            &source_path,
            source,
            &tokens,
            COMPATIBLE_VERSION,
        );

        assert_eq!(
            get_tokens_in(cache_dir.path(), &source_path, source),
            Some(tokens)
        );
    }

    #[test]
    fn get_tokens_is_a_miss_for_an_uncached_path() {
        let cache_dir = tempdir().unwrap();
        let project_dir = tempdir().unwrap();
        let source_path = project_dir.path().join("Example.psc");
        std::fs::write(&source_path, "ScriptName Example\n").unwrap();

        assert_eq!(
            get_tokens_in(cache_dir.path(), &source_path, "ScriptName Example\n"),
            None
        );
    }

    #[test]
    fn get_tokens_is_a_miss_when_the_content_changed_even_if_the_mtime_did_not() {
        let cache_dir = tempdir().unwrap();
        let project_dir = tempdir().unwrap();
        let source_path = project_dir.path().join("Example.psc");
        let original = "ScriptName Example\n";
        std::fs::write(&source_path, original).unwrap();

        put_tokens_in(
            cache_dir.path(),
            &source_path,
            original,
            &sample_tokens(),
            COMPATIBLE_VERSION,
        );

        let changed = "ScriptName Renamed\n";
        assert_eq!(get_tokens_in(cache_dir.path(), &source_path, changed), None);
    }

    #[test]
    fn putting_tokens_preserves_an_already_cached_ast() {
        let cache_dir = tempdir().unwrap();
        let project_dir = tempdir().unwrap();
        let source_path = project_dir.path().join("Example.psc");
        let source = "ScriptName Example\n";
        std::fs::write(&source_path, source).unwrap();

        let ast = sample_ast();
        put_in(
            cache_dir.path(),
            &source_path,
            source,
            &ast,
            COMPATIBLE_VERSION,
        );

        let tokens = sample_tokens();
        put_tokens_in(
            cache_dir.path(),
            &source_path,
            source,
            &tokens,
            COMPATIBLE_VERSION,
        );

        assert_eq!(get_in(cache_dir.path(), &source_path, source), Some(ast));
        assert_eq!(
            get_tokens_in(cache_dir.path(), &source_path, source),
            Some(tokens)
        );
    }

    #[test]
    fn putting_ast_preserves_already_cached_tokens() {
        let cache_dir = tempdir().unwrap();
        let project_dir = tempdir().unwrap();
        let source_path = project_dir.path().join("Example.psc");
        let source = "ScriptName Example\n";
        std::fs::write(&source_path, source).unwrap();

        let tokens = sample_tokens();
        put_tokens_in(
            cache_dir.path(),
            &source_path,
            source,
            &tokens,
            COMPATIBLE_VERSION,
        );

        let ast = sample_ast();
        put_in(
            cache_dir.path(),
            &source_path,
            source,
            &ast,
            COMPATIBLE_VERSION,
        );

        assert_eq!(get_in(cache_dir.path(), &source_path, source), Some(ast));
        assert_eq!(
            get_tokens_in(cache_dir.path(), &source_path, source),
            Some(tokens)
        );
    }

    #[test]
    fn an_entry_missing_the_tokens_field_still_deserializes_as_a_miss_for_get_tokens() {
        let cache_dir = tempdir().unwrap();
        let project_dir = tempdir().unwrap();
        let source_path = project_dir.path().join("Example.psc");
        let source = "ScriptName Example\n";
        std::fs::write(&source_path, source).unwrap();

        // Simulates an entry written before the `tokens` field existed --
        // #[serde(default)] should fill it in as `None` on read rather than
        // failing to deserialize.
        let raw = format!(
            r#"{{"modified_unix_secs":{},"content_md5":"{:x}","linter_version":"{}","ast":{}}}"#,
            file_modified_unix_secs(&source_path).unwrap(),
            md5::compute(source.as_bytes()),
            COMPATIBLE_VERSION,
            serde_json::to_string(&sample_ast()).unwrap(),
        );
        std::fs::create_dir_all(cache_dir.path()).unwrap();
        std::fs::write(cache_file_path(cache_dir.path(), &source_path), raw).unwrap();

        assert_eq!(
            get_in(cache_dir.path(), &source_path, source),
            Some(sample_ast())
        );
        assert_eq!(get_tokens_in(cache_dir.path(), &source_path, source), None);
    }

    #[test]
    fn public_get_tokens_and_put_tokens_do_not_panic() {
        let project_dir = tempdir().unwrap();
        let source_path = project_dir.path().join("Example.psc");
        let source = "ScriptName Example\n";
        std::fs::write(&source_path, source).unwrap();

        let tokens = sample_tokens();
        put_tokens(&source_path, source, &tokens);
        let _ = get_tokens(&source_path, source);
    }
}
