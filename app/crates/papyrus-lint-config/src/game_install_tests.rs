use super::*;

#[test]
fn merge_lookup_roots_appends_unique_paths_ignoring_slash_and_case() {
    let mut roots = vec!["C:/Games/Skyrim Special Edition/Data/Scripts/Source".to_string()];
    merge_lookup_roots(
        &mut roots,
        &[
            r"c:\Games\Skyrim Special Edition\Data\Scripts\Source".to_string(),
            "C:/Games/Skyrim Special Edition/Data/Source/Scripts".to_string(),
        ],
    );

    assert_eq!(
        roots,
        vec![
            "C:/Games/Skyrim Special Edition/Data/Scripts/Source".to_string(),
            "C:/Games/Skyrim Special Edition/Data/Source/Scripts".to_string()
        ]
    );
}

#[test]
fn detected_script_lookup_dirs_for_game_returns_empty_for_starfield() {
    assert_eq!(
        detected_script_lookup_dirs_for_game(Game::Starfield),
        Vec::<String>::new()
    );
}
