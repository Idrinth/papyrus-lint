use super::*;
use crate::entry::{cache_file_path_for_game, file_modified_unix_secs, CacheEntry};
use crate::ops::store::{put_in_for_game, put_tokens_in_for_game};
use crate::ops::test_support::{
    harness, sample_ast, sample_tokens, write_raw, COMPATIBLE_VERSION, GAME,
};
use tempfile::tempdir;

#[test]
fn get_is_a_hit_when_the_cached_version_is_newer_than_the_minimum_compatible_version() {
    let cache_dir = tempdir().unwrap();
    let project_dir = tempdir().unwrap();
    let source_path = project_dir.path().join("Example.psc");
    let source = "ScriptName Example\n";
    std::fs::write(&source_path, source).unwrap();

    let ast = sample_ast();
    put_in_for_game(cache_dir.path(), GAME, &source_path, source, &ast, "9.9.9");

    assert_eq!(
        get_in_for_game(cache_dir.path(), GAME, &source_path, source),
        Some(ast)
    );
}

#[test]
fn get_is_a_miss_for_an_uncached_path() {
    let cache_dir = tempdir().unwrap();
    let project_dir = tempdir().unwrap();
    let source_path = project_dir.path().join("Example.psc");
    std::fs::write(&source_path, "ScriptName Example\n").unwrap();

    assert_eq!(
        get_in_for_game(cache_dir.path(), GAME, &source_path, "ScriptName Example\n"),
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

    put_in_for_game(
        cache_dir.path(),
        GAME,
        &source_path,
        original,
        &sample_ast(),
        COMPATIBLE_VERSION,
    );

    let changed = "ScriptName Renamed\n";
    assert_eq!(
        get_in_for_game(cache_dir.path(), GAME, &source_path, changed),
        None
    );
}

#[test]
fn get_is_a_miss_when_the_file_was_modified_after_caching() {
    let cache_dir = tempdir().unwrap();
    let project_dir = tempdir().unwrap();
    let source_path = project_dir.path().join("Example.psc");
    let source = "ScriptName Example\n";
    std::fs::write(&source_path, source).unwrap();

    put_in_for_game(
        cache_dir.path(),
        GAME,
        &source_path,
        source,
        &sample_ast(),
        COMPATIBLE_VERSION,
    );

    let filetime_now = std::time::SystemTime::now() + std::time::Duration::from_secs(120);
    std::fs::write(&source_path, source).unwrap();
    let file = std::fs::File::open(&source_path).unwrap();
    file.set_modified(filetime_now).unwrap();

    assert_eq!(
        get_in_for_game(cache_dir.path(), GAME, &source_path, source),
        None
    );
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
        cache_file_path_for_game(cache_dir.path(), GAME, &source_path),
        serde_json::to_vec(&entry).unwrap(),
    )
    .unwrap();

    assert_eq!(
        get_in_for_game(cache_dir.path(), GAME, &source_path, source),
        None
    );
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
        cache_file_path_for_game(cache_dir.path(), GAME, &source_path),
        serde_json::to_vec(&entry).unwrap(),
    )
    .unwrap();

    assert_eq!(
        get_in_for_game(cache_dir.path(), GAME, &source_path, source),
        None
    );
}

#[test]
fn get_is_a_miss_on_malformed_cache_contents() {
    let cache_dir = tempdir().unwrap();
    let project_dir = tempdir().unwrap();
    let source_path = project_dir.path().join("Example.psc");
    std::fs::write(&source_path, "ScriptName Example\n").unwrap();

    std::fs::create_dir_all(cache_dir.path()).unwrap();
    std::fs::write(
        cache_file_path_for_game(cache_dir.path(), GAME, &source_path),
        b"not json",
    )
    .unwrap();

    assert_eq!(
        get_in_for_game(cache_dir.path(), GAME, &source_path, "ScriptName Example\n"),
        None
    );
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
    put_in_for_game(
        cache_dir.path(),
        GAME,
        &path_a,
        "ScriptName A\n",
        &ast_a,
        COMPATIBLE_VERSION,
    );
    put_in_for_game(
        cache_dir.path(),
        GAME,
        &path_b,
        "ScriptName B\n",
        &ast_b,
        COMPATIBLE_VERSION,
    );

    assert_eq!(
        get_in_for_game(cache_dir.path(), GAME, &path_a, "ScriptName A\n"),
        Some(ast_a)
    );
    assert_eq!(
        get_in_for_game(cache_dir.path(), GAME, &path_b, "ScriptName B\n"),
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
    put_in_for_game(
        cache_dir.path(),
        GAME,
        &path_a,
        source_a,
        &ast_a,
        COMPATIBLE_VERSION,
    );
    put_in_for_game(
        cache_dir.path(),
        GAME,
        &path_b,
        source_b,
        &ast_b,
        COMPATIBLE_VERSION,
    );

    assert_ne!(
        cache_file_path_for_game(cache_dir.path(), GAME, &path_a),
        cache_file_path_for_game(cache_dir.path(), GAME, &path_b)
    );
    assert_eq!(
        get_in_for_game(cache_dir.path(), GAME, &path_a, source_a),
        Some(ast_a)
    );
    assert_eq!(
        get_in_for_game(cache_dir.path(), GAME, &path_b, source_b),
        Some(ast_b)
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
    // subsequent `papyrus_parser::parse(source)` call returning it proves
    // it came from `get_in_for_game`'s in-memory priming rather than a fresh parse
    // of `source`.
    let distinct_ast = papyrus_parser::parse(
        "ScriptName PrimesInMemory extends Quest\n\nInt Property Marker = 1 Auto\n",
    )
    .unwrap();
    put_in_for_game(
        cache_dir.path(),
        GAME,
        &source_path,
        source,
        &distinct_ast,
        COMPATIBLE_VERSION,
    );

    let cached = get_in_for_game(cache_dir.path(), GAME, &source_path, source).unwrap();
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
    // proves it came from `get_tokens_in_for_game`'s in-memory priming rather than a
    // fresh tokenize of `source`.
    let distinct_tokens = papyrus_parser::tokenize(
        "ScriptName PrimesTokensInMemory extends Quest\n\nInt Property Marker = 1 Auto\n",
    )
    .unwrap();
    put_tokens_in_for_game(
        cache_dir.path(),
        GAME,
        &source_path,
        source,
        &distinct_tokens,
        COMPATIBLE_VERSION,
    );

    let cached = get_tokens_in_for_game(cache_dir.path(), GAME, &source_path, source).unwrap();
    assert_eq!(cached, distinct_tokens);
    assert_eq!(papyrus_parser::tokenize(source).unwrap(), distinct_tokens);
}

#[test]
fn get_is_a_miss_when_the_source_file_was_deleted() {
    let cache_dir = tempdir().unwrap();
    let project_dir = tempdir().unwrap();
    let source_path = project_dir.path().join("Example.psc");
    let source = "ScriptName Example\n";
    std::fs::write(&source_path, source).unwrap();
    put_in_for_game(
        cache_dir.path(),
        GAME,
        &source_path,
        source,
        &sample_ast(),
        COMPATIBLE_VERSION,
    );
    std::fs::remove_file(&source_path).unwrap();

    assert_eq!(
        get_in_for_game(cache_dir.path(), GAME, &source_path, source),
        None
    );
}

#[test]
fn get_tokens_is_a_miss_for_an_uncached_path() {
    let cache_dir = tempdir().unwrap();
    let project_dir = tempdir().unwrap();
    let source_path = project_dir.path().join("Example.psc");
    std::fs::write(&source_path, "ScriptName Example\n").unwrap();

    assert_eq!(
        get_tokens_in_for_game(cache_dir.path(), GAME, &source_path, "ScriptName Example\n"),
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

    put_tokens_in_for_game(
        cache_dir.path(),
        GAME,
        &source_path,
        original,
        &sample_tokens(),
        COMPATIBLE_VERSION,
    );

    let changed = "ScriptName Renamed\n";
    assert_eq!(
        get_tokens_in_for_game(cache_dir.path(), GAME, &source_path, changed),
        None
    );
}

#[test]
fn get_tokens_is_a_miss_when_the_file_was_modified_after_caching() {
    let cache_dir = tempdir().unwrap();
    let project_dir = tempdir().unwrap();
    let source_path = project_dir.path().join("Example.psc");
    let source = "ScriptName Example\n";
    std::fs::write(&source_path, source).unwrap();

    put_tokens_in_for_game(
        cache_dir.path(),
        GAME,
        &source_path,
        source,
        &sample_tokens(),
        COMPATIBLE_VERSION,
    );

    let changed_time = std::time::SystemTime::now() + std::time::Duration::from_secs(120);
    let file = std::fs::File::open(&source_path).unwrap();
    file.set_modified(changed_time).unwrap();

    assert_eq!(
        get_tokens_in_for_game(cache_dir.path(), GAME, &source_path, source),
        None
    );
}

#[test]
fn get_tokens_is_a_miss_when_the_source_file_was_deleted() {
    let cache_dir = tempdir().unwrap();
    let project_dir = tempdir().unwrap();
    let source_path = project_dir.path().join("Example.psc");
    let source = "ScriptName Example\n";
    std::fs::write(&source_path, source).unwrap();
    put_tokens_in_for_game(
        cache_dir.path(),
        GAME,
        &source_path,
        source,
        &sample_tokens(),
        COMPATIBLE_VERSION,
    );
    std::fs::remove_file(&source_path).unwrap();

    assert_eq!(
        get_tokens_in_for_game(cache_dir.path(), GAME, &source_path, source),
        None
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
    std::fs::write(
        cache_file_path_for_game(cache_dir.path(), GAME, &source_path),
        raw,
    )
    .unwrap();

    assert_eq!(
        get_in_for_game(cache_dir.path(), GAME, &source_path, source),
        Some(sample_ast())
    );
    assert_eq!(
        get_tokens_in_for_game(cache_dir.path(), GAME, &source_path, source),
        None
    );
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
        get_tokens_in_for_game(h.cache_dir.path(), GAME, &h.source_path, h.source),
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
        get_tokens_in_for_game(h.cache_dir.path(), GAME, &h.source_path, h.source),
        None
    );
}

#[test]
fn get_tokens_is_a_miss_on_malformed_cache_contents() {
    let h = harness("Example.psc", "ScriptName Example\n");
    std::fs::create_dir_all(h.cache_dir.path()).unwrap();
    std::fs::write(
        cache_file_path_for_game(h.cache_dir.path(), GAME, &h.source_path),
        b"not json",
    )
    .unwrap();

    assert_eq!(
        get_tokens_in_for_game(h.cache_dir.path(), GAME, &h.source_path, h.source),
        None
    );
}

#[test]
fn get_tokens_is_a_hit_when_the_cached_version_is_newer_than_the_minimum() {
    let h = harness("Example.psc", "ScriptName Example\n");
    let tokens = sample_tokens();
    put_tokens_in_for_game(
        h.cache_dir.path(),
        GAME,
        &h.source_path,
        h.source,
        &tokens,
        "9.9.9",
    );

    assert_eq!(
        get_tokens_in_for_game(h.cache_dir.path(), GAME, &h.source_path, h.source),
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

    assert_eq!(
        get_in_for_game(h.cache_dir.path(), GAME, &h.source_path, h.source),
        None
    );
    assert_eq!(
        get_tokens_in_for_game(h.cache_dir.path(), GAME, &h.source_path, h.source),
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
        get_in_for_game(h.cache_dir.path(), GAME, &h.source_path, h.source),
        Some(ast)
    );
    assert_eq!(
        get_tokens_in_for_game(h.cache_dir.path(), GAME, &h.source_path, h.source),
        None
    );
}

#[test]
fn get_is_a_miss_when_the_cache_file_is_a_directory() {
    let h = harness("Example.psc", "ScriptName Example\n");
    std::fs::create_dir_all(cache_file_path_for_game(
        h.cache_dir.path(),
        GAME,
        &h.source_path,
    ))
    .unwrap();

    assert_eq!(
        get_in_for_game(h.cache_dir.path(), GAME, &h.source_path, h.source),
        None
    );
    assert_eq!(
        get_tokens_in_for_game(h.cache_dir.path(), GAME, &h.source_path, h.source),
        None
    );
}

#[test]
fn extra_json_fields_do_not_invalidate_a_fresh_entry() {
    let h = harness("ExtraFields.psc", "ScriptName ExtraFields\n");
    let ast = papyrus_parser::parse(h.source).unwrap();
    put_in_for_game(
        h.cache_dir.path(),
        GAME,
        &h.source_path,
        h.source,
        &ast,
        COMPATIBLE_VERSION,
    );

    let file = cache_file_path_for_game(h.cache_dir.path(), GAME, &h.source_path);
    let mut value: serde_json::Value =
        serde_json::from_slice(&std::fs::read(&file).unwrap()).unwrap();
    value
        .as_object_mut()
        .unwrap()
        .insert("future_field".to_string(), serde_json::json!("ok"));
    std::fs::write(&file, serde_json::to_vec(&value).unwrap()).unwrap();

    assert_eq!(
        get_in_for_game(h.cache_dir.path(), GAME, &h.source_path, h.source),
        Some(ast)
    );
}
