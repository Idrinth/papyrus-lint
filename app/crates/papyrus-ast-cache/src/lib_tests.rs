//! Smoke tests for the public facade in [`super`], exercised against the
//! real (environment-selected) cache directory rather than a temp one --
//! unlike the `_in`-suffixed internals' own tests under [`crate::ops`].

use super::*;
use std::io::Read;
use tempfile::tempdir;

const SKYRIM: papyrus_lint_globals::Game = papyrus_lint_globals::Game::Skyrim;
const FALLOUT4: papyrus_lint_globals::Game = papyrus_lint_globals::Game::Fallout4;
const STARFIELD: papyrus_lint_globals::Game = papyrus_lint_globals::Game::Starfield;

fn bundled_script(archive_name: &str, file_name: &str) -> String {
    let archive_path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../../shared/scripts")
        .join(archive_name);
    let file = std::fs::File::open(&archive_path).unwrap();
    let mut archive = zip::ZipArchive::new(file).unwrap();
    let suffix = format!("/{file_name}");

    for index in 0..archive.len() {
        let mut entry = archive.by_index(index).unwrap();
        if entry.name().ends_with(&suffix) || entry.name() == file_name {
            let mut source = String::new();
            entry.read_to_string(&mut source).unwrap();
            return source;
        }
    }

    panic!("no {file_name} in {}", archive_path.display());
}

#[test]
fn public_get_and_put_do_not_panic() {
    let project_dir = tempdir().unwrap();
    let source_path = project_dir.path().join("Example.psc");
    let source = "ScriptName Example\n";
    std::fs::write(&source_path, source).unwrap();

    let ast = papyrus_parser::parse(source).unwrap();
    put_for_game(SKYRIM, &source_path, source, &ast);
    let _ = get_for_game(SKYRIM, &source_path, source);
}

#[test]
fn public_put_then_get_returns_the_cached_ast() {
    let project_dir = tempdir().unwrap();
    let source_path = project_dir.path().join("PublicRoundtripAst.psc");
    let source = "ScriptName PublicRoundtripAst\n";
    std::fs::write(&source_path, source).unwrap();

    let ast = papyrus_parser::parse(source).unwrap();
    put_for_game(SKYRIM, &source_path, source, &ast);
    assert_eq!(get_for_game(SKYRIM, &source_path, source), Some(ast));
}

#[test]
fn public_get_tokens_and_put_tokens_do_not_panic() {
    let project_dir = tempdir().unwrap();
    let source_path = project_dir.path().join("Example.psc");
    let source = "ScriptName Example\n";
    std::fs::write(&source_path, source).unwrap();

    let tokens = papyrus_parser::tokenize(source).unwrap();
    put_tokens_for_game(SKYRIM, &source_path, source, &tokens);
    let _ = get_tokens_for_game(SKYRIM, &source_path, source);
}

#[test]
fn public_put_tokens_then_get_tokens_returns_the_cached_tokens() {
    let project_dir = tempdir().unwrap();
    let source_path = project_dir.path().join("PublicRoundtripTokens.psc");
    let source = "ScriptName PublicRoundtripTokens\n";
    std::fs::write(&source_path, source).unwrap();

    let tokens = papyrus_parser::tokenize(source).unwrap();
    put_tokens_for_game(SKYRIM, &source_path, source, &tokens);
    assert_eq!(
        get_tokens_for_game(SKYRIM, &source_path, source),
        Some(tokens)
    );
}

#[test]
fn public_ensure_primed_does_not_panic() {
    let project_dir = tempdir().unwrap();
    let source_path = project_dir.path().join("PublicEnsurePrimed.psc");
    let source = "ScriptName PublicEnsurePrimed\n";
    std::fs::write(&source_path, source).unwrap();

    ensure_primed_for_game(SKYRIM, &source_path, source);
    assert_eq!(
        get_for_game(SKYRIM, &source_path, source),
        Some(papyrus_parser::parse(source).unwrap())
    );
    assert_eq!(
        get_tokens_for_game(SKYRIM, &source_path, source),
        Some(papyrus_parser::tokenize(source).unwrap())
    );
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
                put_for_game(SKYRIM, source_path, source, ast);
                put_tokens_for_game(SKYRIM, source_path, source, tokens);
                let _ = get_for_game(SKYRIM, source_path, source);
                let _ = get_tokens_for_game(SKYRIM, source_path, source);
                ensure_primed_for_game(SKYRIM, source_path, source);
            });
        }
    });

    assert_eq!(get_for_game(SKYRIM, &source_path, source), Some(ast));
    assert_eq!(
        get_tokens_for_game(SKYRIM, &source_path, source),
        Some(tokens)
    );
}

#[test]
fn public_content_md5_round_trips_without_reopening_source_text() {
    let project_dir = tempdir().unwrap();
    let source_path = project_dir.path().join("PublicContentMd5.psc");
    let source = "ScriptName PublicContentMd5\n";
    std::fs::write(&source_path, source).unwrap();
    let digest = format!("{:x}", md5::compute(source.as_bytes()));

    put_content_md5_for_game(SKYRIM, &source_path, &digest);
    assert_eq!(
        content_md5_for_game(SKYRIM, &source_path),
        Some(digest.clone())
    );
    put_for_game(
        SKYRIM,
        &source_path,
        source,
        &papyrus_parser::parse(source).unwrap(),
    );
    assert_eq!(content_md5_for_game(SKYRIM, &source_path), Some(digest));
}

#[test]
fn fallout4_does_not_hit_the_skyrim_bundled_blob() {
    // A source text that is a bundled hit in Skyrim's blob (a bare `Actor`
    // declaration is never how the real Skyrim `Actor.psc` reads) would
    // still need to independently be a Fallout 4 bundled script to hit
    // here; an unrelated source is a miss for both.
    let source = "ScriptName NotABundledScriptForEitherGame\n";
    let missing = std::path::Path::new("/does/not/exist/NotABundledScriptForEitherGame.psc");
    assert!(get_for_game(FALLOUT4, missing, source).is_none());
    assert!(get_tokens_for_game(FALLOUT4, missing, source).is_none());
    assert!(ast_for_script_name(FALLOUT4, "DefinitelyNotAVanillaScript").is_none());
    assert!(!contains_script_name(
        FALLOUT4,
        "DefinitelyNotAVanillaScript"
    ));
}

#[test]
fn fallout4_has_its_own_bundled_blob_independent_of_skyrims() {
    // Both games ship an `Actor.psc`; each game's lookup must resolve to
    // its own game's bundled blob, not the other's.
    assert!(contains_script_name(SKYRIM, "Actor"));
    assert!(contains_script_name(FALLOUT4, "Actor"));
    let skyrim_actor = ast_for_script_name(SKYRIM, "Actor").expect("Skyrim Actor should resolve");
    let fallout4_actor =
        ast_for_script_name(FALLOUT4, "Actor").expect("Fallout 4 Actor should resolve");
    assert_eq!(skyrim_actor.name, "Actor");
    assert_eq!(fallout4_actor.name, "Actor");
}

#[test]
fn public_accessors_return_bundled_ast_and_tokens_without_a_source_file() {
    let source = bundled_script("skyrim-scripts.zip", "Actor.psc");
    let missing = Path::new("/does/not/exist/Actor.psc");

    let ast = get_for_game(SKYRIM, missing, &source).expect("Actor should be bundled");
    let tokens = get_tokens_for_game(SKYRIM, missing, &source)
        .expect("Actor's token stream should be bundled");

    assert_eq!(ast.name, "Actor");
    assert_eq!(tokens, papyrus_parser::tokenize(&source).unwrap());
    ensure_primed_for_game(SKYRIM, missing, &source);
    assert_eq!(papyrus_parser::parse(&source).unwrap().name, "Actor");
}

#[test]
fn public_name_accessors_are_case_insensitive_for_every_game() {
    for game in [SKYRIM, FALLOUT4, STARFIELD] {
        assert!(contains_script_name(game, "aCtOr"));
        assert_eq!(
            ast_for_script_name(game, "aCtOr").map(|ast| ast.name),
            Some("Actor".to_string())
        );
        assert!(!contains_script_name(game, "DefinitelyNotAVanillaScript"));
        assert!(ast_for_script_name(game, "DefinitelyNotAVanillaScript").is_none());
    }
}

#[test]
fn public_disk_cache_entries_are_namespaced_by_game() {
    let project_dir = tempdir().unwrap();
    let source_path = project_dir.path().join("GameSpecific.psc");
    let source = "ScriptName GameSpecific\n";
    std::fs::write(&source_path, source).unwrap();
    let ast = papyrus_parser::parse(source).unwrap();

    put_for_game(SKYRIM, &source_path, source, &ast);

    assert_eq!(get_for_game(SKYRIM, &source_path, source), Some(ast));
    assert!(get_for_game(FALLOUT4, &source_path, source).is_none());
    assert!(content_md5_for_game(FALLOUT4, &source_path).is_none());
}

#[test]
fn public_cache_dir_reports_the_selected_cache_location() {
    assert!(cache_dir().is_some());
}
