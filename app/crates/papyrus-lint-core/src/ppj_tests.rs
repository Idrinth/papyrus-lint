use super::*;

fn write_ppj(dir: &Path, name: &str, contents: &str) -> PathBuf {
    let path = dir.join(name);
    fs::write(&path, contents).expect("failed to write test ppj file");
    path
}

#[test]
fn parses_the_real_world_example_with_imports_and_a_folder() {
    let dir = tempfile::tempdir().expect("failed to create temp dir");
    fs::create_dir_all(dir.path().join("Source/Scripts")).unwrap();
    fs::write(dir.path().join("Source/Scripts/Foo.psc"), "ScriptName Foo").unwrap();
    let ppj_path = write_ppj(
        dir.path(),
        "Project.ppj",
        r#"<?xml version='1.0'?><!-- Game is either sse, tesv, or fo4 -->
<PapyrusProject xmlns="PapyrusProject.xsd"
    Flags="TESV_Papyrus_Flags.flg"
    Game="sse"
    Output="Scripts"
    Optimize="false"
    Release="false"
    Final="false">
    <Imports>
        <Import>.\Source\Scripts</Import>
        <Import>C:\Program Files (x86)\Steam\steamapps\common\Skyrim Special Edition\Data\Source\Scripts</Import>
    </Imports>
    <Folders>
        <Folder>.\Source\Scripts</Folder>
    </Folders>
</PapyrusProject>"#,
    );

    let project = parse_ppj(&ppj_path).expect("parsing should succeed");

    assert_eq!(project.game.as_deref(), Some("sse"));
    assert_eq!(project.flags.as_deref(), Some("TESV_Papyrus_Flags.flg"));
    assert_eq!(project.output, Some(dir.path().join("Scripts")));
    assert_eq!(
        project.imports,
        vec![
            dir.path().join("./Source/Scripts"),
            PathBuf::from(
                "C:/Program Files (x86)/Steam/steamapps/common/Skyrim Special Edition/Data/Source/Scripts"
            ),
        ]
    );
    assert_eq!(
        project.scripts,
        vec![dir.path().join("./Source/Scripts/Foo.psc")]
    );
}

#[test]
fn folder_recurses_by_default() {
    let dir = tempfile::tempdir().expect("failed to create temp dir");
    fs::create_dir_all(dir.path().join("Scripts/Nested")).unwrap();
    fs::write(dir.path().join("Scripts/Top.psc"), "ScriptName Top").unwrap();
    fs::write(
        dir.path().join("Scripts/Nested/Deep.psc"),
        "ScriptName Deep",
    )
    .unwrap();
    let ppj_path = write_ppj(
        dir.path(),
        "Project.ppj",
        r#"<PapyrusProject>
    <Folders>
        <Folder>Scripts</Folder>
    </Folders>
</PapyrusProject>"#,
    );

    let project = parse_ppj(&ppj_path).expect("parsing should succeed");

    assert_eq!(
        project.scripts,
        vec![
            dir.path().join("Scripts/Nested/Deep.psc"),
            dir.path().join("Scripts/Top.psc"),
        ]
    );
}

#[test]
fn folder_no_recurse_skips_subdirectories() {
    let dir = tempfile::tempdir().expect("failed to create temp dir");
    fs::create_dir_all(dir.path().join("Scripts/Nested")).unwrap();
    fs::write(dir.path().join("Scripts/Top.psc"), "ScriptName Top").unwrap();
    fs::write(
        dir.path().join("Scripts/Nested/Deep.psc"),
        "ScriptName Deep",
    )
    .unwrap();
    let ppj_path = write_ppj(
        dir.path(),
        "Project.ppj",
        r#"<PapyrusProject>
    <Folders>
        <Folder NoRecurse="true">Scripts</Folder>
    </Folders>
</PapyrusProject>"#,
    );

    let project = parse_ppj(&ppj_path).expect("parsing should succeed");

    assert_eq!(project.scripts, vec![dir.path().join("Scripts/Top.psc")]);
}

#[test]
fn script_entries_resolve_dotted_names_against_imports() {
    let dir = tempfile::tempdir().expect("failed to create temp dir");
    fs::create_dir_all(dir.path().join("Source/Scripts/MyMod")).unwrap();
    fs::write(
        dir.path().join("Source/Scripts/MyMod/MyQuestScript.psc"),
        "ScriptName MyMod:MyQuestScript",
    )
    .unwrap();
    let ppj_path = write_ppj(
        dir.path(),
        "Project.ppj",
        r#"<PapyrusProject>
    <Imports>
        <Import>Source\Scripts</Import>
    </Imports>
    <Scripts>
        <Script>MyMod:MyQuestScript</Script>
    </Scripts>
</PapyrusProject>"#,
    );

    let project = parse_ppj(&ppj_path).expect("parsing should succeed");

    assert_eq!(
        project.scripts,
        vec![dir.path().join("Source/Scripts/MyMod/MyQuestScript.psc")]
    );
}

#[test]
fn unresolved_script_entry_falls_back_to_first_import() {
    let dir = tempfile::tempdir().expect("failed to create temp dir");
    let ppj_path = write_ppj(
        dir.path(),
        "Project.ppj",
        r#"<PapyrusProject>
    <Imports>
        <Import>Source\Scripts</Import>
    </Imports>
    <Scripts>
        <Script>Missing</Script>
    </Scripts>
</PapyrusProject>"#,
    );

    let project = parse_ppj(&ppj_path).expect("parsing should succeed");

    assert_eq!(
        project.scripts,
        vec![dir.path().join("Source/Scripts").join("Missing.psc")]
    );
}

#[test]
fn script_entry_falls_back_to_ppj_directory_without_any_import() {
    let dir = tempfile::tempdir().expect("failed to create temp dir");
    let ppj_path = write_ppj(
        dir.path(),
        "Project.ppj",
        r#"<PapyrusProject>
    <Scripts>
        <Script>Standalone</Script>
    </Scripts>
</PapyrusProject>"#,
    );

    let project = parse_ppj(&ppj_path).expect("parsing should succeed");

    assert_eq!(project.scripts, vec![dir.path().join("Standalone.psc")]);
}

#[test]
fn absolute_import_is_preserved_without_joining_the_ppj_directory() {
    let dir = tempfile::tempdir().expect("failed to create temp dir");
    let absolute = dir.path().join("elsewhere/Source/Scripts");
    let ppj_path = write_ppj(
        dir.path(),
        "Project.ppj",
        &format!(
            r#"<PapyrusProject>
    <Imports>
        <Import>{}</Import>
    </Imports>
</PapyrusProject>"#,
            absolute.display()
        ),
    );

    let project = parse_ppj(&ppj_path).expect("parsing should succeed");

    assert_eq!(project.imports, vec![absolute]);
}

#[test]
fn windows_drive_letter_import_is_treated_as_absolute_on_any_host() {
    let dir = tempfile::tempdir().expect("failed to create temp dir");
    let ppj_path = write_ppj(
        dir.path(),
        "Project.ppj",
        r#"<PapyrusProject>
    <Imports>
        <Import>C:\Games\Skyrim\Data\Source\Scripts</Import>
        <Import>C:/Games/Skyrim/Data/Source/Scripts2</Import>
    </Imports>
</PapyrusProject>"#,
    );

    let project = parse_ppj(&ppj_path).expect("parsing should succeed");

    assert_eq!(
        project.imports,
        vec![
            PathBuf::from("C:/Games/Skyrim/Data/Source/Scripts"),
            PathBuf::from("C:/Games/Skyrim/Data/Source/Scripts2"),
        ]
    );
}

#[test]
fn windows_unc_import_is_treated_as_absolute() {
    let dir = tempfile::tempdir().expect("failed to create temp dir");
    let ppj_path = write_ppj(
        dir.path(),
        "Project.ppj",
        r#"<PapyrusProject>
    <Imports>
        <Import>\\server\share\Scripts</Import>
    </Imports>
</PapyrusProject>"#,
    );

    let project = parse_ppj(&ppj_path).expect("parsing should succeed");

    assert_eq!(
        project.imports,
        vec![PathBuf::from("//server/share/Scripts")]
    );
}

#[test]
fn no_output_flags_or_game_attribute_are_none() {
    let dir = tempfile::tempdir().expect("failed to create temp dir");
    let ppj_path = write_ppj(
        dir.path(),
        "Project.ppj",
        "<PapyrusProject></PapyrusProject>",
    );

    let project = parse_ppj(&ppj_path).expect("parsing should succeed");

    assert_eq!(project.output, None);
    assert_eq!(project.flags, None);
    assert_eq!(project.game, None);
    assert!(project.imports.is_empty());
    assert!(project.scripts.is_empty());
}

#[test]
fn errors_on_missing_file() {
    let missing_path = PathBuf::from("/nonexistent/path/does-not-exist.ppj");

    let result = parse_ppj(&missing_path);

    assert!(matches!(result, Err(PpjError::Io(_))));
}

#[test]
fn errors_on_invalid_xml() {
    let dir = tempfile::tempdir().expect("failed to create temp dir");
    let ppj_path = write_ppj(dir.path(), "broken.ppj", "not xml at all <<<");

    let result = parse_ppj(&ppj_path);

    assert!(matches!(result, Err(PpjError::Xml(_))));
}

#[test]
fn errors_when_root_element_is_not_papyrus_project() {
    let dir = tempfile::tempdir().expect("failed to create temp dir");
    let ppj_path = write_ppj(
        dir.path(),
        "wrong-root.ppj",
        "<SomethingElse></SomethingElse>",
    );

    let result = parse_ppj(&ppj_path);

    assert!(matches!(result, Err(PpjError::NotAPapyrusProject)));
}

#[test]
fn io_error_display_includes_context_and_underlying_error() {
    let error = PpjError::from(std::io::Error::new(
        std::io::ErrorKind::PermissionDenied,
        "access denied",
    ));

    assert_eq!(error.to_string(), "failed to read ppj file: access denied");
}

#[test]
fn not_a_papyrus_project_error_display() {
    assert_eq!(
        PpjError::NotAPapyrusProject.to_string(),
        "failed to parse ppj file: root element is not <PapyrusProject>"
    );
}

#[test]
fn xml_error_display_includes_context_and_underlying_error() {
    let error = PpjError::from(roxmltree::Document::parse("<broken>").unwrap_err());
    let underlying = match &error {
        PpjError::Xml(error) => error.to_string(),
        other => panic!("expected an XML error, got {other:?}"),
    };

    assert_eq!(
        error.to_string(),
        format!("failed to parse ppj file: {underlying}")
    );
}

#[test]
fn empty_folder_and_script_elements_are_ignored() {
    let dir = tempfile::tempdir().expect("failed to create temp dir");
    let ppj_path = write_ppj(
        dir.path(),
        "Project.ppj",
        r#"<PapyrusProject>
    <Folders><Folder/></Folders>
    <Scripts><Script/></Scripts>
</PapyrusProject>"#,
    );

    let project = parse_ppj(&ppj_path).expect("parsing should succeed");

    assert!(project.scripts.is_empty());
}
