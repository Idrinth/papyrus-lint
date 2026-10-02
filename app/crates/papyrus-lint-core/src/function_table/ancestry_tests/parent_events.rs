use super::super::super::test_support::{diagnostics_for, write_script};
use super::super::*;

fn table(root: &std::path::Path) -> FunctionTable {
    FunctionTable::new(root.to_path_buf())
}

#[test]
fn non_empty_parent_event_needs_a_call() {
    let root = tempfile::tempdir().expect("failed to create temp dir");
    write_script(
        root.path(),
        "GrandBase",
        "ScriptName GrandBase\n\nEvent OnInit()\n    Debug.Trace(\"setup\")\nEndEvent\n\nEvent OnHit()\n    Debug.Trace(\"hit\")\nEndEvent\n",
    );

    let mut scripts = table(root.path());

    assert_eq!(
        scripts.parent_event_needs_call("GrandBase", "OnInit"),
        Some(true)
    );
    assert_eq!(
        scripts.parent_event_needs_call("grandbase", "onhit"),
        Some(true)
    );
    assert_eq!(
        scripts.parent_event_needs_call("GrandBase", "OnMissing"),
        Some(false)
    );
}

#[test]
fn empty_parent_event_does_not_need_a_call_even_when_a_grandparent_has_a_body() {
    let root = tempfile::tempdir().expect("failed to create temp dir");
    write_script(
        root.path(),
        "GrandBase",
        "ScriptName GrandBase\n\nEvent OnInit()\n    Debug.Trace(\"setup\")\nEndEvent\n\nEvent OnLoad()\n    Debug.Trace(\"load\")\nEndEvent\n",
    );
    write_script(
        root.path(),
        "MidEmpty",
        "ScriptName MidEmpty Extends GrandBase\n\nEvent OnInit()\nEndEvent\n",
    );
    write_script(
        root.path(),
        "MidReturn",
        "ScriptName MidReturn Extends GrandBase\n\nEvent OnLoad()\n    Return\nEndEvent\n",
    );
    write_script(
        root.path(),
        "MidPassthrough",
        "ScriptName MidPassthrough Extends GrandBase\n",
    );

    let mut scripts = table(root.path());

    assert_eq!(
        scripts.parent_event_needs_call("MidEmpty", "OnInit"),
        Some(false)
    );
    assert_eq!(
        scripts.parent_event_needs_call("MidReturn", "OnLoad"),
        Some(false)
    );
    assert_eq!(
        scripts.parent_event_needs_call("MidPassthrough", "OnInit"),
        Some(true)
    );
    assert!(matches!(
        scripts.parent_event_needs_call_cached("MidEmpty", "OnInit"),
        CacheProbe::Hit(Some(false))
    ));
}

#[test]
fn native_script_and_native_event_do_not_need_a_call() {
    let root = tempfile::tempdir().expect("failed to create temp dir");
    write_script(
        root.path(),
        "EngineScript",
        "ScriptName EngineScript Native\n\nEvent OnInit()\n    Debug.Trace(\"engine\")\nEndEvent\n",
    );
    write_script(
        root.path(),
        "NativeEventScript",
        "ScriptName NativeEventScript\n\nEvent OnInit() Native\n",
    );

    let mut scripts = table(root.path());

    assert_eq!(
        scripts.parent_event_needs_call("EngineScript", "OnInit"),
        Some(false)
    );
    assert_eq!(
        scripts.parent_event_needs_call("NativeEventScript", "OnInit"),
        Some(false)
    );
}

#[test]
fn bundled_empty_engine_event_does_not_need_a_call() {
    let root = tempfile::tempdir().expect("failed to create temp dir");
    let mut scripts = table(root.path());

    assert_eq!(
        scripts.parent_event_needs_call("Actor", "OnPlayerLoadGame"),
        Some(false)
    );
    assert_eq!(
        scripts.parent_event_needs_call("ObjectReference", "OnLoad"),
        Some(false)
    );
}

#[test]
fn missing_parent_is_not_a_guess() {
    let root = tempfile::tempdir().expect("failed to create temp dir");
    write_script(
        root.path(),
        "Child",
        "ScriptName Child Extends MissingParent\n",
    );

    let mut scripts = table(root.path());

    assert_eq!(scripts.parent_event_needs_call("Child", "OnInit"), None);
}

#[test]
fn empty_parent_event_does_not_warn_its_children() {
    let root = tempfile::tempdir().expect("failed to create temp dir");
    write_script(
        root.path(),
        "GrandBase",
        "ScriptName GrandBase\n\nEvent OnInit()\n    Debug.Trace(\"setup\")\nEndEvent\n\nEvent OnHit()\n    Debug.Trace(\"hit\")\nEndEvent\n",
    );
    write_script(
        root.path(),
        "MidEmpty",
        "ScriptName MidEmpty Extends GrandBase\n\nEvent OnInit()\nEndEvent\n\nEvent OnHit()\n    Debug.Trace(\"mid\")\nEndEvent\n",
    );

    let mut scripts = table(root.path());
    let child_of_empty = "\
ScriptName ChildOfEmpty Extends MidEmpty

Event OnInit()
    MySetup()
EndEvent

Event OnHit()
    MyHit()
EndEvent
";
    let child_of_grand = "\
ScriptName ChildOfGrand Extends GrandBase

Event OnInit()
    MySetup()
EndEvent

Event OnHit()
    Parent.OnHit()
EndEvent
";

    let empty_parent = diagnostics_for(
        "missing-parent-call-in-override",
        child_of_empty,
        &mut scripts,
    );
    let lines: Vec<_> = empty_parent
        .iter()
        .map(|diagnostic| diagnostic.line)
        .collect();
    assert_eq!(
        lines,
        vec![7],
        "OnInit's empty parent must stay quiet; OnHit's body must warn: {empty_parent:?}"
    );

    let grand = diagnostics_for(
        "missing-parent-call-in-override",
        child_of_grand,
        &mut scripts,
    );
    assert_eq!(grand.len(), 1);
    assert_eq!(grand[0].line, 3);
    assert!(grand[0].message.contains("Parent.OnInit()"));
}
