//! Smoke tests for the public facade in [`super`], exercised against the
//! real (environment-selected) cache directory rather than a temp one --
//! unlike the `_in`-suffixed internals' own tests under [`crate::ops`].

use super::*;
use tempfile::tempdir;

#[test]
fn public_get_and_put_do_not_panic() {
    let project_dir = tempdir().unwrap();
    let source_path = project_dir.path().join("Example.psc");
    let source = "ScriptName Example\n";
    std::fs::write(&source_path, source).unwrap();

    let ast = papyrus_parser::parse(source).unwrap();
    put(&source_path, source, &ast);
    let _ = get(&source_path, source);
}

#[test]
fn public_put_then_get_returns_the_cached_ast() {
    let project_dir = tempdir().unwrap();
    let source_path = project_dir.path().join("PublicRoundtripAst.psc");
    let source = "ScriptName PublicRoundtripAst\n";
    std::fs::write(&source_path, source).unwrap();

    let ast = papyrus_parser::parse(source).unwrap();
    put(&source_path, source, &ast);
    assert_eq!(get(&source_path, source), Some(ast));
}

#[test]
fn public_get_tokens_and_put_tokens_do_not_panic() {
    let project_dir = tempdir().unwrap();
    let source_path = project_dir.path().join("Example.psc");
    let source = "ScriptName Example\n";
    std::fs::write(&source_path, source).unwrap();

    let tokens = papyrus_parser::tokenize(source).unwrap();
    put_tokens(&source_path, source, &tokens);
    let _ = get_tokens(&source_path, source);
}

#[test]
fn public_put_tokens_then_get_tokens_returns_the_cached_tokens() {
    let project_dir = tempdir().unwrap();
    let source_path = project_dir.path().join("PublicRoundtripTokens.psc");
    let source = "ScriptName PublicRoundtripTokens\n";
    std::fs::write(&source_path, source).unwrap();

    let tokens = papyrus_parser::tokenize(source).unwrap();
    put_tokens(&source_path, source, &tokens);
    assert_eq!(get_tokens(&source_path, source), Some(tokens));
}

#[test]
fn public_ensure_primed_does_not_panic() {
    let project_dir = tempdir().unwrap();
    let source_path = project_dir.path().join("PublicEnsurePrimed.psc");
    let source = "ScriptName PublicEnsurePrimed\n";
    std::fs::write(&source_path, source).unwrap();

    ensure_primed(&source_path, source);
    assert_eq!(
        get(&source_path, source),
        Some(papyrus_parser::parse(source).unwrap())
    );
    assert_eq!(
        get_tokens(&source_path, source),
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
                put(source_path, source, ast);
                put_tokens(source_path, source, tokens);
                let _ = get(source_path, source);
                let _ = get_tokens(source_path, source);
                ensure_primed(source_path, source);
            });
        }
    });

    assert_eq!(get(&source_path, source), Some(ast));
    assert_eq!(get_tokens(&source_path, source), Some(tokens));
}
