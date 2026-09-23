use super::*;

#[test]
fn display_path_only_shortens_paths_when_requested() {
    let path = Path::new("project/scripts/source/Example.psc");
    let root = Path::new("project");

    assert_eq!(
        display_path(path, root, true),
        Path::new("scripts/source/Example.psc")
            .display()
            .to_string()
    );
    assert_eq!(display_path(path, root, false), path.display().to_string());
}

#[test]
fn display_path_leaves_paths_outside_the_project_root_unchanged() {
    let path = Path::new("other-project/scripts/source/Example.psc");

    assert_eq!(
        display_path(path, Path::new("project"), true),
        path.display().to_string()
    );
}

#[test]
fn candidate_pair_root_recognizes_every_supported_directory_order() {
    let root = Path::new("project");

    assert_eq!(
        find_candidate_pair_root(&root.join("scripts/source/Example.psc")),
        Some(root.to_path_buf())
    );
    assert_eq!(
        find_candidate_pair_root(&root.join("source/scripts/Example.psc")),
        Some(root.to_path_buf())
    );
}

#[test]
fn candidate_pair_root_is_case_insensitive_and_supports_nested_scripts() {
    let script = Path::new("project/SCRIPTS/Source/User/Example.psc");

    assert_eq!(
        find_candidate_pair_root(script),
        Some(PathBuf::from("project"))
    );
}

#[test]
fn psc_project_root_uses_the_legacy_fallback_without_a_candidate_pair() {
    assert_eq!(
        find_psc_project_root(Path::new("project/custom/source/Example.psc")),
        PathBuf::from("project")
    );
    assert_eq!(
        find_psc_project_root(Path::new("Example.psc")),
        PathBuf::from(".")
    );
}

#[test]
fn psc_project_root_finds_a_config_file_when_no_candidate_pair_exists() {
    let dir = tempfile::tempdir().expect("failed to create temp dir");
    let project_root = dir.path().join("MyMod");
    let script_path = project_root.join("scripts/User/Example.psc");
    std::fs::create_dir_all(script_path.parent().unwrap()).expect("failed to create dirs");
    std::fs::write(project_root.join("papyrus-lint.yaml"), "semicolon: true\n")
        .expect("failed to write config file");

    assert_eq!(find_psc_project_root(&script_path), project_root);
}

#[test]
fn psc_project_root_uses_a_candidate_pair_when_no_config_file_exists() {
    let dir = tempfile::tempdir().expect("failed to create temp dir");
    let outer_root = dir.path().join("Outer");
    let inner_root = outer_root.join("scripts/source");
    let script_path = inner_root.join("Example.psc");
    std::fs::create_dir_all(&inner_root).expect("failed to create dirs");

    assert_eq!(find_psc_project_root(&script_path), outer_root);
}

#[test]
fn psc_project_root_prefers_a_config_file_above_a_candidate_pair_root() {
    // A common Bethesda mod layout: the workspace/project root (with its
    // own papyrus-lint.yaml) sits one level above a `Data` folder whose
    // `Scripts/Source` pair would otherwise make find_candidate_pair_root
    // land on `Data` instead — the exact case VS Code/Sublime hit, since
    // they invoke the CLI on a single .psc file.
    let dir = tempfile::tempdir().expect("failed to create temp dir");
    let workspace_root = dir.path().join("Workspace");
    let script_dir = workspace_root.join("Data/Scripts/Source");
    let script_path = script_dir.join("Example.psc");
    std::fs::create_dir_all(&script_dir).expect("failed to create dirs");
    std::fs::write(
        workspace_root.join("papyrus-lint.yaml"),
        "semicolon: true\n",
    )
    .expect("failed to write config file");

    assert_eq!(find_psc_project_root(&script_path), workspace_root);
}

#[test]
fn psc_project_root_falls_back_to_the_legacy_guess_without_any_config_file() {
    let dir = tempfile::tempdir().expect("failed to create temp dir");
    let script_path = dir.path().join("MyMod/scripts/User/Example.psc");
    std::fs::create_dir_all(script_path.parent().unwrap()).expect("failed to create dirs");

    assert_eq!(
        find_psc_project_root(&script_path),
        dir.path().join("MyMod")
    );
}
