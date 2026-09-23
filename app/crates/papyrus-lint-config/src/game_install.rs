//! Dispatches vanilla script-source detection to the game a project is
//! actually configured for, and merges the result into a project's
//! `lookup_script_roots` without duplicating entries it already lists.

use papyrus_lints::Game;

use crate::fallout4::detected_fallout4_script_lookup_dirs;
use crate::skyrim::detected_skyrim_script_lookup_dirs;

/// Vanilla Papyrus source directories for `game`'s install, used to seed a
/// project's `lookup_script_roots`. Games without registry-based detection
/// support yet (currently Starfield) return an empty list.
pub(crate) fn detected_script_lookup_dirs_for_game(game: Game) -> Vec<String> {
    match game {
        Game::Skyrim => detected_skyrim_script_lookup_dirs(),
        Game::Fallout4 => detected_fallout4_script_lookup_dirs(),
        Game::Starfield => Vec::new(),
    }
}

/// Appends each of `extra`'s directories to `roots` that aren't already
/// present there (compared with [`lookup_paths_equal`]), trimming and
/// skipping blank entries. Used to seed a project's `lookup_script_roots`
/// with a detected game install's directories without duplicating ones it
/// already lists.
pub(crate) fn merge_lookup_roots(roots: &mut Vec<String>, extra: &[String]) {
    for dir in extra {
        let dir = dir.trim();
        if dir.is_empty() {
            continue;
        }
        if roots
            .iter()
            .any(|existing| lookup_paths_equal(existing, dir))
        {
            continue;
        }
        roots.push(dir.to_string());
    }
}

fn lookup_paths_equal(left: &str, right: &str) -> bool {
    fn normalize(path: &str) -> String {
        path.replace('\\', "/")
            .trim_end_matches('/')
            .to_ascii_lowercase()
    }
    normalize(left) == normalize(right)
}

#[cfg(test)]
mod tests {
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
}
