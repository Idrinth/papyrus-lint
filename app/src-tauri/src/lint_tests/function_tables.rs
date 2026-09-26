use super::super::*;
use tempfile::tempdir;

#[test]
fn project_function_table_reuses_one_table_for_the_same_roots() {
    let dir = tempdir().unwrap();
    let root = dir.path().to_string_lossy().into_owned();
    let additional = vec!["/extra".to_string()];
    let lookup = vec!["/vanilla".to_string()];

    let first = project_function_table(root.clone(), additional.clone(), lookup.clone());
    let second = project_function_table(root, additional, lookup);

    assert!(std::sync::Arc::ptr_eq(&first, &second));
}

#[test]
fn project_function_table_is_distinct_for_different_roots() {
    let first_dir = tempdir().unwrap();
    let second_dir = tempdir().unwrap();

    let first = project_function_table(
        first_dir.path().to_string_lossy().into_owned(),
        Vec::new(),
        Vec::new(),
    );
    let second = project_function_table(
        second_dir.path().to_string_lossy().into_owned(),
        Vec::new(),
        Vec::new(),
    );

    assert!(!std::sync::Arc::ptr_eq(&first, &second));
}

#[test]
fn project_function_table_keys_include_game_and_search_roots() {
    let dir = tempdir().unwrap();
    let root = dir.path().to_string_lossy().into_owned();
    let base = project_function_table_for_game(
        papyrus_lints::Game::Skyrim,
        root.clone(),
        vec!["additional-a".to_string()],
        vec!["lookup-a".to_string()],
    );

    for candidate in [
        project_function_table_for_game(
            papyrus_lints::Game::Fallout4,
            root.clone(),
            vec!["additional-a".to_string()],
            vec!["lookup-a".to_string()],
        ),
        project_function_table_for_game(
            papyrus_lints::Game::Skyrim,
            root.clone(),
            vec!["additional-b".to_string()],
            vec!["lookup-a".to_string()],
        ),
        project_function_table_for_game(
            papyrus_lints::Game::Skyrim,
            root,
            vec!["additional-a".to_string()],
            vec!["lookup-b".to_string()],
        ),
    ] {
        assert!(!std::sync::Arc::ptr_eq(&base, &candidate));
    }
}
