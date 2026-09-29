use papyrus_lint_globals::Game;
use papyrus_parser::parser::GameEdition;
use papyrus_parser::{parse_for_game, PapyrusError};

#[test]
fn game_targets_select_the_expected_parser_dialect() {
    let fallout_source = "ScriptName FalloutSyntax\nStruct Point\nInt X\nEndStruct\n";
    for game in [Game::Skyrim, Game::Legacy] {
        assert!(
            parse_for_game(fallout_source, game).is_err(),
            "{game:?} should reject Fallout 4 declarations"
        );
    }
    for game in [Game::Fallout4, Game::Starfield] {
        let script = parse_for_game(fallout_source, game).unwrap_or_else(|error| {
            panic!("{game:?} should accept Fallout 4 declarations: {error}")
        });
        assert_eq!(script.structs.len(), 1, "{game:?}");
        assert_eq!(script.structs[0].name, "Point", "{game:?}");
    }

    let starfield_source = "ScriptName StarfieldSyntax\nGuard ProcessGuard\n";
    for game in [Game::Skyrim, Game::Legacy, Game::Fallout4] {
        assert!(
            parse_for_game(starfield_source, game).is_err(),
            "{game:?} should reject Starfield guard declarations"
        );
    }
    let script = parse_for_game(starfield_source, Game::Starfield)
        .expect("Starfield should accept guard declarations");
    assert_eq!(script.guards.len(), 1);
    assert_eq!(script.guards[0].name, "ProcessGuard");
}

#[test]
fn every_game_target_maps_to_the_documented_edition() {
    for (game, edition) in [
        (Game::Skyrim, GameEdition::Skyrim),
        (Game::Legacy, GameEdition::Skyrim),
        (Game::Fallout4, GameEdition::Fallout4),
        (Game::Starfield, GameEdition::Starfield),
    ] {
        assert_eq!(GameEdition::from(game), edition, "{game:?}");
    }
}

#[test]
fn game_routing_preserves_lexer_and_parser_errors() {
    let lex_error = parse_for_game("ScriptName Broken\n@", Game::Fallout4).unwrap_err();
    assert!(matches!(lex_error, PapyrusError::Lex(_)));

    let parse_error = parse_for_game("ScriptName", Game::Starfield).unwrap_err();
    assert!(matches!(parse_error, PapyrusError::Parse(_)));
}
