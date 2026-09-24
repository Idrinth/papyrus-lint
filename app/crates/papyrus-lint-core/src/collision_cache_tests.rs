use super::*;
use papyrus_lint_globals::Game;
use tempfile::tempdir;

const GAME: Game = Game::Skyrim;

fn write_script(dir: &Path, name: &str, source: &str) -> PathBuf {
    let path = dir.join(name);
    std::fs::write(&path, source).unwrap();
    path
}

#[test]
fn remember_source_then_content_hash_returns_the_recorded_digest() {
    let cache = tempdir().unwrap();
    let project = tempdir().unwrap();
    let path = write_script(project.path(), "Example.psc", "ScriptName Example\n");
    remember_source_in(cache.path(), GAME, &path, "ScriptName Example\n");
    assert_eq!(
        content_hash_in(cache.path(), GAME, &path),
        Some(sha256_hex("ScriptName Example\n"))
    );
}

#[test]
fn flush_writes_the_documented_collision_document() {
    let cache = tempdir().unwrap();
    let project = tempdir().unwrap();
    let path = write_script(project.path(), "Example.psc", "ScriptName Example\n");
    remember_source_in(cache.path(), GAME, &path, "ScriptName Example\n");
    flush_in(cache.path());

    let file = collisions_path(cache.path(), GAME, "example.psc");
    let groups: Vec<ScriptCollisions> =
        serde_json::from_slice(&std::fs::read(&file).unwrap()).unwrap();
    assert_eq!(groups.len(), 1);
    assert_eq!(groups[0].scriptname, "example.psc");
    assert_eq!(groups[0].implementers.len(), 1);
    assert_eq!(groups[0].implementers[0].algorithm, ALGORITHM);
    assert_eq!(
        groups[0].implementers[0].hash,
        sha256_hex("ScriptName Example\n")
    );
    assert!(!groups[0].implementers[0].path.is_empty());
    assert!(!groups[0].implementers[0].mtime.is_empty());
}

#[test]
fn preload_reuses_a_flushed_entry_without_rehashing() {
    let cache = tempdir().unwrap();
    let project = tempdir().unwrap();
    let path = write_script(project.path(), "Shared.psc", "ScriptName Shared\n");
    remember_source_in(cache.path(), GAME, &path, "ScriptName Shared\n");
    flush_in(cache.path());

    // Drop in-memory state for this group by loading from disk in a fresh
    // lookup after clearing the loaded flag via a different cache dir, then
    // reading the real one again.
    let other = tempdir().unwrap();
    preload_in(other.path(), GAME, [&path]);
    preload_in(cache.path(), GAME, [&path]);
    assert_eq!(
        content_hash_in(cache.path(), GAME, &path),
        Some(sha256_hex("ScriptName Shared\n"))
    );
}

#[test]
fn stale_mtime_recomputes_the_hash_from_disk() {
    let cache = tempdir().unwrap();
    let project = tempdir().unwrap();
    let path = write_script(project.path(), "Stale.psc", "ScriptName Stale\n");
    remember_source_in(cache.path(), GAME, &path, "ScriptName Stale\n");
    flush_in(cache.path());

    std::thread::sleep(std::time::Duration::from_millis(1100));
    std::fs::write(&path, "ScriptName StaleV2\n").unwrap();
    assert_eq!(
        content_hash_in(cache.path(), GAME, &path),
        Some(sha256_hex("ScriptName StaleV2\n"))
    );
}

#[test]
fn sha256_of_empty_string_is_the_well_known_digest() {
    assert_eq!(
        sha256_hex(""),
        "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
    );
}
