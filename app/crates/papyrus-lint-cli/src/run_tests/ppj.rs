//! `.ppj` (Papyrus Project XML) input handling: `run()` accepts a `.ppj` the
//! same way it accepts an `.achlist`, resolving its `<Folders>`/`<Scripts>`
//! entries into the scripts to lint and its `<Import>` entries into
//! additional cross-script search roots.

use crate::test_support::*;

#[test]
fn lints_every_psc_found_under_a_folder_entry() {
    let dir = tempfile::tempdir().expect("failed to create temp dir");
    write_file(
        &dir.path().join("Source/Scripts/Example.psc"),
        "ScriptName Example\n",
    );
    write_file(
        &dir.path().join("Project.ppj"),
        r#"<PapyrusProject>
    <Imports>
        <Import>.\Source\Scripts</Import>
    </Imports>
    <Folders>
        <Folder>.\Source\Scripts</Folder>
    </Folders>
</PapyrusProject>"#,
    );
    let ppj_path = dir.path().join("Project.ppj");

    let (code, stdout, stderr) = run_captured(&[ppj_path.to_string_lossy().into_owned()]);

    assert_eq!(code, 0, "stderr: {stderr}");
    assert!(stdout.contains("no problems found in 1 script"));
}

#[test]
fn resolves_dotted_script_entries_against_imports() {
    let dir = tempfile::tempdir().expect("failed to create temp dir");
    write_file(
        &dir.path().join("Source/Scripts/MyMod/MyQuestScript.psc"),
        "ScriptName MyQuestScript\n",
    );
    write_file(
        &dir.path().join("Project.ppj"),
        r#"<PapyrusProject>
    <Imports>
        <Import>Source/Scripts</Import>
    </Imports>
    <Scripts>
        <Script>MyMod:MyQuestScript</Script>
    </Scripts>
</PapyrusProject>"#,
    );
    let ppj_path = dir.path().join("Project.ppj");

    let (code, stdout, stderr) = run_captured(&[ppj_path.to_string_lossy().into_owned()]);

    assert_eq!(code, 0, "stderr: {stderr}");
    assert!(stdout.contains("no problems found in 1 script"));
}

#[test]
fn imports_resolve_cross_script_argument_types_outside_the_folder_entries() {
    // The project's own scripts live under `Folders`, but a call into a
    // script that only exists under a separate `<Import>` (e.g. a vendored
    // shared library, or a base game's own vanilla scripts) must still
    // resolve, the same way an achlist's own additional_script_roots would.
    let dir = tempfile::tempdir().expect("failed to create temp dir");
    write_file(
        &dir.path().join("Vendor/Greeter.psc"),
        "ScriptName Greeter\n\nFunction Greet(String name)\nEndFunction\n",
    );
    write_file(
        &dir.path().join("Source/Scripts/Example.psc"),
        "ScriptName Example\n\nGreeter Property Target Auto\n\nFunction Test()\n    Target.Greet(1)\nEndFunction\n",
    );
    write_file(
        &dir.path().join("Project.ppj"),
        r#"<PapyrusProject>
    <Imports>
        <Import>Source/Scripts</Import>
        <Import>Vendor</Import>
    </Imports>
    <Folders>
        <Folder>Source/Scripts</Folder>
    </Folders>
</PapyrusProject>"#,
    );
    let ppj_path = dir.path().join("Project.ppj");

    let (code, stdout, stderr) = run_captured(&[ppj_path.to_string_lossy().into_owned()]);

    assert_eq!(code, 1, "stderr: {stderr}");
    assert!(stdout.contains("[argument-types]"));
}

#[test]
fn folder_no_recurse_skips_nested_scripts() {
    let dir = tempfile::tempdir().expect("failed to create temp dir");
    write_file(&dir.path().join("Scripts/Top.psc"), "ScriptName Top\n");
    write_file(
        &dir.path().join("Scripts/Nested/Deep.psc"),
        "ScriptName Deep\n",
    );
    write_file(
        &dir.path().join("Project.ppj"),
        r#"<PapyrusProject>
    <Folders>
        <Folder NoRecurse="true">Scripts</Folder>
    </Folders>
</PapyrusProject>"#,
    );
    let ppj_path = dir.path().join("Project.ppj");

    let (code, stdout, stderr) = run_captured(&[ppj_path.to_string_lossy().into_owned()]);

    assert_eq!(code, 0, "stderr: {stderr}");
    assert!(stdout.contains("no problems found in 1 script"));
}

#[test]
fn errors_when_ppj_is_missing() {
    let dir = tempfile::tempdir().expect("failed to create temp dir");
    let ppj_path = dir.path().join("missing.ppj");

    let (code, _stdout, stderr) = run_captured(&[ppj_path.to_string_lossy().into_owned()]);

    assert_eq!(code, 2);
    assert!(stderr.starts_with("error:"));
}
