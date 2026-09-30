use super::*;

fn check(source: &str) -> Vec<Diagnostic> {
    let ast = papyrus_parser::parse(source).ok();
    let tokens = papyrus_parser::tokenize(source).ok();
    super::check(
        source,
        ast.as_ref(),
        tokens.as_deref(),
        &crate::config::Config::default(),
        &mut crate::external_signatures::NoExternalSignatures,
    )
}

#[test]
fn flags_full_property_whose_backing_is_never_written() {
    let diagnostics = check(
        "ScriptName Example\n\
         Int _count\n\
         Int Property Count\n\
           Int Function Get()\n\
             Return _count\n\
           EndFunction\n\
           Function Set(Int value)\n\
             _count = value\n\
           EndFunction\n\
         EndProperty\n",
    );

    assert_eq!(diagnostics.len(), 1);
    assert_eq!(diagnostics[0].line, 3);
    assert_eq!(diagnostics[0].rule, RULE);
    assert!(diagnostics[0].message.starts_with("[warning]"));
    assert!(diagnostics[0].message.contains("Count"));
}

#[test]
fn ignores_write_to_backing_outside_the_property() {
    let diagnostics = check(
        "ScriptName Example\n\
         Int _count\n\
         Int Property Count\n\
           Int Function Get()\n\
             Return _count\n\
           EndFunction\n\
           Function Set(Int value)\n\
             _count = value\n\
           EndFunction\n\
         EndProperty\n\
         Function Init()\n\
           _count = 1\n\
         EndFunction\n",
    );

    assert!(diagnostics.is_empty());
}

#[test]
fn ignores_assignment_to_the_property_name() {
    let diagnostics = check(
        "ScriptName Example\n\
         Int _count\n\
         Int Property Count\n\
           Int Function Get()\n\
             Return _count\n\
           EndFunction\n\
           Function Set(Int value)\n\
             _count = value\n\
           EndFunction\n\
         EndProperty\n\
         Function Init()\n\
           Count = 1\n\
         EndFunction\n",
    );

    assert!(diagnostics.is_empty());
}

#[test]
fn ignores_variable_initializer_as_a_write() {
    let diagnostics = check(
        "ScriptName Example\n\
         Int _count = 3\n\
         Int Property Count\n\
           Int Function Get()\n\
             Return _count\n\
           EndFunction\n\
         EndProperty\n",
    );

    assert!(diagnostics.is_empty());
}

#[test]
fn ignores_auto_and_autoreadonly_properties() {
    let diagnostics = check(
        "ScriptName Example\n\
         Int Property Count Auto\n\
         Int Property Frozen = 1 AutoReadOnly\n",
    );

    assert!(diagnostics.is_empty());
}

#[test]
fn ignores_computed_property_with_no_backing_field() {
    let diagnostics = check(
        "ScriptName Example\n\
         Int[] items\n\
         Int Property Count\n\
           Int Function Get()\n\
             Return items.Length\n\
           EndFunction\n\
         EndProperty\n",
    );

    assert!(diagnostics.is_empty());
}

#[test]
fn ignores_external_annotation() {
    let diagnostics = check(
        "ScriptName Example\n\
         Int _count\n\
         Int Property Count ; @external\n\
           Int Function Get()\n\
             Return _count\n\
           EndFunction\n\
           Function Set(Int value)\n\
             _count = value\n\
           EndFunction\n\
         EndProperty\n",
    );

    assert!(diagnostics.is_empty());
}

#[test]
fn matches_writes_case_insensitively() {
    let diagnostics = check(
        "ScriptName Example\n\
         Int _count\n\
         Int Property Count\n\
           Int Function Get()\n\
             Return _count\n\
           EndFunction\n\
         EndProperty\n\
         Function Init()\n\
           _COUNT = 2\n\
         EndFunction\n",
    );

    assert!(diagnostics.is_empty());
}

#[test]
fn does_not_treat_member_on_another_object_as_a_write() {
    let diagnostics = check(
        "ScriptName Example\n\
         Int _count\n\
         Int Property Count\n\
           Int Function Get()\n\
             Return _count\n\
           EndFunction\n\
           Function Set(Int value)\n\
             _count = value\n\
           EndFunction\n\
         EndProperty\n\
         Function Tick(ObjectReference other)\n\
           other.Count = 1\n\
         EndFunction\n",
    );

    assert_eq!(diagnostics.len(), 1);
    assert!(diagnostics[0].message.contains("Count"));
}
