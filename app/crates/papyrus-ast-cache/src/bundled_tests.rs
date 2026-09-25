use super::*;
use crate::bundled_blob::{self, PackedEntry};
use std::io::Read;
use std::path::Path;

const SKYRIM: papyrus_lint_globals::Game = papyrus_lint_globals::Game::Skyrim;
const FALLOUT4: papyrus_lint_globals::Game = papyrus_lint_globals::Game::Fallout4;
const STARFIELD: papyrus_lint_globals::Game = papyrus_lint_globals::Game::Starfield;

fn zip_path(archive_name: &str) -> std::path::PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../../shared/scripts")
        .join(archive_name)
}

fn decode_psc_bytes(bytes: &[u8]) -> String {
    String::from_utf8(bytes.to_vec()).unwrap_or_else(|err| {
        panic!("test helper only loads UTF-8 vanilla scripts, got invalid UTF-8: {err}")
    })
}

fn zip_script(archive_name: &str, file_name: &str) -> String {
    let zip_path = zip_path(archive_name);
    let file = std::fs::File::open(&zip_path).unwrap();
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
    panic!("no {file_name} in {}", zip_path.display());
}

fn strip_deprecation(script: &mut papyrus_parser::ast::Script) {
    for function in script.functions.iter_mut().chain(
        script
            .states
            .iter_mut()
            .flat_map(|state| state.functions.iter_mut()),
    ) {
        function.deprecation = None;
    }
}

#[test]
fn encode_then_parse_round_trips_a_synthetic_entry() {
    let source = "ScriptName BundledBlobRoundtrip\nFunction Legacy()\nEndFunction\n";
    let mut ast = papyrus_parser::parse(source).unwrap();
    ast.functions[0].deprecation = Some(papyrus_parser::ast::Deprecation {
        replacement: Some("Current()".to_string()),
        message: "Use Current() instead".to_string(),
    });
    let tokens = papyrus_parser::tokenize(source).unwrap();
    let packed = PackedEntry {
        md5: md5::compute(source.as_bytes()).0,
        name: "bundledblobroundtrip".to_string(),
        ast: bundled_blob::serialize_ast(&ast).unwrap(),
        tokens: bundled_blob::serialize_tokens(&tokens).unwrap(),
    };
    let blob = bundled_blob::encode_blob(&[packed]);
    let parsed = bundled_blob::parse_blob(&blob).unwrap();
    assert_eq!(parsed.by_md5.len(), 1);
    assert_eq!(parsed.by_name.len(), 1);
    let payload = &blob[parsed.payload_start..];
    let entry = parsed
        .by_md5
        .get(&md5::compute(source.as_bytes()).0)
        .unwrap();
    assert_eq!(bundled_blob::decode_ast(payload, entry).unwrap(), ast);
    assert_eq!(bundled_blob::decode_tokens(payload, entry).unwrap(), tokens);
    let named = parsed.by_name.get("bundledblobroundtrip").unwrap();
    assert_eq!(bundled_blob::decode_ast(payload, named).unwrap(), ast);
}

#[test]
fn parse_blob_name_index_keeps_the_last_duplicate_name() {
    let first = "ScriptName SharedName\nInt Function First()\nEndFunction\n";
    let second = "ScriptName SharedName\nInt Function Second()\nEndFunction\n";
    let first_ast = papyrus_parser::parse(first).unwrap();
    let second_ast = papyrus_parser::parse(second).unwrap();
    let blob = bundled_blob::encode_blob(&[
        PackedEntry {
            md5: md5::compute(first.as_bytes()).0,
            name: "sharedname".to_string(),
            ast: bundled_blob::serialize_ast(&first_ast).unwrap(),
            tokens: bundled_blob::serialize_tokens(&papyrus_parser::tokenize(first).unwrap())
                .unwrap(),
        },
        PackedEntry {
            md5: md5::compute(second.as_bytes()).0,
            name: "sharedname".to_string(),
            ast: bundled_blob::serialize_ast(&second_ast).unwrap(),
            tokens: bundled_blob::serialize_tokens(&papyrus_parser::tokenize(second).unwrap())
                .unwrap(),
        },
    ]);
    let parsed = bundled_blob::parse_blob(&blob).unwrap();
    assert_eq!(parsed.by_md5.len(), 2);
    let named = parsed.by_name.get("sharedname").unwrap();
    let ast = bundled_blob::decode_ast(&blob[parsed.payload_start..], named).unwrap();
    assert!(ast
        .functions
        .iter()
        .any(|function| function.name == "Second"));
    assert!(!ast
        .functions
        .iter()
        .any(|function| function.name == "First"));
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
fn empty_blob_round_trips_without_entries() {
    let blob = bundled_blob::encode_blob(&[]);
    let parsed = bundled_blob::parse_blob(&blob).unwrap();

    assert!(parsed.by_md5.is_empty());
    assert!(parsed.by_name.is_empty());
    assert_eq!(parsed.payload_start, blob.len());
}

#[test]
fn parse_blob_rejects_each_truncated_index_boundary() {
    let source = "ScriptName TruncatedIndex\n";
    let entry = PackedEntry {
        md5: md5::compute(source.as_bytes()).0,
        name: "truncatedindex".to_string(),
        ast: bundled_blob::serialize_ast(&papyrus_parser::parse(source).unwrap()).unwrap(),
        tokens: bundled_blob::serialize_tokens(&papyrus_parser::tokenize(source).unwrap()).unwrap(),
    };
    let blob = bundled_blob::encode_blob(&[entry]);
    let payload_start = bundled_blob::parse_blob(&blob).unwrap().payload_start;

    for end in 12..payload_start {
        assert!(
            bundled_blob::parse_blob(&blob[..end]).is_none(),
            "accepted an index truncated at byte {end}"
        );
    }
}

#[test]
fn parse_blob_rejects_a_non_utf8_script_name() {
    let source = "ScriptName InvalidName\n";
    let entry = PackedEntry {
        md5: md5::compute(source.as_bytes()).0,
        name: "invalidname".to_string(),
        ast: bundled_blob::serialize_ast(&papyrus_parser::parse(source).unwrap()).unwrap(),
        tokens: bundled_blob::serialize_tokens(&papyrus_parser::tokenize(source).unwrap()).unwrap(),
    };
    let mut blob = bundled_blob::encode_blob(&[entry]);
    // The first name starts after the 12-byte header, 16-byte MD5, and
    // 2-byte name length.
    blob[30] = 0xff;

    assert!(bundled_blob::parse_blob(&blob).is_none());
}

#[test]
fn empty_script_names_are_not_added_to_the_name_index() {
    let source = "ScriptName UnnamedIndexEntry\n";
    let ast = papyrus_parser::parse(source).unwrap();
    let tokens = papyrus_parser::tokenize(source).unwrap();
    let blob = bundled_blob::encode_blob(&[PackedEntry {
        md5: md5::compute(source.as_bytes()).0,
        name: String::new(),
        ast: bundled_blob::serialize_ast(&ast).unwrap(),
        tokens: bundled_blob::serialize_tokens(&tokens).unwrap(),
    }]);
    let parsed = bundled_blob::parse_blob(&blob).unwrap();

    assert_eq!(parsed.by_md5.len(), 1);
    assert!(parsed.by_name.is_empty());
}

#[test]
fn decoders_reject_out_of_bounds_and_invalid_payloads() {
    let out_of_bounds = bundled_blob::IndexEntry {
        ast_offset: u64::MAX,
        ast_len: 1,
        tokens_offset: 0,
        tokens_len: u32::MAX,
    };
    assert!(bundled_blob::decode_ast(&[], &out_of_bounds).is_none());
    assert!(bundled_blob::decode_tokens(&[], &out_of_bounds).is_none());

    let invalid_bincode = bundled_blob::IndexEntry {
        ast_offset: 0,
        ast_len: 1,
        tokens_offset: 0,
        tokens_len: 1,
    };
    assert!(bundled_blob::decode_ast(&[0xff], &invalid_bincode).is_none());
    assert!(bundled_blob::decode_tokens(&[0xff], &invalid_bincode).is_none());
}

#[test]
fn parse_cache_rejects_invalid_gzip_and_invalid_blob_contents() {
    assert!(parse_cache(b"not gzip data").is_none());

    use std::io::Write;
    let mut compressed = Vec::new();
    {
        let mut encoder =
            flate2::write::GzEncoder::new(&mut compressed, flate2::Compression::default());
        encoder.write_all(b"not a bundled cache blob").unwrap();
        encoder.finish().unwrap();
    }
    assert!(parse_cache(&compressed).is_none());
}

#[test]
fn bundled_cache_covers_the_skyrim_vanilla_script_archive() {
    assert!(
        entry_count(SKYRIM) >= 14_000,
        "expected the bundled cache to cover the Skyrim archive, got {}",
        entry_count(SKYRIM)
    );
}

#[test]
fn bundled_cache_covers_the_fallout4_vanilla_script_archive() {
    // Lower than Skyrim's threshold: a chunk of the Fallout 4 archive uses
    // constructs `papyrus-parser` doesn't accept yet (e.g. the
    // `Namespace:ScriptName` form used by Creation Club content), which
    // `build.rs` skips with a `cargo:warning` rather than failing the
    // build (see its module docs). This only needs to cover the base
    // engine hierarchy (`Actor`, `ObjectReference`, `Form`, …) that
    // `FunctionTable` actually walks without project data.
    assert!(
        entry_count(FALLOUT4) >= 700,
        "expected the bundled cache to cover most of the Fallout 4 archive, got {}",
        entry_count(FALLOUT4)
    );
}

#[test]
fn bundled_cache_covers_the_starfield_vanilla_script_archive() {
    assert!(
        entry_count(STARFIELD) >= 700,
        "expected the bundled cache to cover most of the Starfield archive, got {}",
        entry_count(STARFIELD)
    );
}

#[test]
fn actor_psc_is_a_bundled_hit_without_a_source_file_on_disk() {
    let source = zip_script("skyrim-scripts.zip", "Actor.psc");
    let missing = Path::new("/does/not/exist/Actor.psc");
    let ast = ast_for(SKYRIM, &source).expect("Actor.psc should be in the bundled cache");
    assert_eq!(ast.name, "Actor");
    assert_eq!(ast.extends.as_deref(), Some("ObjectReference"));
    assert_eq!(
        crate::get_for_game(SKYRIM, missing, &source),
        Some(ast.clone())
    );
    assert_eq!(
        crate::get_tokens_for_game(SKYRIM, missing, &source),
        Some(papyrus_parser::tokenize(&source).unwrap())
    );
    assert!(prime(SKYRIM, &source));
}

#[test]
fn fallout4_actor_psc_is_a_bundled_hit_without_a_source_file_on_disk() {
    let source = zip_script("fallout4-scripts.zip", "Actor.psc");
    let missing = Path::new("/does/not/exist/Actor.psc");
    let ast = ast_for(FALLOUT4, &source).expect("Actor.psc should be in the Fallout 4 bundle");
    assert_eq!(ast.name, "Actor");
    assert_eq!(
        crate::get_for_game(FALLOUT4, missing, &source),
        Some(ast.clone())
    );
    assert_eq!(
        crate::get_tokens_for_game(FALLOUT4, missing, &source),
        Some(papyrus_parser::tokenize(&source).unwrap())
    );
    assert!(prime(FALLOUT4, &source));
}

#[test]
fn starfield_actor_psc_is_a_bundled_hit_without_a_source_file_on_disk() {
    let source = zip_script("starfield-scripts.zip", "Actor.psc");
    let missing = Path::new("/does/not/exist/Actor.psc");
    let ast = ast_for(STARFIELD, &source).expect("Actor.psc should be in the Starfield bundle");
    assert_eq!(ast.name, "Actor");
    assert_eq!(
        crate::get_for_game(STARFIELD, missing, &source),
        Some(ast.clone())
    );
    assert_eq!(
        crate::get_tokens_for_game(STARFIELD, missing, &source),
        Some(papyrus_parser::tokenize(&source).unwrap())
    );
    assert!(prime(STARFIELD, &source));
}

#[test]
fn form_and_game_are_bundled_hits() {
    let form = zip_script("skyrim-scripts.zip", "Form.psc");
    let ast = ast_for(SKYRIM, &form).expect("Form.psc should be in the bundled cache");
    assert_eq!(ast.name, "Form");
    assert!(ast.extends.is_none());

    let game = zip_script("skyrim-scripts.zip", "Game.psc");
    let ast = ast_for(SKYRIM, &game).expect("Game.psc should be in the bundled cache");
    assert_eq!(ast.name, "Game");
}

#[test]
fn skse_psc_is_a_bundled_hit() {
    let source = zip_script("skyrim-extender-scripts.zip", "SKSE.psc");
    let ast = ast_for(SKYRIM, &source).expect("SKSE.psc should be in the bundled cache");
    assert_eq!(ast.name, "SKSE");
    assert_eq!(
        tokens_for(SKYRIM, &source),
        papyrus_parser::tokenize(&source).ok()
    );
    assert!(prime(SKYRIM, &source));
}

#[test]
fn f4se_psc_is_a_bundled_hit() {
    let source = zip_script("fallout4-extender-scripts.zip", "F4SE.psc");
    let ast = ast_for(FALLOUT4, &source).expect("F4SE.psc should be in the Fallout 4 bundle");
    assert_eq!(ast.name, "F4SE");
    assert_eq!(
        tokens_for(FALLOUT4, &source),
        papyrus_parser::tokenize(&source).ok()
    );
    assert!(prime(FALLOUT4, &source));
}

#[test]
fn a_modified_vanilla_script_is_a_bundled_miss() {
    let mut source = zip_script("skyrim-scripts.zip", "Actor.psc");
    source.push_str("\n; user edit\n");
    assert!(ast_for(SKYRIM, &source).is_none());
    assert!(tokens_for(SKYRIM, &source).is_none());
    assert!(!prime(SKYRIM, &source));
}

#[test]
fn unrelated_source_is_a_bundled_miss() {
    let source = "ScriptName NotAVanillaScript\n";
    assert!(ast_for(SKYRIM, source).is_none());
    assert!(tokens_for(SKYRIM, source).is_none());
    assert!(!prime(SKYRIM, source));
}

#[test]
fn a_skyrim_script_is_not_a_fallout4_bundled_hit() {
    let source = zip_script("skyrim-extender-scripts.zip", "SKSE.psc");
    assert!(ast_for(FALLOUT4, &source).is_none());
    assert!(tokens_for(FALLOUT4, &source).is_none());
    assert!(!prime(FALLOUT4, &source));
}

#[test]
fn bundled_actor_matches_a_fresh_parse_and_tokenize() {
    let source = zip_script("skyrim-scripts.zip", "Actor.psc");
    let mut ast = ast_for(SKYRIM, &source).unwrap();
    strip_deprecation(&mut ast);
    let tokens = tokens_for(SKYRIM, &source).unwrap();
    // `ast_for`/`tokens_for` above prime `papyrus_parser`'s in-memory memo
    // cache with the catalog-enriched result for this exact source, so
    // `papyrus_parser::parse`/`tokenize` would just hand that same result
    // back instead of doing a real fresh parse. Bypass the memo cache to
    // get a genuinely independent parse to compare against.
    let fresh_tokens = papyrus_parser::lexer::Lexer::new(&source)
        .tokenize()
        .unwrap();
    let fresh_ast = papyrus_parser::parser::Parser::new(fresh_tokens.clone())
        .parse_script()
        .unwrap();
    assert_eq!(ast, fresh_ast);
    assert_eq!(tokens, fresh_tokens);
}

#[test]
fn bundled_actor_carries_catalogued_deprecation_metadata() {
    let ast = ast_for_name(SKYRIM, "Actor").expect("Actor should be in the name index");
    let favor = ast
        .functions
        .iter()
        .find(|function| function.name.eq_ignore_ascii_case("ModFavorPoints"))
        .expect("ModFavorPoints should exist on Actor");
    let deprecation = favor
        .deprecation
        .as_ref()
        .expect("catalogued Actor.ModFavorPoints should be marked deprecated");
    assert_eq!(
        deprecation.replacement.as_deref(),
        Some("MakePlayerFriend()")
    );
}

#[test]
fn ensure_primed_skips_the_disk_cache_for_a_bundled_script() {
    let source = zip_script("skyrim-scripts.zip", "ObjectReference.psc");
    let missing = Path::new("/does/not/exist/ObjectReference.psc");
    crate::ensure_primed_for_game(SKYRIM, missing, &source);
    assert_eq!(
        crate::get_for_game(SKYRIM, missing, &source).map(|script| script.name),
        Some("ObjectReference".to_string())
    );
    assert_eq!(
        crate::get_tokens_for_game(SKYRIM, missing, &source),
        Some(papyrus_parser::tokenize(&source).unwrap())
    );
}

#[test]
fn bundled_lookups_are_safe_under_concurrent_use() {
    let source = zip_script("skyrim-scripts.zip", "Quest.psc");
    std::thread::scope(|scope| {
        for _ in 0..8 {
            let source = &source;
            scope.spawn(move || {
                assert!(ast_for(SKYRIM, source).is_some());
                assert!(tokens_for(SKYRIM, source).is_some());
                assert!(prime(SKYRIM, source));
            });
        }
    });
    let ast = ast_for(SKYRIM, &source).unwrap();
    assert_eq!(ast.name, "Quest");
}

#[test]
fn actor_is_a_bundled_hit_by_script_name_without_source_bytes() {
    assert!(contains_name(SKYRIM, "Actor"));
    assert!(contains_name(SKYRIM, "actor"));
    assert!(contains_name(SKYRIM, "OBJECTREFERENCE"));
    assert!(!contains_name(SKYRIM, "DefinitelyNotAVanillaScript"));

    let ast = ast_for_name(SKYRIM, "Actor").expect("Actor should be in the name index");
    assert_eq!(ast.name, "Actor");
    assert_eq!(ast.extends.as_deref(), Some("ObjectReference"));
    let tokens =
        tokens_for_name(SKYRIM, "actor").expect("Actor tokens should be in the name index");
    assert!(!tokens.is_empty());
    assert_eq!(crate::ast_for_script_name(SKYRIM, "Actor"), Some(ast));
    assert!(crate::contains_script_name(SKYRIM, "Form"));
}

#[test]
fn fallout4_is_a_bundled_hit_by_script_name_without_source_bytes() {
    assert!(contains_name(FALLOUT4, "Actor"));
    assert!(!contains_name(FALLOUT4, "DefinitelyNotAVanillaScript"));

    let ast = ast_for_name(FALLOUT4, "Actor").expect("Actor should be in the Fallout 4 index");
    assert_eq!(ast.name, "Actor");
    assert_eq!(crate::ast_for_script_name(FALLOUT4, "Actor"), Some(ast));
    assert!(crate::contains_script_name(FALLOUT4, "Form"));
}

#[test]
fn starfield_is_a_bundled_hit_by_script_name_without_source_bytes() {
    assert!(contains_name(STARFIELD, "Actor"));
    assert!(!contains_name(STARFIELD, "DefinitelyNotAVanillaScript"));

    let ast = ast_for_name(STARFIELD, "Actor").expect("Actor should be in the Starfield index");
    assert_eq!(ast.name, "Actor");
    assert_eq!(crate::ast_for_script_name(STARFIELD, "Actor"), Some(ast));
    assert!(crate::contains_script_name(STARFIELD, "Form"));
}

#[test]
fn name_lookup_walks_the_vanilla_extends_chain() {
    let actor = ast_for_name(SKYRIM, "Actor").unwrap();
    assert_eq!(actor.extends.as_deref(), Some("ObjectReference"));
    let object_reference = ast_for_name(SKYRIM, actor.extends.as_deref().unwrap()).unwrap();
    assert_eq!(object_reference.extends.as_deref(), Some("Form"));
    let form = ast_for_name(SKYRIM, object_reference.extends.as_deref().unwrap()).unwrap();
    assert!(form.extends.is_none());
}
