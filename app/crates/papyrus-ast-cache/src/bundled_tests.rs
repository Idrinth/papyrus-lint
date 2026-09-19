use super::*;
use crate::bundled_blob::{self, PackedEntry};
use std::io::Read;
use std::path::Path;

fn zip_path() -> std::path::PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../shared/skyrim-scripts.zip")
}

fn decode_psc_bytes(bytes: &[u8]) -> String {
    String::from_utf8(bytes.to_vec()).unwrap_or_else(|err| {
        panic!("test helper only loads UTF-8 vanilla scripts, got invalid UTF-8: {err}")
    })
}

fn zip_script(file_name: &str) -> String {
    let file = std::fs::File::open(zip_path()).unwrap();
    let mut archive = zip::ZipArchive::new(file).unwrap();
    let suffix = format!("/{file_name}");
    for i in 0..archive.len() {
        let mut entry = archive.by_index(i).unwrap();
        let name = entry.name().to_string();
        if name.ends_with(&suffix) || name == file_name {
            let mut bytes = Vec::new();
            entry.read_to_end(&mut bytes).unwrap();
            return decode_psc_bytes(&bytes);
        }
    }
    panic!("no {file_name} in {}", zip_path().display());
}

#[test]
fn encode_then_parse_round_trips_a_synthetic_entry() {
    let source = "ScriptName BundledBlobRoundtrip\n";
    let ast = papyrus_parser::parse(source).unwrap();
    let tokens = papyrus_parser::tokenize(source).unwrap();
    let packed = PackedEntry {
        md5: md5::compute(source.as_bytes()).0,
        ast: bundled_blob::serialize_ast(&ast).unwrap(),
        tokens: bundled_blob::serialize_tokens(&tokens).unwrap(),
    };
    let blob = bundled_blob::encode_blob(&[packed]);
    let (index, payload_start) = bundled_blob::parse_blob(&blob).unwrap();
    assert_eq!(index.len(), 1);
    let payload = &blob[payload_start..];
    let entry = index.get(&md5::compute(source.as_bytes()).0).unwrap();
    assert_eq!(bundled_blob::decode_ast(payload, entry).unwrap(), ast);
    assert_eq!(bundled_blob::decode_tokens(payload, entry).unwrap(), tokens);
}

#[test]
fn parse_blob_rejects_a_truncated_or_unknown_header() {
    assert!(bundled_blob::parse_blob(b"").is_none());
    assert!(bundled_blob::parse_blob(b"PLAC").is_none());
    let mut unknown_version = Vec::from(*bundled_blob::MAGIC);
    unknown_version.extend_from_slice(&999u32.to_le_bytes());
    unknown_version.extend_from_slice(&0u32.to_le_bytes());
    assert!(bundled_blob::parse_blob(&unknown_version).is_none());
}

#[test]
fn bundled_cache_covers_the_vanilla_script_archive() {
    assert!(
        entry_count() >= 14_000,
        "expected the bundled cache to cover the Skyrim archive, got {}",
        entry_count()
    );
}

#[test]
fn actor_psc_is_a_bundled_hit_without_a_source_file_on_disk() {
    let source = zip_script("Actor.psc");
    let missing = Path::new("/does/not/exist/Actor.psc");
    let ast = ast_for(&source).expect("Actor.psc should be in the bundled cache");
    assert_eq!(ast.name, "Actor");
    assert_eq!(ast.extends.as_deref(), Some("ObjectReference"));
    assert_eq!(crate::get(missing, &source), Some(ast.clone()));
    assert_eq!(
        crate::get_tokens(missing, &source),
        Some(papyrus_parser::tokenize(&source).unwrap())
    );
    assert!(prime(&source));
}

#[test]
fn form_and_game_are_bundled_hits() {
    let form = zip_script("Form.psc");
    let ast = ast_for(&form).expect("Form.psc should be in the bundled cache");
    assert_eq!(ast.name, "Form");
    assert!(ast.extends.is_none());

    let game = zip_script("Game.psc");
    let ast = ast_for(&game).expect("Game.psc should be in the bundled cache");
    assert_eq!(ast.name, "Game");
}

#[test]
fn a_modified_vanilla_script_is_a_bundled_miss() {
    let mut source = zip_script("Actor.psc");
    source.push_str("\n; user edit\n");
    assert!(ast_for(&source).is_none());
    assert!(tokens_for(&source).is_none());
    assert!(!prime(&source));
}

#[test]
fn unrelated_source_is_a_bundled_miss() {
    let source = "ScriptName NotAVanillaScript\n";
    assert!(ast_for(source).is_none());
    assert!(tokens_for(source).is_none());
    assert!(!prime(source));
}

#[test]
fn bundled_actor_matches_a_fresh_parse_and_tokenize() {
    let source = zip_script("Actor.psc");
    let ast = ast_for(&source).unwrap();
    let tokens = tokens_for(&source).unwrap();
    assert_eq!(ast, papyrus_parser::parse(&source).unwrap());
    assert_eq!(tokens, papyrus_parser::tokenize(&source).unwrap());
}

#[test]
fn ensure_primed_skips_the_disk_cache_for_a_bundled_script() {
    let source = zip_script("ObjectReference.psc");
    let missing = Path::new("/does/not/exist/ObjectReference.psc");
    crate::ensure_primed(missing, &source);
    assert_eq!(
        crate::get(missing, &source).map(|script| script.name),
        Some("ObjectReference".to_string())
    );
    assert_eq!(
        crate::get_tokens(missing, &source),
        Some(papyrus_parser::tokenize(&source).unwrap())
    );
}

#[test]
fn bundled_lookups_are_safe_under_concurrent_use() {
    let source = zip_script("Quest.psc");
    std::thread::scope(|scope| {
        for _ in 0..8 {
            let source = &source;
            scope.spawn(move || {
                assert!(ast_for(source).is_some());
                assert!(tokens_for(source).is_some());
                assert!(prime(source));
            });
        }
    });
    let ast = ast_for(&source).unwrap();
    assert_eq!(ast.name, "Quest");
}
