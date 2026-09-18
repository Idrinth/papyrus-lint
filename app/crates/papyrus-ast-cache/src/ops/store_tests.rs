use super::*;
use crate::entry::{cache_file_path, file_modified_unix_secs, CacheEntry};
use crate::ops::load::{get_in, get_tokens_in};
use crate::ops::test_support::{harness, sample_ast, sample_tokens, write_raw, COMPATIBLE_VERSION};
use tempfile::tempdir;

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
        papyrus_parser::tokenize("ScriptName Example\n\nInt Property Marker = 1 Auto\n").unwrap();
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

    let raw = std::fs::read_to_string(cache_file_path(h.cache_dir.path(), &h.source_path)).unwrap();
    assert!(raw.contains("\"linter_version\":\"9.9.9\""));
    assert!(!raw.contains(&format!("\"linter_version\":\"{COMPATIBLE_VERSION}\"")));
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
