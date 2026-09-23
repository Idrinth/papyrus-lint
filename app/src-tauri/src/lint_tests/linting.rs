use super::super::*;
use tempfile::tempdir;

#[test]
fn lint_psc_file_lints_source_from_disk() {
    let dir = tempdir().unwrap();
    let path = dir.path().join("Example.psc");
    std::fs::write(
        &path,
        "ScriptName Example\n\nFunction Run()\n    Game.GetPlayer()\nEndFunction\n",
    )
    .unwrap();

    let diagnostics = lint_psc_file(
        path.to_string_lossy().into_owned(),
        ProjectLintContext {
            root: dir.path().to_string_lossy().into_owned(),
            ..Default::default()
        },
    )
    .unwrap();

    assert!(diagnostics
        .iter()
        .any(|diagnostic| diagnostic.rule == "forbidden-functions"));
}

#[test]
fn preload_project_scripts_lets_lint_psc_file_resolve_a_sibling_immediately() {
    let dir = tempdir().unwrap();
    let source_dir = dir.path().join("scripts/source");
    std::fs::create_dir_all(&source_dir).unwrap();
    let base_path = source_dir.join("BaseScript.psc");
    std::fs::write(
        &base_path,
        "ScriptName BaseScript\n\nFunction DoIt()\nEndFunction\n",
    )
    .unwrap();
    let derived_path = source_dir.join("DerivedScript.psc");
    std::fs::write(
        &derived_path,
        "ScriptName DerivedScript extends BaseScript\n\nFunction DoIt()\nEndFunction\n",
    )
    .unwrap();

    let context = ProjectLintContext {
        root: dir.path().to_string_lossy().into_owned(),
        ..Default::default()
    };

    preload_project_scripts(
        vec![
            base_path.to_string_lossy().into_owned(),
            derived_path.to_string_lossy().into_owned(),
        ],
        context.clone(),
    );

    let diagnostics = lint_psc_file(derived_path.to_string_lossy().into_owned(), context).unwrap();

    assert!(diagnostics
        .iter()
        .any(|diagnostic| diagnostic.rule == "function-override"));
}

#[test]
fn lint_psc_file_resolves_argument_types_through_additional_script_roots() {
    let extra = tempdir().unwrap();
    std::fs::write(
        extra.path().join("Helpers.psc"),
        "ScriptName Helpers\n\nFunction NeedInt(Int count)\nEndFunction\n",
    )
    .unwrap();
    let dir = tempdir().unwrap();
    let path = dir.path().join("Caller.psc");
    std::fs::write(
            &path,
            "ScriptName Caller\n\nFunction Run(Helpers helper)\n    helper.NeedInt(\"nope\")\nEndFunction\n",
        )
        .unwrap();
    let extra_root = extra.path().to_string_lossy().into_owned();

    let without_roots = lint_psc_file(
        path.to_string_lossy().into_owned(),
        ProjectLintContext {
            root: dir.path().to_string_lossy().into_owned(),
            ..Default::default()
        },
    )
    .unwrap();
    assert!(without_roots
        .iter()
        .all(|diagnostic| diagnostic.rule != "argument-types"));

    let with_roots = lint_psc_file(
        path.to_string_lossy().into_owned(),
        ProjectLintContext {
            root: dir.path().to_string_lossy().into_owned(),
            additional_roots: vec![extra_root],
            ..Default::default()
        },
    )
    .unwrap();
    assert!(with_roots.iter().any(|diagnostic| {
        diagnostic.rule == "argument-types"
            && diagnostic.message.contains("expects Int")
            && diagnostic.message.contains("got String")
    }));
}
