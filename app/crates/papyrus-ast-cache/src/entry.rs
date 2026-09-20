//! On-disk representation of a cache entry: where it lives, how it's
//! addressed, and the raw read/write of it. [`crate::ops`] builds the
//! actual `get`/`put` semantics on top of these primitives.

use std::path::{Path, PathBuf};
use std::time::UNIX_EPOCH;

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

/// The cache file `source_path` is stored under within `dir`: an MD5 of its
/// absolute path, so path separators and length can't collide with
/// filesystem naming limits.
pub(crate) fn cache_file_path(dir: &Path, source_path: &Path) -> PathBuf {
    let digest = md5::compute(source_path.to_string_lossy().as_bytes());
    dir.join(format!("{digest:x}.json"))
}

pub(crate) fn cache_file_path_for_game(dir: &Path, game: &str, source_path: &Path) -> PathBuf {
    let digest = md5::compute(source_path.to_string_lossy().as_bytes());
    dir.join(format!("{game}-{digest:x}.json"))
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
#[cfg(test)]
pub(crate) fn valid_entry_in(dir: &Path, source_path: &Path, source: &str) -> Option<CacheEntry> {
    let raw = std::fs::read(cache_file_path(dir, source_path)).ok()?;
    deserialize_fresh_entry(raw, source_path, source)
}

pub(crate) fn valid_entry_in_for_game(
    dir: &Path,
    game: &str,
    source_path: &Path,
    source: &str,
) -> Option<CacheEntry> {
    let expected = cache_file_path_for_game(dir, game, source_path);
    if let Ok(raw) = std::fs::read(&expected) {
        return deserialize_fresh_entry(raw, source_path, source);
    }
    if game != "skyrim" {
        return None;
    }
    let legacy = cache_file_path(dir, source_path);
    let entry = deserialize_fresh_entry(std::fs::read(&legacy).ok()?, source_path, source)?;
    let _ = std::fs::rename(legacy, expected);
    Some(entry)
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

#[cfg(test)]
pub(crate) fn write_entry_in(dir: &Path, source_path: &Path, entry: &CacheEntry) {
    let Ok(serialized) = serde_json::to_vec(entry) else {
        return;
    };
    if std::fs::create_dir_all(dir).is_err() {
        return;
    }
    let _ = std::fs::write(cache_file_path(dir, source_path), serialized);
}

pub(crate) fn write_entry_in_for_game(
    dir: &Path,
    game: &str,
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
mod tests {
    use super::*;
    use crate::version::MIN_COMPATIBLE_VERSION;
    use std::time::{Duration, UNIX_EPOCH};
    use tempfile::tempdir;

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
        let path = cache_file_path(dir, Path::new("/mods/Scripts/Example.psc"));
        let name = path.file_name().unwrap().to_str().unwrap();
        assert!(name.ends_with(".json"));
        let digest = name.trim_end_matches(".json");
        assert_eq!(digest.len(), 32);
        assert!(digest.chars().all(|c| c.is_ascii_hexdigit()));
        assert_eq!(path.parent(), Some(dir));
    }

    #[test]
    fn cache_file_path_is_stable_for_the_same_source_path() {
        let dir = Path::new("/tmp/ast-cache");
        let source = Path::new("/mods/Scripts/Example.psc");
        assert_eq!(cache_file_path(dir, source), cache_file_path(dir, source));
    }

    #[test]
    fn game_cache_file_path_prefixes_the_path_digest() {
        let dir = Path::new("/tmp/ast-cache");
        let source = Path::new("/mods/Scripts/Example.psc");
        let name = cache_file_path_for_game(dir, "skyrim", source)
            .file_name()
            .unwrap()
            .to_str()
            .unwrap()
            .to_string();
        assert!(name.starts_with("skyrim-"));
        assert_eq!(name.len(), "skyrim-".len() + 32 + ".json".len());
    }

    #[test]
    fn skyrim_read_migrates_a_legacy_cache_file() {
        let cache_dir = tempdir().unwrap();
        let project_dir = tempdir().unwrap();
        let source_path = project_dir.path().join("Example.psc");
        let source = "ScriptName Example\n";
        std::fs::write(&source_path, source).unwrap();
        let legacy = cache_file_path(cache_dir.path(), &source_path);
        write_entry_in(
            cache_dir.path(),
            &source_path,
            &fresh_entry(&source_path, source),
        );

        assert!(
            valid_entry_in_for_game(cache_dir.path(), "skyrim", &source_path, source).is_some()
        );
        assert!(!legacy.exists());
        assert!(cache_file_path_for_game(cache_dir.path(), "skyrim", &source_path).exists());
    }

    #[test]
    fn another_game_does_not_consume_a_legacy_skyrim_cache_file() {
        let cache_dir = tempdir().unwrap();
        let project_dir = tempdir().unwrap();
        let source_path = project_dir.path().join("Example.psc");
        let source = "ScriptName Example\n";
        std::fs::write(&source_path, source).unwrap();
        let legacy = cache_file_path(cache_dir.path(), &source_path);
        write_entry_in(
            cache_dir.path(),
            &source_path,
            &fresh_entry(&source_path, source),
        );

        assert!(
            valid_entry_in_for_game(cache_dir.path(), "fallout4", &source_path, source).is_none()
        );
        assert!(legacy.exists());
    }

    #[test]
    fn cache_file_path_keys_on_the_path_string_not_the_inode() {
        let dir = Path::new("/tmp/ast-cache");
        let absolute = Path::new("/mods/Scripts/Example.psc");
        let relative = Path::new("Example.psc");
        assert_ne!(
            cache_file_path(dir, absolute),
            cache_file_path(dir, relative)
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

        let file = cache_file_path(cache_dir.path(), &source_path);
        std::fs::write(&file, b"").unwrap();
        assert!(valid_entry_in(cache_dir.path(), &source_path, source).is_none());

        std::fs::write(&file, b"{\"modified_unix_secs\":1").unwrap();
        assert!(valid_entry_in(cache_dir.path(), &source_path, source).is_none());

        std::fs::write(&file, b"[]").unwrap();
        assert!(valid_entry_in(cache_dir.path(), &source_path, source).is_none());
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
        // `get_in` already treat `ast: None` as a miss.
        let raw = format!(
            r#"{{"modified_unix_secs":{},"content_md5":"{:x}","linter_version":"{}"}}"#,
            file_modified_unix_secs(&source_path).unwrap(),
            md5::compute(source.as_bytes()),
            MIN_COMPATIBLE_VERSION,
        );
        std::fs::write(cache_file_path(cache_dir.path(), &source_path), raw).unwrap();

        let entry = valid_entry_in(cache_dir.path(), &source_path, source).unwrap();
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
            cache_file_path(cache_dir.path(), &source_path),
            serde_json::to_vec(&value).unwrap(),
        )
        .unwrap();

        let entry = valid_entry_in(cache_dir.path(), &source_path, source).unwrap();
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
        write_entry_in(cache_dir.path(), &source_path, &entry);

        let loaded = valid_entry_in(cache_dir.path(), &source_path, source).unwrap();
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
        write_entry_in(cache_dir.path(), &source_path, &entry);
        assert!(valid_entry_in(cache_dir.path(), &source_path, source).is_none());

        let mut entry = fresh_entry(&source_path, source);
        entry.content_md5 = format!("{:x}", md5::compute(b"different source"));
        write_entry_in(cache_dir.path(), &source_path, &entry);
        assert!(valid_entry_in(cache_dir.path(), &source_path, source).is_none());

        let mut entry = fresh_entry(&source_path, source);
        entry.modified_unix_secs = entry.modified_unix_secs.saturating_add(1);
        write_entry_in(cache_dir.path(), &source_path, &entry);
        assert!(valid_entry_in(cache_dir.path(), &source_path, source).is_none());
    }

    #[test]
    fn valid_entry_in_is_a_miss_when_the_cache_file_is_missing() {
        let cache_dir = tempdir().unwrap();
        let project_dir = tempdir().unwrap();
        let source_path = project_dir.path().join("Example.psc");
        let source = "ScriptName Example\n";
        std::fs::write(&source_path, source).unwrap();

        assert!(valid_entry_in(cache_dir.path(), &source_path, source).is_none());
    }

    #[test]
    fn write_entry_in_round_trips_a_valid_entry() {
        let cache_dir = tempdir().unwrap();
        let project_dir = tempdir().unwrap();
        let source_path = project_dir.path().join("Example.psc");
        let source = "ScriptName Example\n";
        std::fs::write(&source_path, source).unwrap();

        let entry = fresh_entry(&source_path, source);
        write_entry_in(cache_dir.path(), &source_path, &entry);

        let loaded = valid_entry_in(cache_dir.path(), &source_path, source).unwrap();
        assert_eq!(loaded.modified_unix_secs, entry.modified_unix_secs);
        assert_eq!(loaded.content_md5, entry.content_md5);
        assert_eq!(loaded.linter_version, entry.linter_version);
        assert_eq!(loaded.ast, entry.ast);
        assert_eq!(loaded.tokens, entry.tokens);
    }

    #[test]
    fn unicode_source_paths_get_their_own_cache_file() {
        let dir = Path::new("/tmp/ast-cache");
        let ascii = cache_file_path(dir, Path::new("/mods/Scripts/Example.psc"));
        let unicode = cache_file_path(dir, Path::new("/mods/Scripts/Привет.psc"));
        assert_ne!(ascii, unicode);
        let name = unicode.file_name().unwrap().to_str().unwrap();
        assert!(name.ends_with(".json"));
        assert_eq!(name.trim_end_matches(".json").len(), 32);
    }
}
