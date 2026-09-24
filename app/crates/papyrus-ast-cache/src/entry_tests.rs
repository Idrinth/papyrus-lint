use super::*;
use crate::version::MIN_COMPATIBLE_VERSION;
use std::time::{Duration, UNIX_EPOCH};
use tempfile::tempdir;

const GAME: papyrus_lint_globals::Game = papyrus_lint_globals::Game::Skyrim;

fn sample_ast() -> papyrus_parser::ast::Script {
    papyrus_parser::parse("ScriptName Example\n").unwrap()
}

fn sample_tokens() -> Vec<papyrus_parser::token::Token> {
    papyrus_parser::tokenize("ScriptName Example\n").unwrap()
}

fn fresh_entry(source_path: &Path, source: &str) -> CacheEntry {
    CacheEntry {
        modified_unix_secs: file_modified_unix_secs(source_path).unwrap(),
        content_md5: format!("{:x}", md5::compute(source.as_bytes())),
        linter_version: MIN_COMPATIBLE_VERSION.to_string(),
        ast: Some(sample_ast()),
        tokens: Some(sample_tokens()),
    }
}

#[test]
fn cache_dir_is_ast_cache_next_to_the_running_executable() {
    let exe = PathBuf::from("/opt/papyrus/PapyrusLinterCLI");
    assert_eq!(
        cache_dir_from(None, Some(exe)),
        Some(PathBuf::from("/opt/papyrus/ast-cache"))
    );
}

#[test]
fn cache_dir_honors_environment_override() {
    assert_eq!(
        cache_dir_from(Some("/cache".into()), None),
        Some(PathBuf::from("/cache"))
    );
}

#[test]
fn cache_dir_ignores_an_empty_environment_override() {
    let exe = PathBuf::from("/opt/papyrus/PapyrusLinterCLI");
    assert_eq!(
        cache_dir_from(Some("".into()), Some(exe)),
        Some(PathBuf::from("/opt/papyrus/ast-cache"))
    );
}

#[test]
fn cache_dir_is_none_when_no_override_or_executable_is_available() {
    assert_eq!(cache_dir_from(None, None), None);
}

#[test]
fn cache_file_path_is_a_32_hex_digit_json_file() {
    let dir = Path::new("/tmp/ast-cache");
    let path = cache_file_path_for_game(dir, GAME, Path::new("/mods/Scripts/Example.psc"));
    let name = path.file_name().unwrap().to_str().unwrap();
    let (game, digest) = name.trim_end_matches(".json").split_once('-').unwrap();
    assert_eq!(game, "skyrim");
    assert_eq!(digest.len(), 32);
    assert!(digest.chars().all(|c| c.is_ascii_hexdigit()));
    assert_eq!(path.parent(), Some(dir));
}

#[test]
fn cache_file_path_is_stable_for_the_same_source_path() {
    let dir = Path::new("/tmp/ast-cache");
    let source = Path::new("/mods/Scripts/Example.psc");
    assert_eq!(
        cache_file_path_for_game(dir, GAME, source),
        cache_file_path_for_game(dir, GAME, source)
    );
}

#[test]
fn game_cache_file_path_prefixes_the_path_digest() {
    let dir = Path::new("/tmp/ast-cache");
    let source = Path::new("/mods/Scripts/Example.psc");
    let name = cache_file_path_for_game(dir, papyrus_lint_globals::Game::Skyrim, source)
        .file_name()
        .unwrap()
        .to_str()
        .unwrap()
        .to_string();
    assert!(name.starts_with("skyrim-"));
    assert_eq!(name.len(), "skyrim-".len() + 32 + ".json".len());
}

#[test]
fn game_read_does_not_consume_a_prefixless_cache_file() {
    let cache_dir = tempdir().unwrap();
    let project_dir = tempdir().unwrap();
    let source_path = project_dir.path().join("Example.psc");
    let source = "ScriptName Example\n";
    std::fs::write(&source_path, source).unwrap();
    // Pre-game cache files were `{path-md5}.json` with no game prefix.
    // A lookup always has a game now, and must not fall back to that name.
    let digest = md5::compute(source_path.to_string_lossy().as_bytes());
    let legacy = cache_dir.path().join(format!("{digest:x}.json"));
    std::fs::create_dir_all(cache_dir.path()).unwrap();
    std::fs::write(
        &legacy,
        serde_json::to_vec(&fresh_entry(&source_path, source)).unwrap(),
    )
    .unwrap();

    assert!(valid_entry_in_for_game(
        cache_dir.path(),
        papyrus_lint_globals::Game::Skyrim,
        &source_path,
        source,
    )
    .is_none());
    assert!(legacy.exists());
}

#[test]
fn cache_file_path_keys_on_the_path_string_not_the_inode() {
    let dir = Path::new("/tmp/ast-cache");
    let absolute = Path::new("/mods/Scripts/Example.psc");
    let relative = Path::new("Example.psc");
    assert_ne!(
        cache_file_path_for_game(dir, GAME, absolute),
        cache_file_path_for_game(dir, GAME, relative)
    );
}

#[test]
fn file_modified_unix_secs_is_none_for_a_missing_path() {
    assert_eq!(
        file_modified_unix_secs(Path::new("/this/path/does/not/exist.psc")),
        None
    );
}

#[test]
fn file_modified_unix_secs_is_none_for_a_pre_epoch_mtime() {
    let dir = tempdir().unwrap();
    let path = dir.path().join("Example.psc");
    std::fs::write(&path, "ScriptName Example\n").unwrap();
    let file = std::fs::File::open(&path).unwrap();
    let pre_epoch = UNIX_EPOCH - Duration::from_secs(10);
    if file.set_modified(pre_epoch).is_ok() {
        assert_eq!(file_modified_unix_secs(&path), None);
    }
}

#[test]
fn valid_entry_in_is_a_miss_on_empty_or_truncated_json() {
    let cache_dir = tempdir().unwrap();
    let project_dir = tempdir().unwrap();
    let source_path = project_dir.path().join("Example.psc");
    let source = "ScriptName Example\n";
    std::fs::write(&source_path, source).unwrap();
    std::fs::create_dir_all(cache_dir.path()).unwrap();

    let file = cache_file_path_for_game(cache_dir.path(), GAME, &source_path);
    std::fs::write(&file, b"").unwrap();
    assert!(valid_entry_in_for_game(cache_dir.path(), GAME, &source_path, source).is_none());

    std::fs::write(&file, b"{\"modified_unix_secs\":1").unwrap();
    assert!(valid_entry_in_for_game(cache_dir.path(), GAME, &source_path, source).is_none());

    std::fs::write(&file, b"[]").unwrap();
    assert!(valid_entry_in_for_game(cache_dir.path(), GAME, &source_path, source).is_none());
}

#[test]
fn valid_entry_in_treats_a_missing_ast_field_as_none() {
    let cache_dir = tempdir().unwrap();
    let project_dir = tempdir().unwrap();
    let source_path = project_dir.path().join("Example.psc");
    let source = "ScriptName Example\n";
    std::fs::write(&source_path, source).unwrap();
    std::fs::create_dir_all(cache_dir.path()).unwrap();

    // Unlike a missing `tokens` field, `ast` is not marked
    // `#[serde(default)]`, but `Option` still deserializes a missing
    // field as `None` rather than failing the whole entry. Callers of
    // `get_in_for_game` already treat `ast: None` as a miss.
    let raw = format!(
        r#"{{"modified_unix_secs":{},"content_md5":"{:x}","linter_version":"{}"}}"#,
        file_modified_unix_secs(&source_path).unwrap(),
        md5::compute(source.as_bytes()),
        MIN_COMPATIBLE_VERSION,
    );
    std::fs::write(
        cache_file_path_for_game(cache_dir.path(), GAME, &source_path),
        raw,
    )
    .unwrap();

    let entry = valid_entry_in_for_game(cache_dir.path(), GAME, &source_path, source).unwrap();
    assert!(entry.ast.is_none());
    assert!(entry.tokens.is_none());
}

#[test]
fn valid_entry_in_ignores_unknown_fields() {
    let cache_dir = tempdir().unwrap();
    let project_dir = tempdir().unwrap();
    let source_path = project_dir.path().join("Example.psc");
    let source = "ScriptName Example\n";
    std::fs::write(&source_path, source).unwrap();

    let mut value = serde_json::to_value(fresh_entry(&source_path, source)).unwrap();
    value
        .as_object_mut()
        .unwrap()
        .insert("future_field".to_string(), serde_json::json!(true));
    std::fs::create_dir_all(cache_dir.path()).unwrap();
    std::fs::write(
        cache_file_path_for_game(cache_dir.path(), GAME, &source_path),
        serde_json::to_vec(&value).unwrap(),
    )
    .unwrap();

    let entry = valid_entry_in_for_game(cache_dir.path(), GAME, &source_path, source).unwrap();
    assert_eq!(entry.ast, Some(sample_ast()));
    assert_eq!(entry.tokens, Some(sample_tokens()));
}

#[test]
fn valid_entry_in_accepts_explicit_null_ast_and_tokens() {
    let cache_dir = tempdir().unwrap();
    let project_dir = tempdir().unwrap();
    let source_path = project_dir.path().join("Example.psc");
    let source = "ScriptName Example\n";
    std::fs::write(&source_path, source).unwrap();

    let entry = CacheEntry {
        ast: None,
        tokens: None,
        ..fresh_entry(&source_path, source)
    };
    write_entry_in_for_game(cache_dir.path(), GAME, &source_path, &entry);

    let loaded = valid_entry_in_for_game(cache_dir.path(), GAME, &source_path, source).unwrap();
    assert!(loaded.ast.is_none());
    assert!(loaded.tokens.is_none());
}

#[test]
fn valid_entry_in_rejects_each_stale_metadata_field() {
    let cache_dir = tempdir().unwrap();
    let project_dir = tempdir().unwrap();
    let source_path = project_dir.path().join("Example.psc");
    let source = "ScriptName Example\n";
    std::fs::write(&source_path, source).unwrap();

    let mut entry = fresh_entry(&source_path, source);
    entry.linter_version = "1.0.0".to_string();
    write_entry_in_for_game(cache_dir.path(), GAME, &source_path, &entry);
    assert!(valid_entry_in_for_game(cache_dir.path(), GAME, &source_path, source).is_none());

    let mut entry = fresh_entry(&source_path, source);
    entry.content_md5 = format!("{:x}", md5::compute(b"different source"));
    write_entry_in_for_game(cache_dir.path(), GAME, &source_path, &entry);
    assert!(valid_entry_in_for_game(cache_dir.path(), GAME, &source_path, source).is_none());

    let mut entry = fresh_entry(&source_path, source);
    entry.modified_unix_secs = entry.modified_unix_secs.saturating_add(1);
    write_entry_in_for_game(cache_dir.path(), GAME, &source_path, &entry);
    assert!(valid_entry_in_for_game(cache_dir.path(), GAME, &source_path, source).is_none());
}

#[test]
fn valid_entry_in_is_a_miss_when_the_cache_file_is_missing() {
    let cache_dir = tempdir().unwrap();
    let project_dir = tempdir().unwrap();
    let source_path = project_dir.path().join("Example.psc");
    let source = "ScriptName Example\n";
    std::fs::write(&source_path, source).unwrap();

    assert!(valid_entry_in_for_game(cache_dir.path(), GAME, &source_path, source).is_none());
}

#[test]
fn write_entry_in_round_trips_a_valid_entry() {
    let cache_dir = tempdir().unwrap();
    let project_dir = tempdir().unwrap();
    let source_path = project_dir.path().join("Example.psc");
    let source = "ScriptName Example\n";
    std::fs::write(&source_path, source).unwrap();

    let entry = fresh_entry(&source_path, source);
    write_entry_in_for_game(cache_dir.path(), GAME, &source_path, &entry);

    let loaded = valid_entry_in_for_game(cache_dir.path(), GAME, &source_path, source).unwrap();
    assert_eq!(loaded.modified_unix_secs, entry.modified_unix_secs);
    assert_eq!(loaded.content_md5, entry.content_md5);
    assert_eq!(loaded.linter_version, entry.linter_version);
    assert_eq!(loaded.ast, entry.ast);
    assert_eq!(loaded.tokens, entry.tokens);
}

#[test]
fn game_entry_round_trips_without_being_visible_to_another_game() {
    let cache_dir = tempdir().unwrap();
    let project_dir = tempdir().unwrap();
    let source_path = project_dir.path().join("Example.psc");
    let source = "ScriptName Example\n";
    std::fs::write(&source_path, source).unwrap();
    let entry = fresh_entry(&source_path, source);

    write_entry_in_for_game(
        cache_dir.path(),
        papyrus_lint_globals::Game::Skyrim,
        &source_path,
        &entry,
    );

    let loaded = valid_entry_in_for_game(
        cache_dir.path(),
        papyrus_lint_globals::Game::Skyrim,
        &source_path,
        source,
    )
    .unwrap();
    assert_eq!(loaded.ast, entry.ast);
    assert_eq!(loaded.tokens, entry.tokens);
    assert!(valid_entry_in_for_game(
        cache_dir.path(),
        papyrus_lint_globals::Game::Fallout4,
        &source_path,
        source,
    )
    .is_none());
}

#[test]
fn game_entry_write_ignores_an_unusable_cache_directory() {
    let cache_parent = tempdir().unwrap();
    let cache_dir = cache_parent.path().join("not-a-directory");
    std::fs::write(&cache_dir, "occupied").unwrap();
    let project_dir = tempdir().unwrap();
    let source_path = project_dir.path().join("Example.psc");
    let source = "ScriptName Example\n";
    std::fs::write(&source_path, source).unwrap();

    write_entry_in_for_game(
        &cache_dir,
        papyrus_lint_globals::Game::Skyrim,
        &source_path,
        &fresh_entry(&source_path, source),
    );

    assert!(cache_dir.is_file());
}

#[test]
fn unicode_source_paths_get_their_own_cache_file() {
    let dir = Path::new("/tmp/ast-cache");
    let ascii = cache_file_path_for_game(dir, GAME, Path::new("/mods/Scripts/Example.psc"));
    let unicode = cache_file_path_for_game(dir, GAME, Path::new("/mods/Scripts/Привет.psc"));
    assert_ne!(ascii, unicode);
    let name = unicode.file_name().unwrap().to_str().unwrap();
    let (game, digest) = name.trim_end_matches(".json").split_once('-').unwrap();
    assert_eq!(game, "skyrim");
    assert_eq!(digest.len(), 32);
}

#[test]
fn mtime_valid_entry_returns_the_stored_hash_without_source_text() {
    let cache_dir = tempdir().unwrap();
    let project_dir = tempdir().unwrap();
    let source_path = project_dir.path().join("Example.psc");
    let source = "ScriptName Example\n";
    std::fs::write(&source_path, source).unwrap();
    write_entry_in_for_game(
        cache_dir.path(),
        GAME,
        &source_path,
        &fresh_entry(&source_path, source),
    );

    let entry = mtime_valid_entry_in_for_game(cache_dir.path(), GAME, &source_path)
        .expect("mtime-fresh entry should be readable");
    assert_eq!(
        entry.content_md5,
        format!("{:x}", md5::compute(source.as_bytes()))
    );
}

#[test]
fn mtime_valid_entry_is_none_after_the_source_mtime_changes() {
    let cache_dir = tempdir().unwrap();
    let project_dir = tempdir().unwrap();
    let source_path = project_dir.path().join("Example.psc");
    let source = "ScriptName Example\n";
    std::fs::write(&source_path, source).unwrap();
    write_entry_in_for_game(
        cache_dir.path(),
        GAME,
        &source_path,
        &fresh_entry(&source_path, source),
    );

    let later = std::time::SystemTime::now() + std::time::Duration::from_secs(120);
    let file = std::fs::File::open(&source_path).unwrap();
    file.set_modified(later).unwrap();

    assert!(mtime_valid_entry_in_for_game(cache_dir.path(), GAME, &source_path).is_none());
}
