use super::*;

#[test]
fn bundled_games_expose_native_globals() {
    assert!(!globals_for(Game::Skyrim).is_empty());
    assert!(!globals_for(Game::Fallout4).is_empty());
}

#[test]
fn recognizes_generated_entries_for_each_supported_game() {
    for game in [Game::Skyrim, Game::Fallout4] {
        let first = globals_for(game)
            .first()
            .expect("supported games should contain native globals");

        assert!(is_known_for(game, first));
    }
}

#[test]
fn rejects_unknown_and_non_lowercase_names() {
    assert!(!is_known_for(Game::Skyrim, "definitely_missing_script"));
    assert!(!is_known_for(Game::Fallout4, "definitely_missing_script"));

    let known = globals_for(Game::Skyrim)
        .first()
        .expect("Skyrim should contain native globals");
    assert!(!is_known_for(Game::Skyrim, &known.to_ascii_uppercase()));
}

#[test]
#[should_panic(expected = "unsupported game starfield provided")]
fn rejects_unsupported_games() {
    let _ = globals_for(Game::Starfield);
}
