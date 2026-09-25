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
    let groups = decode_collisions(&std::fs::read(&file).unwrap()).unwrap();
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
    assert!(file.extension().and_then(|ext| ext.to_str()) == Some("iplcc"));
    assert!(file.as_os_str().to_string_lossy().contains("skyrim-"));
}

#[test]
fn load_group_ignores_legacy_json_and_wrong_magic() {
    let cache = tempdir().unwrap();
    let project = tempdir().unwrap();
    let path = write_script(project.path(), "Legacy.psc", "ScriptName Legacy\n");
    let file = collisions_path(cache.path(), GAME, "legacy.psc");
    std::fs::write(&file, br#"[{"scriptname":"legacy.psc","implementers":[]}]"#).unwrap();
    preload_in(cache.path(), GAME, [&path]);
    assert_eq!(
        content_hash_in(cache.path(), GAME, &path),
        Some(sha256_hex("ScriptName Legacy\n"))
    );
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

#[test]
fn content_hash_reads_and_remembers_an_uncached_file() {
    let cache = tempdir().unwrap();
    let project = tempdir().unwrap();
    let path = write_script(project.path(), "Uncached.psc", "ScriptName Uncached\n");

    let hash = content_hash_in(cache.path(), GAME, &path).unwrap();
    assert_eq!(hash, sha256_hex("ScriptName Uncached\n"));

    flush_in(cache.path());
    let groups = decode_collisions(
        &std::fs::read(collisions_path(cache.path(), GAME, "uncached.psc")).unwrap(),
    )
    .unwrap();
    assert_eq!(groups[0].implementers[0].hash, hash);
}

#[test]
fn content_hash_returns_none_for_a_missing_file() {
    let cache = tempdir().unwrap();
    let project = tempdir().unwrap();

    assert_eq!(
        content_hash_in(cache.path(), GAME, &project.path().join("Missing.psc")),
        None
    );
}

#[test]
fn a_fresh_disk_entry_is_used_without_reading_source_contents() {
    let cache = tempdir().unwrap();
    let project = tempdir().unwrap();
    let path = write_script(project.path(), "Cached.psc", "ScriptName Cached\n");
    let cached_hash = "digest-provided-by-the-cache";
    let groups = vec![ScriptCollisions {
        scriptname: "CACHED.PSC".to_string(),
        implementers: vec![Implementer {
            hash: cached_hash.to_string(),
            algorithm: ALGORITHM.to_string(),
            mtime: file_mtime_secs(&path).unwrap().to_string(),
            path: stored_path(&path),
        }],
    }];
    std::fs::write(
        collisions_path(cache.path(), GAME, "cached.psc"),
        encode_collisions(&groups).unwrap(),
    )
    .unwrap();

    preload_in(cache.path(), GAME, [&path]);

    assert_eq!(
        content_hash_in(cache.path(), GAME, &path),
        Some(cached_hash.to_string())
    );
}

#[test]
fn wrong_algorithm_in_disk_entry_causes_source_to_be_hashed() {
    let cache = tempdir().unwrap();
    let project = tempdir().unwrap();
    let source = "ScriptName Algorithm\n";
    let path = write_script(project.path(), "Algorithm.psc", source);
    let groups = vec![ScriptCollisions {
        scriptname: "algorithm.psc".to_string(),
        implementers: vec![Implementer {
            hash: "obsolete-digest".to_string(),
            algorithm: "sha1".to_string(),
            mtime: file_mtime_secs(&path).unwrap().to_string(),
            path: stored_path(&path),
        }],
    }];
    std::fs::write(
        collisions_path(cache.path(), GAME, "algorithm.psc"),
        encode_collisions(&groups).unwrap(),
    )
    .unwrap();

    assert_eq!(
        content_hash_in(cache.path(), GAME, &path),
        Some(sha256_hex(source))
    );
    flush_in(cache.path());
    let persisted = decode_collisions(
        &std::fs::read(collisions_path(cache.path(), GAME, "algorithm.psc")).unwrap(),
    )
    .unwrap();
    assert_eq!(persisted[0].implementers[0].algorithm, ALGORITHM);
}

#[test]
fn implementers_are_sorted_and_an_existing_path_is_updated() {
    let cache = tempdir().unwrap();
    let first_project = tempdir().unwrap();
    let second_project = tempdir().unwrap();
    let first = write_script(first_project.path(), "Shared.psc", "first");
    let second = write_script(second_project.path(), "Shared.psc", "second");

    remember_source_in(cache.path(), GAME, &second, "second");
    remember_source_in(cache.path(), GAME, &first, "first");
    remember_source_in(cache.path(), GAME, &first, "updated from memory");
    flush_in(cache.path());

    let groups = decode_collisions(
        &std::fs::read(collisions_path(cache.path(), GAME, "shared.psc")).unwrap(),
    )
    .unwrap();
    let implementers = &groups[0].implementers;
    assert_eq!(implementers.len(), 2);
    assert!(implementers[0].path < implementers[1].path);
    let first_path = stored_path(&first);
    assert_eq!(
        implementers
            .iter()
            .find(|implementer| implementer.path == first_path)
            .unwrap()
            .hash,
        sha256_hex("updated from memory")
    );
}

#[test]
fn an_unchanged_record_is_not_marked_dirty_again() {
    let cache = tempdir().unwrap();
    let project = tempdir().unwrap();
    let source = "ScriptName Clean\n";
    let path = write_script(project.path(), "Clean.psc", source);
    let collision_file = collisions_path(cache.path(), GAME, "clean.psc");

    remember_source_in(cache.path(), GAME, &path, source);
    flush_in(cache.path());
    std::fs::remove_file(&collision_file).unwrap();
    remember_source_in(cache.path(), GAME, &path, source);
    flush_in(cache.path());

    assert!(!collision_file.exists());
}

#[test]
fn flush_only_writes_groups_for_the_requested_cache_directory() {
    let first_cache = tempdir().unwrap();
    let second_cache = tempdir().unwrap();
    let project = tempdir().unwrap();
    let path = write_script(project.path(), "Deferred.psc", "ScriptName Deferred\n");

    remember_source_in(second_cache.path(), GAME, &path, "ScriptName Deferred\n");
    flush_in(first_cache.path());
    assert!(!collisions_path(second_cache.path(), GAME, "deferred.psc").exists());

    flush_in(second_cache.path());
    assert!(collisions_path(second_cache.path(), GAME, "deferred.psc").is_file());
}

#[test]
fn decode_rejects_truncated_unknown_version_and_invalid_payloads() {
    assert_eq!(decode_collisions(b"IPLC"), Err(()));

    let mut wrong_version = Vec::from(MAGIC.as_slice());
    wrong_version.extend_from_slice(&(FORMAT_VERSION + 1).to_le_bytes());
    assert_eq!(decode_collisions(&wrong_version), Err(()));

    let mut invalid_payload = Vec::from(MAGIC.as_slice());
    invalid_payload.extend_from_slice(&FORMAT_VERSION.to_le_bytes());
    invalid_payload.extend_from_slice(b"not bincode");
    assert_eq!(decode_collisions(&invalid_payload), Err(()));
}

#[test]
fn non_utf8_source_is_decoded_as_windows_1252_before_hashing() {
    let cache = tempdir().unwrap();
    let project = tempdir().unwrap();
    let path = project.path().join("Encoded.psc");
    std::fs::write(&path, b"ScriptName Caf\xe9\n").unwrap();

    assert_eq!(
        content_hash_in(cache.path(), GAME, &path),
        Some(sha256_hex("ScriptName Café\n"))
    );
}

#[test]
fn helpers_normalize_names_and_fall_back_to_skyrim() {
    let cache = tempdir().unwrap();
    assert_eq!(group_key(cache.path(), GAME, "MiXeD.PSC").2, "mixed.psc");
    assert_eq!(
        collisions_path(cache.path(), GAME, "MiXeD.PSC"),
        collisions_path(cache.path(), GAME, "mixed.psc")
    );
    assert_eq!(game_from_key("not-a-game"), Game::Skyrim);
}
