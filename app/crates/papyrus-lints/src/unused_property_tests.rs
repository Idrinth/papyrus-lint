use super::*;

#[test]
fn flags_unused_auto_property() {
    let diagnostics = check(
        "ScriptName Example\n\nInt Property MyValue = 1 Auto\n\nFunction DoThing()\nEndFunction\n",
    );

    assert_eq!(diagnostics.len(), 1);
    assert_eq!(diagnostics[0].line, 3);
    assert!(diagnostics[0].message.starts_with("[warning]"));
    assert!(diagnostics[0].message.contains("MyValue"));
}

#[test]
fn ignores_property_used_elsewhere_in_the_script() {
    let diagnostics = check(
            "ScriptName Example\n\nInt Property MyValue = 1 Auto\n\nFunction DoThing()\n  Debug.Trace(MyValue)\nEndFunction\n",
        );

    assert!(diagnostics.is_empty());
}

#[test]
fn matches_property_usage_case_insensitively() {
    let diagnostics = check(
            "ScriptName Example\n\nInt Property MyValue = 1 Auto\n\nFunction DoThing()\n  Debug.Trace(myvalue)\nEndFunction\n",
        );

    assert!(diagnostics.is_empty());
}

#[test]
fn flags_unused_autoreadonly_property() {
    let diagnostics = check("ScriptName Example\n\nInt Property MyValue = 1 AutoReadOnly\n");

    assert_eq!(diagnostics.len(), 1);
    assert!(diagnostics[0].message.contains("MyValue"));
}

#[test]
fn flags_unused_array_typed_property() {
    let diagnostics = check("ScriptName Example\n\nInt[] Property Values Auto\n");

    assert_eq!(diagnostics.len(), 1);
    assert!(diagnostics[0].message.contains("Values"));
}

#[test]
fn flags_unused_full_property() {
    let diagnostics = check(
            "ScriptName Example\n\nInt Property MyValue\n  Int Function Get()\n    Return 1\n  EndFunction\nEndProperty\n",
        );

    assert_eq!(diagnostics.len(), 1);
    assert!(diagnostics[0].message.contains("MyValue"));
}

#[test]
fn ignores_property_used_only_qualified_through_self() {
    let diagnostics = check(
            "ScriptName Example\n\nInt Property MyValue = 1 Auto\n\nFunction DoThing()\n  Debug.Trace(self.MyValue)\nEndFunction\n",
        );

    assert!(diagnostics.is_empty());
}

#[test]
fn does_not_flag_unrelated_declarations() {
    let diagnostics =
        check("ScriptName Example\n\nFunction DoThing()\n  Int MyValue = 1\nEndFunction\n");

    assert!(diagnostics.is_empty());
}

#[test]
fn does_not_crash_on_unparseable_source() {
    let diagnostics = check("ScriptName Example\n\nInt Property MyValue = \"unterminated\n");
    assert!(diagnostics.is_empty());
}
