use super::*;

#[test]
fn default_is_skyrim() {
    assert_eq!(Game::default(), Game::Skyrim);
    assert_eq!(Game::default().as_str(), "skyrim");
}

#[test]
fn as_str_matches_serde_and_from_str_keys() {
    for game in Game::ALL {
        assert_eq!(game.to_string(), game.as_str());
        assert_eq!(Game::from_str(game.as_str()), Ok(game));
    }
}

#[test]
fn from_str_rejects_unknown_keys() {
    assert!(Game::from_str("oblivion").is_err());
    assert!(Game::from_str("Skyrim").is_err());
    assert!(Game::from_str("fallout-4").is_err());
}

#[test]
fn fallout4_extensions_follow_the_fo4_dialect() {
    assert!(!Game::Skyrim.has_fallout4_extensions());
    assert!(Game::Fallout4.has_fallout4_extensions());
    assert!(Game::Starfield.has_fallout4_extensions());
}

#[test]
fn every_recognized_game_is_supported() {
    for game in Game::ALL {
        game.assert_supported();
    }
}

#[test]
fn all_lists_every_variant_once() {
    assert_eq!(Game::ALL.len(), 3);
    assert!(Game::ALL.contains(&Game::Skyrim));
    assert!(Game::ALL.contains(&Game::Fallout4));
    assert!(Game::ALL.contains(&Game::Starfield));
}
