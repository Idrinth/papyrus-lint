use super::*;
use crate::entry::{cache_file_path, file_modified_unix_secs, CacheEntry};
use crate::ops::load::{get_in, get_tokens_in};
use crate::ops::store::put_in;
use crate::ops::test_support::{harness, sample_ast, sample_tokens, COMPATIBLE_VERSION};
use tempfile::tempdir;

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
    crate::ops::store::put_tokens_in(
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
    crate::ops::test_support::write_raw(
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
