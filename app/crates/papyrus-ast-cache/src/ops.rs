//! `get`/`put`/`ensure_primed` semantics for a given cache directory, built
//! on the on-disk primitives in [`crate::entry`]. [`crate::lib`] wraps these
//! with [`crate::CACHE_LOCK`] and the real `ast-cache` directory to form the
//! crate's public API.

use std::path::Path;

use crate::entry::{file_modified_unix_secs, valid_entry_in, write_entry_in, CacheEntry};

/// Also primes `papyrus_parser`'s own in-memory memoization (see
/// [`papyrus_parser::prime_cache`]) with a hit, so anything that parses
/// `source` itself later in this process -- notably
/// `papyrus_lints::lint()`/`repair()`, which never see `source_path` and so
/// can't consult this cache directly -- reuses it instead of re-parsing.
pub(crate) fn get_in(
    dir: &Path,
    source_path: &Path,
    source: &str,
) -> Option<papyrus_parser::ast::Script> {
    let ast = valid_entry_in(dir, source_path, source)?.ast?;
    papyrus_parser::prime_cache(source, ast.clone());
    Some(ast)
}

/// Also primes `papyrus_parser`'s own in-memory memoization (see
/// [`papyrus_parser::prime_tokenize_cache`]) with a hit, the same way
/// [`get_in`] does for the AST.
pub(crate) fn get_tokens_in(
    dir: &Path,
    source_path: &Path,
    source: &str,
) -> Option<Vec<papyrus_parser::token::Token>> {
    let tokens = valid_entry_in(dir, source_path, source)?.tokens?;
    papyrus_parser::prime_tokenize_cache(source, tokens.clone());
    Some(tokens)
}

pub(crate) fn put_in(
    dir: &Path,
    source_path: &Path,
    source: &str,
    ast: &papyrus_parser::ast::Script,
    linter_version: &str,
) {
    let tokens = valid_entry_in(dir, source_path, source).and_then(|entry| entry.tokens);
    write_stamped_entry(
        dir,
        source_path,
        source,
        linter_version,
        Some(ast.clone()),
        tokens,
    );
}

pub(crate) fn put_tokens_in(
    dir: &Path,
    source_path: &Path,
    source: &str,
    tokens: &[papyrus_parser::token::Token],
    linter_version: &str,
) {
    let ast = valid_entry_in(dir, source_path, source).and_then(|entry| entry.ast);
    write_stamped_entry(
        dir,
        source_path,
        source,
        linter_version,
        ast,
        Some(tokens.to_vec()),
    );
}

fn write_stamped_entry(
    dir: &Path,
    source_path: &Path,
    source: &str,
    linter_version: &str,
    ast: Option<papyrus_parser::ast::Script>,
    tokens: Option<Vec<papyrus_parser::token::Token>>,
) {
    let Some(modified_unix_secs) = file_modified_unix_secs(source_path) else {
        return;
    };
    let entry = CacheEntry {
        modified_unix_secs,
        content_md5: format!("{:x}", md5::compute(source.as_bytes())),
        linter_version: linter_version.to_string(),
        ast,
        tokens,
    };
    write_entry_in(dir, source_path, &entry);
}

/// Makes sure `papyrus_parser`'s in-memory memoization has both an AST and
/// a token stream ready for `source` before something that parses/
/// tokenizes `source` itself -- typically `papyrus_lints::lint()`/
/// `repair()`, called with only the raw source text, never `source_path` --
/// runs. A disk cache hit for either ([`get_in`]/[`get_tokens_in`]) already
/// primes the matching in-memory cache as a side effect; a miss for either
/// parses/tokenizes `source` once here instead (which populates the
/// in-memory cache the same way a hit would) and writes the result to the
/// disk cache for next time. See [`crate::ensure_primed`], the public
/// wrapper that supplies the real cache directory and version.
pub(crate) fn ensure_primed_in(dir: &Path, source_path: &Path, source: &str, linter_version: &str) {
    if get_in(dir, source_path, source).is_none() {
        if let Ok(ast) = papyrus_parser::parse(source) {
            put_in(dir, source_path, source, &ast, linter_version);
        }
    }
    if get_tokens_in(dir, source_path, source).is_none() {
        if let Ok(tokens) = papyrus_parser::tokenize(source) {
            put_tokens_in(dir, source_path, source, &tokens, linter_version);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::entry::cache_file_path;
    use crate::version::MIN_COMPATIBLE_VERSION;
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

    struct Harness {
        cache_dir: tempfile::TempDir,
        _project_dir: tempfile::TempDir,
        source_path: std::path::PathBuf,
        source: &'static str,
    }

    fn harness(filename: &str, source: &'static str) -> Harness {
        let cache_dir = tempdir().unwrap();
        let project_dir = tempdir().unwrap();
        let source_path = project_dir.path().join(filename);
        std::fs::write(&source_path, source).unwrap();
        Harness {
            cache_dir,
            _project_dir: project_dir,
            source_path,
            source,
        }
    }

    fn write_raw(h: &Harness, entry: CacheEntry) {
        std::fs::create_dir_all(h.cache_dir.path()).unwrap();
        std::fs::write(
            cache_file_path(h.cache_dir.path(), &h.source_path),
            serde_json::to_vec(&entry).unwrap(),
        )
        .unwrap();
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
    fn public_get_and_put_do_not_panic() {
        let project_dir = tempdir().unwrap();
        let source_path = project_dir.path().join("Example.psc");
        let source = "ScriptName Example\n";
        std::fs::write(&source_path, source).unwrap();

        let ast = sample_ast();
        crate::put(&source_path, source, &ast);
        let _ = crate::get(&source_path, source);
    }

    #[test]
    fn ensure_primed_populates_the_disk_cache_on_a_miss_and_is_a_hit_afterwards() {
        let cache_dir = tempdir().unwrap();
        let project_dir = tempdir().unwrap();
        let source_path = project_dir.path().join("EnsurePrimedMiss.psc");
        let source = "ScriptName EnsurePrimedMiss\n";
        std::fs::write(&source_path, source).unwrap();

        assert_eq!(get_in(cache_dir.path(), &source_path, source), None);
        assert_eq!(get_tokens_in(cache_dir.path(), &source_path, source), None);

        ensure_primed_in(cache_dir.path(), &source_path, source, COMPATIBLE_VERSION);

        assert_eq!(
            get_in(cache_dir.path(), &source_path, source),
            Some(papyrus_parser::parse(source).unwrap())
        );
        assert_eq!(
            get_tokens_in(cache_dir.path(), &source_path, source),
            Some(papyrus_parser::tokenize(source).unwrap())
        );
    }

    #[test]
    fn ensure_primed_is_a_noop_on_an_already_cached_entry() {
        let cache_dir = tempdir().unwrap();
        let project_dir = tempdir().unwrap();
        let source_path = project_dir.path().join("EnsurePrimedHit.psc");
        let source = "ScriptName EnsurePrimedHit\n";
        std::fs::write(&source_path, source).unwrap();

        ensure_primed_in(cache_dir.path(), &source_path, source, COMPATIBLE_VERSION);
        let first_ast = get_in(cache_dir.path(), &source_path, source);
        let first_tokens = get_tokens_in(cache_dir.path(), &source_path, source);

        ensure_primed_in(cache_dir.path(), &source_path, source, COMPATIBLE_VERSION);
        let second_ast = get_in(cache_dir.path(), &source_path, source);
        let second_tokens = get_tokens_in(cache_dir.path(), &source_path, source);

        assert_eq!(first_ast, second_ast);
        assert!(first_ast.is_some());
        assert_eq!(first_tokens, second_tokens);
        assert!(first_tokens.is_some());
    }

    #[test]
    fn ensure_primed_fills_in_a_missing_tokens_field_without_disturbing_an_already_cached_ast() {
        let cache_dir = tempdir().unwrap();
        let project_dir = tempdir().unwrap();
        let source_path = project_dir.path().join("EnsurePrimedAstOnly.psc");
        let source = "ScriptName EnsurePrimedAstOnly\n";
        std::fs::write(&source_path, source).unwrap();

        let ast = sample_ast();
        put_in(
            cache_dir.path(),
            &source_path,
            source,
            &ast,
            COMPATIBLE_VERSION,
        );
        assert_eq!(get_tokens_in(cache_dir.path(), &source_path, source), None);

        ensure_primed_in(cache_dir.path(), &source_path, source, COMPATIBLE_VERSION);

        assert_eq!(get_in(cache_dir.path(), &source_path, source), Some(ast));
        assert_eq!(
            get_tokens_in(cache_dir.path(), &source_path, source),
            Some(papyrus_parser::tokenize(source).unwrap())
        );
    }

    #[test]
    fn ensure_primed_fills_in_a_missing_ast_field_without_disturbing_already_cached_tokens() {
        let cache_dir = tempdir().unwrap();
        let project_dir = tempdir().unwrap();
        let source_path = project_dir.path().join("EnsurePrimedTokensOnly.psc");
        let source = "ScriptName EnsurePrimedTokensOnly\n";
        std::fs::write(&source_path, source).unwrap();

        let tokens = papyrus_parser::tokenize(source).unwrap();
        put_tokens_in(
            cache_dir.path(),
            &source_path,
            source,
            &tokens,
            COMPATIBLE_VERSION,
        );
        assert_eq!(get_in(cache_dir.path(), &source_path, source), None);

        ensure_primed_in(cache_dir.path(), &source_path, source, COMPATIBLE_VERSION);

        assert_eq!(
            get_in(cache_dir.path(), &source_path, source),
            Some(papyrus_parser::parse(source).unwrap())
        );
        assert_eq!(
            get_tokens_in(cache_dir.path(), &source_path, source),
            Some(tokens)
        );
    }

    #[test]
    fn ensure_primed_caches_tokens_when_the_source_does_not_parse() {
        let cache_dir = tempdir().unwrap();
        let project_dir = tempdir().unwrap();
        let source_path = project_dir.path().join("Invalid.psc");
        let source = "ScriptName Invalid\nFunction Broken(\n";
        std::fs::write(&source_path, source).unwrap();
        assert!(papyrus_parser::parse(source).is_err());

        ensure_primed_in(cache_dir.path(), &source_path, source, COMPATIBLE_VERSION);

        assert_eq!(get_in(cache_dir.path(), &source_path, source), None);
        assert_eq!(
            get_tokens_in(cache_dir.path(), &source_path, source),
            Some(papyrus_parser::tokenize(source).unwrap())
        );
    }

    #[test]
    fn get_in_primes_papyrus_parsers_in_memory_cache_with_the_disk_cached_ast() {
        let cache_dir = tempdir().unwrap();
        let project_dir = tempdir().unwrap();
        let source_path = project_dir.path().join("PrimesInMemory.psc");
        let source = "ScriptName PrimesInMemory extends Quest\n";
        std::fs::write(&source_path, source).unwrap();

        // Deliberately not what `source` actually parses to, so that a
        // subsequent `papyrus_parser::parse(source)` call returning it
        // proves it came from `get_in`'s in-memory priming rather than a
        // fresh parse of `source`.
        let distinct_ast = papyrus_parser::parse(
            "ScriptName PrimesInMemory extends Quest\n\nInt Property Marker = 1 Auto\n",
        )
        .unwrap();
        put_in(
            cache_dir.path(),
            &source_path,
            source,
            &distinct_ast,
            COMPATIBLE_VERSION,
        );

        let cached = get_in(cache_dir.path(), &source_path, source).unwrap();
        assert_eq!(cached, distinct_ast);
        assert_eq!(papyrus_parser::parse(source).unwrap(), distinct_ast);
    }

    #[test]
    fn get_tokens_in_primes_papyrus_parsers_in_memory_cache_with_the_disk_cached_tokens() {
        let cache_dir = tempdir().unwrap();
        let project_dir = tempdir().unwrap();
        let source_path = project_dir.path().join("PrimesTokensInMemory.psc");
        let source = "ScriptName PrimesTokensInMemory extends Quest\n";
        std::fs::write(&source_path, source).unwrap();

        // Deliberately not what `source` actually tokenizes to, so that a
        // subsequent `papyrus_parser::tokenize(source)` call returning it
        // proves it came from `get_tokens_in`'s in-memory priming rather
        // than a fresh tokenize of `source`.
        let distinct_tokens = papyrus_parser::tokenize(
            "ScriptName PrimesTokensInMemory extends Quest\n\nInt Property Marker = 1 Auto\n",
        )
        .unwrap();
        put_tokens_in(
            cache_dir.path(),
            &source_path,
            source,
            &distinct_tokens,
            COMPATIBLE_VERSION,
        );

        let cached = get_tokens_in(cache_dir.path(), &source_path, source).unwrap();
        assert_eq!(cached, distinct_tokens);
        assert_eq!(papyrus_parser::tokenize(source).unwrap(), distinct_tokens);
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
    fn get_tokens_is_a_miss_when_the_file_was_modified_after_caching() {
        let cache_dir = tempdir().unwrap();
        let project_dir = tempdir().unwrap();
        let source_path = project_dir.path().join("Example.psc");
        let source = "ScriptName Example\n";
        std::fs::write(&source_path, source).unwrap();

        put_tokens_in(
            cache_dir.path(),
            &source_path,
            source,
            &sample_tokens(),
            COMPATIBLE_VERSION,
        );

        let changed_time = std::time::SystemTime::now() + std::time::Duration::from_secs(120);
        let file = std::fs::File::open(&source_path).unwrap();
        file.set_modified(changed_time).unwrap();

        assert_eq!(get_tokens_in(cache_dir.path(), &source_path, source), None);
    }

    #[test]
    fn get_tokens_is_a_miss_when_the_source_file_was_deleted() {
        let cache_dir = tempdir().unwrap();
        let project_dir = tempdir().unwrap();
        let source_path = project_dir.path().join("Example.psc");
        let source = "ScriptName Example\n";
        std::fs::write(&source_path, source).unwrap();
        put_tokens_in(
            cache_dir.path(),
            &source_path,
            source,
            &sample_tokens(),
            COMPATIBLE_VERSION,
        );
        std::fs::remove_file(&source_path).unwrap();

        assert_eq!(get_tokens_in(cache_dir.path(), &source_path, source), None);
    }

    #[test]
    fn put_tokens_is_a_noop_when_the_source_file_does_not_exist() {
        let cache_dir = tempdir().unwrap();
        let missing_source = cache_dir.path().join("Missing.psc");

        put_tokens_in(
            cache_dir.path(),
            &missing_source,
            "ScriptName Missing\n",
            &sample_tokens(),
            COMPATIBLE_VERSION,
        );

        assert!(!cache_file_path(cache_dir.path(), &missing_source).exists());
    }

    #[test]
    fn writes_are_silently_ignored_when_the_cache_directory_is_a_file() {
        let root = tempdir().unwrap();
        let cache_path = root.path().join("not-a-directory");
        std::fs::write(&cache_path, "occupied").unwrap();
        let source_path = root.path().join("Example.psc");
        let source = "ScriptName Example\n";
        std::fs::write(&source_path, source).unwrap();

        put_in(
            &cache_path,
            &source_path,
            source,
            &sample_ast(),
            COMPATIBLE_VERSION,
        );
        put_tokens_in(
            &cache_path,
            &source_path,
            source,
            &sample_tokens(),
            COMPATIBLE_VERSION,
        );

        assert_eq!(std::fs::read_to_string(&cache_path).unwrap(), "occupied");
        assert_eq!(get_in(&cache_path, &source_path, source), None);
        assert_eq!(get_tokens_in(&cache_path, &source_path, source), None);
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
    fn putting_ast_does_not_preserve_tokens_cached_for_different_content() {
        let cache_dir = tempdir().unwrap();
        let project_dir = tempdir().unwrap();
        let source_path = project_dir.path().join("Example.psc");
        let original = "ScriptName Original\n";
        let changed = "ScriptName Changed\n";
        std::fs::write(&source_path, original).unwrap();
        put_tokens_in(
            cache_dir.path(),
            &source_path,
            original,
            &papyrus_parser::tokenize(original).unwrap(),
            COMPATIBLE_VERSION,
        );

        std::fs::write(&source_path, changed).unwrap();
        let changed_ast = papyrus_parser::parse(changed).unwrap();
        put_in(
            cache_dir.path(),
            &source_path,
            changed,
            &changed_ast,
            COMPATIBLE_VERSION,
        );

        assert_eq!(
            get_in(cache_dir.path(), &source_path, changed),
            Some(changed_ast)
        );
        assert_eq!(get_tokens_in(cache_dir.path(), &source_path, changed), None);
    }

    #[test]
    fn putting_tokens_does_not_preserve_an_ast_cached_for_different_content() {
        let cache_dir = tempdir().unwrap();
        let project_dir = tempdir().unwrap();
        let source_path = project_dir.path().join("Example.psc");
        let original = "ScriptName Original\n";
        let changed = "ScriptName Changed\n";
        std::fs::write(&source_path, original).unwrap();
        put_in(
            cache_dir.path(),
            &source_path,
            original,
            &papyrus_parser::parse(original).unwrap(),
            COMPATIBLE_VERSION,
        );

        std::fs::write(&source_path, changed).unwrap();
        let changed_tokens = papyrus_parser::tokenize(changed).unwrap();
        put_tokens_in(
            cache_dir.path(),
            &source_path,
            changed,
            &changed_tokens,
            COMPATIBLE_VERSION,
        );

        assert_eq!(get_in(cache_dir.path(), &source_path, changed), None);
        assert_eq!(
            get_tokens_in(cache_dir.path(), &source_path, changed),
            Some(changed_tokens)
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
        crate::put_tokens(&source_path, source, &tokens);
        let _ = crate::get_tokens(&source_path, source);
    }

    #[test]
    fn get_tokens_is_a_miss_when_the_cached_version_is_older_than_the_minimum_compatible_version() {
        let h = harness("Example.psc", "ScriptName Example\n");
        write_raw(
            &h,
            CacheEntry {
                modified_unix_secs: file_modified_unix_secs(&h.source_path).unwrap(),
                content_md5: format!("{:x}", md5::compute(h.source.as_bytes())),
                linter_version: "1.10.1".to_string(),
                ast: None,
                tokens: Some(sample_tokens()),
            },
        );

        assert_eq!(
            get_tokens_in(h.cache_dir.path(), &h.source_path, h.source),
            None
        );
    }

    #[test]
    fn get_tokens_is_a_miss_when_the_cached_version_does_not_parse() {
        let h = harness("Example.psc", "ScriptName Example\n");
        write_raw(
            &h,
            CacheEntry {
                modified_unix_secs: file_modified_unix_secs(&h.source_path).unwrap(),
                content_md5: format!("{:x}", md5::compute(h.source.as_bytes())),
                linter_version: "not-a-version".to_string(),
                ast: None,
                tokens: Some(sample_tokens()),
            },
        );

        assert_eq!(
            get_tokens_in(h.cache_dir.path(), &h.source_path, h.source),
            None
        );
    }

    #[test]
    fn get_tokens_is_a_miss_on_malformed_cache_contents() {
        let h = harness("Example.psc", "ScriptName Example\n");
        std::fs::create_dir_all(h.cache_dir.path()).unwrap();
        std::fs::write(
            cache_file_path(h.cache_dir.path(), &h.source_path),
            b"not json",
        )
        .unwrap();

        assert_eq!(
            get_tokens_in(h.cache_dir.path(), &h.source_path, h.source),
            None
        );
    }

    #[test]
    fn get_tokens_is_a_hit_when_the_cached_version_is_newer_than_the_minimum() {
        let h = harness("Example.psc", "ScriptName Example\n");
        let tokens = sample_tokens();
        put_tokens_in(
            h.cache_dir.path(),
            &h.source_path,
            h.source,
            &tokens,
            "9.9.9",
        );

        assert_eq!(
            get_tokens_in(h.cache_dir.path(), &h.source_path, h.source),
            Some(tokens)
        );
    }

    #[test]
    fn get_is_a_miss_when_ast_is_explicitly_null() {
        let h = harness("Example.psc", "ScriptName Example\n");
        let tokens = sample_tokens();
        write_raw(
            &h,
            CacheEntry {
                modified_unix_secs: file_modified_unix_secs(&h.source_path).unwrap(),
                content_md5: format!("{:x}", md5::compute(h.source.as_bytes())),
                linter_version: COMPATIBLE_VERSION.to_string(),
                ast: None,
                tokens: Some(tokens.clone()),
            },
        );

        assert_eq!(get_in(h.cache_dir.path(), &h.source_path, h.source), None);
        assert_eq!(
            get_tokens_in(h.cache_dir.path(), &h.source_path, h.source),
            Some(tokens)
        );
    }

    #[test]
    fn get_tokens_is_a_miss_when_tokens_is_explicitly_null() {
        let h = harness("Example.psc", "ScriptName Example\n");
        let ast = sample_ast();
        write_raw(
            &h,
            CacheEntry {
                modified_unix_secs: file_modified_unix_secs(&h.source_path).unwrap(),
                content_md5: format!("{:x}", md5::compute(h.source.as_bytes())),
                linter_version: COMPATIBLE_VERSION.to_string(),
                ast: Some(ast.clone()),
                tokens: None,
            },
        );

        assert_eq!(
            get_in(h.cache_dir.path(), &h.source_path, h.source),
            Some(ast)
        );
        assert_eq!(
            get_tokens_in(h.cache_dir.path(), &h.source_path, h.source),
            None
        );
    }

    #[test]
    fn putting_ast_overwrites_a_previously_cached_ast() {
        let h = harness("Example.psc", "ScriptName Example\n");
        put_in(
            h.cache_dir.path(),
            &h.source_path,
            h.source,
            &sample_ast(),
            COMPATIBLE_VERSION,
        );

        let replacement =
            papyrus_parser::parse("ScriptName Example\n\nInt Property Marker = 1 Auto\n").unwrap();
        put_in(
            h.cache_dir.path(),
            &h.source_path,
            h.source,
            &replacement,
            COMPATIBLE_VERSION,
        );

        assert_eq!(
            get_in(h.cache_dir.path(), &h.source_path, h.source),
            Some(replacement)
        );
    }

    #[test]
    fn putting_tokens_overwrites_previously_cached_tokens() {
        let h = harness("Example.psc", "ScriptName Example\n");
        put_tokens_in(
            h.cache_dir.path(),
            &h.source_path,
            h.source,
            &sample_tokens(),
            COMPATIBLE_VERSION,
        );

        let replacement =
            papyrus_parser::tokenize("ScriptName Example\n\nInt Property Marker = 1 Auto\n")
                .unwrap();
        put_tokens_in(
            h.cache_dir.path(),
            &h.source_path,
            h.source,
            &replacement,
            COMPATIBLE_VERSION,
        );

        assert_eq!(
            get_tokens_in(h.cache_dir.path(), &h.source_path, h.source),
            Some(replacement)
        );
    }

    #[test]
    fn putting_tokens_does_not_preserve_an_ast_from_an_incompatible_entry() {
        let h = harness("Example.psc", "ScriptName Example\n");
        write_raw(
            &h,
            CacheEntry {
                modified_unix_secs: file_modified_unix_secs(&h.source_path).unwrap(),
                content_md5: format!("{:x}", md5::compute(h.source.as_bytes())),
                linter_version: "1.10.1".to_string(),
                ast: Some(sample_ast()),
                tokens: None,
            },
        );

        put_tokens_in(
            h.cache_dir.path(),
            &h.source_path,
            h.source,
            &sample_tokens(),
            COMPATIBLE_VERSION,
        );

        assert_eq!(get_in(h.cache_dir.path(), &h.source_path, h.source), None);
        assert_eq!(
            get_tokens_in(h.cache_dir.path(), &h.source_path, h.source),
            Some(sample_tokens())
        );
    }

    #[test]
    fn putting_ast_does_not_preserve_tokens_from_an_incompatible_entry() {
        let h = harness("Example.psc", "ScriptName Example\n");
        write_raw(
            &h,
            CacheEntry {
                modified_unix_secs: file_modified_unix_secs(&h.source_path).unwrap(),
                content_md5: format!("{:x}", md5::compute(h.source.as_bytes())),
                linter_version: "1.10.1".to_string(),
                ast: None,
                tokens: Some(sample_tokens()),
            },
        );

        put_in(
            h.cache_dir.path(),
            &h.source_path,
            h.source,
            &sample_ast(),
            COMPATIBLE_VERSION,
        );

        assert_eq!(
            get_in(h.cache_dir.path(), &h.source_path, h.source),
            Some(sample_ast())
        );
        assert_eq!(
            get_tokens_in(h.cache_dir.path(), &h.source_path, h.source),
            None
        );
    }

    #[test]
    fn putting_updates_the_stamped_linter_version() {
        let h = harness("Example.psc", "ScriptName Example\n");
        put_in(
            h.cache_dir.path(),
            &h.source_path,
            h.source,
            &sample_ast(),
            COMPATIBLE_VERSION,
        );
        put_in(
            h.cache_dir.path(),
            &h.source_path,
            h.source,
            &sample_ast(),
            "9.9.9",
        );

        let raw =
            std::fs::read_to_string(cache_file_path(h.cache_dir.path(), &h.source_path)).unwrap();
        assert!(raw.contains("\"linter_version\":\"9.9.9\""));
        assert!(!raw.contains(&format!("\"linter_version\":\"{COMPATIBLE_VERSION}\"")));
    }

    #[test]
    fn get_is_a_miss_when_the_cache_file_is_a_directory() {
        let h = harness("Example.psc", "ScriptName Example\n");
        std::fs::create_dir_all(cache_file_path(h.cache_dir.path(), &h.source_path)).unwrap();

        assert_eq!(get_in(h.cache_dir.path(), &h.source_path, h.source), None);
        assert_eq!(
            get_tokens_in(h.cache_dir.path(), &h.source_path, h.source),
            None
        );
    }

    #[test]
    fn unicode_source_paths_round_trip() {
        let h = harness("Привет.psc", "ScriptName Example\n");
        let ast = sample_ast();
        let tokens = sample_tokens();
        put_in(
            h.cache_dir.path(),
            &h.source_path,
            h.source,
            &ast,
            COMPATIBLE_VERSION,
        );
        put_tokens_in(
            h.cache_dir.path(),
            &h.source_path,
            h.source,
            &tokens,
            COMPATIBLE_VERSION,
        );

        assert_eq!(
            get_in(h.cache_dir.path(), &h.source_path, h.source),
            Some(ast)
        );
        assert_eq!(
            get_tokens_in(h.cache_dir.path(), &h.source_path, h.source),
            Some(tokens)
        );
    }

    #[test]
    fn ensure_primed_rewrites_after_the_source_content_changes() {
        let cache_dir = tempdir().unwrap();
        let project_dir = tempdir().unwrap();
        let source_path = project_dir.path().join("Rewritten.psc");
        let original = "ScriptName Original\n";
        let changed = "ScriptName Changed\n";
        std::fs::write(&source_path, original).unwrap();

        ensure_primed_in(cache_dir.path(), &source_path, original, COMPATIBLE_VERSION);
        assert_eq!(
            get_in(cache_dir.path(), &source_path, original),
            Some(papyrus_parser::parse(original).unwrap())
        );

        std::fs::write(&source_path, changed).unwrap();
        ensure_primed_in(cache_dir.path(), &source_path, changed, COMPATIBLE_VERSION);

        assert_eq!(get_in(cache_dir.path(), &source_path, original), None);
        assert_eq!(
            get_in(cache_dir.path(), &source_path, changed),
            Some(papyrus_parser::parse(changed).unwrap())
        );
        assert_eq!(
            get_tokens_in(cache_dir.path(), &source_path, changed),
            Some(papyrus_parser::tokenize(changed).unwrap())
        );
    }

    #[test]
    fn ensure_primed_rewrites_after_the_file_mtime_changes() {
        let h = harness("MtimeRewrite.psc", "ScriptName MtimeRewrite\n");
        ensure_primed_in(
            h.cache_dir.path(),
            &h.source_path,
            h.source,
            COMPATIBLE_VERSION,
        );
        assert!(get_in(h.cache_dir.path(), &h.source_path, h.source).is_some());

        let later = std::time::SystemTime::now() + std::time::Duration::from_secs(120);
        let file = std::fs::File::open(&h.source_path).unwrap();
        file.set_modified(later).unwrap();

        assert_eq!(get_in(h.cache_dir.path(), &h.source_path, h.source), None);
        ensure_primed_in(
            h.cache_dir.path(),
            &h.source_path,
            h.source,
            COMPATIBLE_VERSION,
        );
        assert_eq!(
            get_in(h.cache_dir.path(), &h.source_path, h.source),
            Some(papyrus_parser::parse(h.source).unwrap())
        );
    }

    #[test]
    fn ensure_primed_recovers_from_a_corrupt_entry() {
        let h = harness("Corrupt.psc", "ScriptName Corrupt\n");
        std::fs::create_dir_all(h.cache_dir.path()).unwrap();
        std::fs::write(
            cache_file_path(h.cache_dir.path(), &h.source_path),
            b"truncated",
        )
        .unwrap();

        ensure_primed_in(
            h.cache_dir.path(),
            &h.source_path,
            h.source,
            COMPATIBLE_VERSION,
        );

        assert_eq!(
            get_in(h.cache_dir.path(), &h.source_path, h.source),
            Some(papyrus_parser::parse(h.source).unwrap())
        );
        assert_eq!(
            get_tokens_in(h.cache_dir.path(), &h.source_path, h.source),
            Some(papyrus_parser::tokenize(h.source).unwrap())
        );
    }

    #[test]
    fn ensure_primed_recovers_from_an_incompatible_version() {
        let h = harness("OldVersion.psc", "ScriptName OldVersion\n");
        write_raw(
            &h,
            CacheEntry {
                modified_unix_secs: file_modified_unix_secs(&h.source_path).unwrap(),
                content_md5: format!("{:x}", md5::compute(h.source.as_bytes())),
                linter_version: "1.10.1".to_string(),
                ast: Some(sample_ast()),
                tokens: Some(sample_tokens()),
            },
        );
        assert_eq!(get_in(h.cache_dir.path(), &h.source_path, h.source), None);

        ensure_primed_in(
            h.cache_dir.path(),
            &h.source_path,
            h.source,
            COMPATIBLE_VERSION,
        );

        assert_eq!(
            get_in(h.cache_dir.path(), &h.source_path, h.source),
            Some(papyrus_parser::parse(h.source).unwrap())
        );
        assert_eq!(
            get_tokens_in(h.cache_dir.path(), &h.source_path, h.source),
            Some(papyrus_parser::tokenize(h.source).unwrap())
        );
    }

    #[test]
    fn ensure_primed_writes_nothing_when_the_source_does_not_tokenize() {
        let h = harness("Unlexable.psc", "ScriptName Broken\nString s = \"oops\n");
        assert!(papyrus_parser::tokenize(h.source).is_err());
        assert!(papyrus_parser::parse(h.source).is_err());

        ensure_primed_in(
            h.cache_dir.path(),
            &h.source_path,
            h.source,
            COMPATIBLE_VERSION,
        );

        assert_eq!(get_in(h.cache_dir.path(), &h.source_path, h.source), None);
        assert_eq!(
            get_tokens_in(h.cache_dir.path(), &h.source_path, h.source),
            None
        );
        assert!(!cache_file_path(h.cache_dir.path(), &h.source_path).exists());
    }

    #[test]
    fn extra_json_fields_do_not_invalidate_a_fresh_entry() {
        let h = harness("ExtraFields.psc", "ScriptName ExtraFields\n");
        let ast = papyrus_parser::parse(h.source).unwrap();
        put_in(
            h.cache_dir.path(),
            &h.source_path,
            h.source,
            &ast,
            COMPATIBLE_VERSION,
        );

        let file = cache_file_path(h.cache_dir.path(), &h.source_path);
        let mut value: serde_json::Value =
            serde_json::from_slice(&std::fs::read(&file).unwrap()).unwrap();
        value
            .as_object_mut()
            .unwrap()
            .insert("future_field".to_string(), serde_json::json!("ok"));
        std::fs::write(&file, serde_json::to_vec(&value).unwrap()).unwrap();

        assert_eq!(
            get_in(h.cache_dir.path(), &h.source_path, h.source),
            Some(ast)
        );
    }

    #[test]
    fn public_ensure_primed_does_not_panic() {
        let project_dir = tempdir().unwrap();
        let source_path = project_dir.path().join("PublicEnsurePrimed.psc");
        let source = "ScriptName PublicEnsurePrimed\n";
        std::fs::write(&source_path, source).unwrap();

        crate::ensure_primed(&source_path, source);
        assert_eq!(
            crate::get(&source_path, source),
            Some(papyrus_parser::parse(source).unwrap())
        );
        assert_eq!(
            crate::get_tokens(&source_path, source),
            Some(papyrus_parser::tokenize(source).unwrap())
        );
    }

    #[test]
    fn public_put_then_get_returns_the_cached_ast() {
        let project_dir = tempdir().unwrap();
        let source_path = project_dir.path().join("PublicRoundtripAst.psc");
        let source = "ScriptName PublicRoundtripAst\n";
        std::fs::write(&source_path, source).unwrap();

        let ast = papyrus_parser::parse(source).unwrap();
        crate::put(&source_path, source, &ast);
        assert_eq!(crate::get(&source_path, source), Some(ast));
    }

    #[test]
    fn public_put_tokens_then_get_tokens_returns_the_cached_tokens() {
        let project_dir = tempdir().unwrap();
        let source_path = project_dir.path().join("PublicRoundtripTokens.psc");
        let source = "ScriptName PublicRoundtripTokens\n";
        std::fs::write(&source_path, source).unwrap();

        let tokens = papyrus_parser::tokenize(source).unwrap();
        crate::put_tokens(&source_path, source, &tokens);
        assert_eq!(crate::get_tokens(&source_path, source), Some(tokens));
    }

    #[test]
    fn public_accessors_are_safe_under_concurrent_use() {
        let project_dir = tempdir().unwrap();
        let source_path = project_dir.path().join("Concurrent.psc");
        let source = "ScriptName Concurrent\n";
        std::fs::write(&source_path, source).unwrap();
        let ast = papyrus_parser::parse(source).unwrap();
        let tokens = papyrus_parser::tokenize(source).unwrap();

        std::thread::scope(|scope| {
            for _ in 0..8 {
                let source_path = &source_path;
                let ast = &ast;
                let tokens = &tokens;
                scope.spawn(move || {
                    crate::put(source_path, source, ast);
                    crate::put_tokens(source_path, source, tokens);
                    let _ = crate::get(source_path, source);
                    let _ = crate::get_tokens(source_path, source);
                    crate::ensure_primed(source_path, source);
                });
            }
        });

        assert_eq!(crate::get(&source_path, source), Some(ast));
        assert_eq!(crate::get_tokens(&source_path, source), Some(tokens));
    }
}
