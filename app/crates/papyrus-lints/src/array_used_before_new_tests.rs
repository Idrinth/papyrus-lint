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
fn flags_index_on_uninitialized_local() {
    let diagnostics = check(
        "ScriptName Example\n\nFunction Test()\n    Int[] a\n    a[0] = 1\nEndFunction\n",
    );

    assert_eq!(diagnostics.len(), 1);
    assert_eq!(diagnostics[0].line, 5);
    assert_eq!(diagnostics[0].rule, RULE);
    assert!(diagnostics[0].message.starts_with("[warning]"));
    assert!(diagnostics[0].message.contains("'a'"));
    assert!(diagnostics[0].message.contains("indexing"));
}

#[test]
fn flags_length_on_none_array() {
    let diagnostics = check(
        "ScriptName Example\n\nFunction Test()\n    Int[] a = None\n    Debug.Trace(a.Length)\nEndFunction\n",
    );

    assert_eq!(diagnostics.len(), 1);
    assert_eq!(diagnostics[0].line, 5);
    assert!(diagnostics[0].message.contains(".Length"));
}

#[test]
fn does_not_flag_after_new() {
    let diagnostics = check(
        "ScriptName Example\n\nFunction Test()\n    Int[] a\n    a = new Int[3]\n    a[0] = 1\n    Debug.Trace(a.Length)\nEndFunction\n",
    );

    assert!(diagnostics.is_empty());
}

#[test]
fn does_not_flag_after_call_assignment() {
    let diagnostics = check(
        "ScriptName Example\n\nInt[] Function Make()\n    Return new Int[1]\nEndFunction\n\nFunction Test()\n    Int[] a\n    a = Make()\n    a[0] = 1\nEndFunction\n",
    );

    assert!(diagnostics.is_empty());
}

#[test]
fn does_not_flag_after_early_return_none_guard() {
    let diagnostics = check(
        "ScriptName Example\n\nFunction Test()\n    Int[] a = None\n    If a == None\n        Return\n    EndIf\n    a[0] = 1\nEndFunction\n",
    );

    assert!(diagnostics.is_empty());
}

#[test]
fn flags_inside_equal_none_branch() {
    let diagnostics = check(
        "ScriptName Example\n\nFunction Test()\n    Int[] a = None\n    If a == None\n        a[0] = 1\n    EndIf\nEndFunction\n",
    );

    assert_eq!(diagnostics.len(), 1);
    assert_eq!(diagnostics[0].line, 6);
}

#[test]
fn inherits_none_through_alias() {
    let diagnostics = check(
        "ScriptName Example\n\nFunction Test()\n    Int[] a = None\n    Int[] b = a\n    b[0] = 1\nEndFunction\n",
    );

    assert_eq!(diagnostics.len(), 1);
    assert_eq!(diagnostics[0].line, 6);
}

#[test]
fn does_not_flag_array_property() {
    let diagnostics = check(
        "ScriptName Example\n\nInt[] Property Items Auto\n\nFunction Test()\n    Items[0] = 1\n    Debug.Trace(Items.Length)\nEndFunction\n",
    );

    assert!(diagnostics.is_empty());
}

#[test]
fn does_not_flag_parameter() {
    let diagnostics = check(
        "ScriptName Example\n\nFunction Test(Int[] a)\n    a[0] = 1\nEndFunction\n",
    );

    assert!(diagnostics.is_empty());
}

#[test]
fn does_not_flag_non_array_none() {
    let diagnostics = check(
        "ScriptName Example\n\nFunction Test()\n    Armor a = None\n    a.GetName()\nEndFunction\n",
    );

    assert!(diagnostics.is_empty());
}
