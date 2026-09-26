use super::*;
use std::fs;
use tempfile::tempdir;

#[test]
fn project_root_uses_the_first_entry_in_a_supported_script_tree() {
    let dir = tempdir().unwrap();
    let first_root = dir.path().join("first-project");
    let second_root = dir.path().join("second-project");
    let first_script = first_root.join("scripts/source/nested/First.psc");
    let second_script = second_root.join("source/scripts/Second.psc");

    assert_eq!(
        find_project_root(
            vec![
                dir.path()
                    .join("unmatched/Example.psc")
                    .to_string_lossy()
                    .into_owned(),
                first_script.to_string_lossy().into_owned(),
                second_script.to_string_lossy().into_owned(),
            ],
            "fallback".to_string(),
        ),
        first_root.to_string_lossy()
    );
}

#[test]
fn project_root_returns_the_fallback_when_no_entry_matches() {
    assert_eq!(
        find_project_root(
            vec!["custom/source/Example.psc".to_string()],
            "selected/project".to_string(),
        ),
        "selected/project"
    );
    assert_eq!(
        find_project_root(Vec::new(), "empty/project".to_string()),
        "empty/project"
    );
}

#[test]
fn psc_project_root_command_handles_conventional_and_fallback_layouts() {
    let separator = std::path::MAIN_SEPARATOR;

    assert_eq!(
        find_psc_project_root_for_path(format!(
            "project{separator}Scripts{separator}Source{separator}nested{separator}Example.psc"
        )),
        "project"
    );
    assert_eq!(
        find_psc_project_root_for_path(format!(
            "project{separator}custom{separator}source{separator}Example.psc"
        )),
        "project"
    );
    assert_eq!(
        find_psc_project_root_for_path("Example.psc".to_string()),
        "."
    );
}

#[test]
fn psc_project_root_command_prefers_a_configured_ancestor_over_the_directory_pair() {
    let dir = tempdir().unwrap();
    let configured_root = dir.path().join("configured-project");
    let conventional_root = configured_root.join("Data");
    let script = conventional_root.join("Scripts/Source/Nested/Example.psc");

    fs::create_dir_all(script.parent().unwrap()).unwrap();
    fs::write(configured_root.join("papyrus-lint.yaml"), "rules: {}\n").unwrap();
    fs::write(&script, "ScriptName Example\n").unwrap();

    assert_eq!(
        find_psc_project_root_for_path(script.to_string_lossy().into_owned()),
        configured_root.to_string_lossy()
    );
}

#[test]
fn psc_project_root_command_recognizes_yml_configs_in_custom_layouts() {
    let dir = tempdir().unwrap();
    let configured_root = dir.path().join("configured-project");
    let script = configured_root.join("custom/deep/layout/Example.psc");

    fs::create_dir_all(script.parent().unwrap()).unwrap();
    fs::write(configured_root.join("papyrus-lint.yml"), "rules: {}\n").unwrap();
    fs::write(&script, "ScriptName Example\n").unwrap();

    assert_eq!(
        find_psc_project_root_for_path(script.to_string_lossy().into_owned()),
        configured_root.to_string_lossy()
    );
}

#[test]
fn project_root_accepts_mixed_case_directory_pairs_and_directory_entries() {
    let separator = std::path::MAIN_SEPARATOR;

    assert_eq!(
        find_project_root(
            vec![format!(
                "project{separator}SoUrCe{separator}ScRiPtS{separator}Nested"
            )],
            "fallback".to_string(),
        ),
        "project"
    );
}
