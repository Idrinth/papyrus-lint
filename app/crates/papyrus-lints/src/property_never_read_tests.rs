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
fn flags_property_written_but_never_read() {
    let diagnostics = check(
        "ScriptName Example\n\nInt Property DebugCounter Auto\n\nFunction Tick()\n  DebugCounter += 1\nEndFunction\n",
    );

    assert_eq!(diagnostics.len(), 1);
    assert_eq!(diagnostics[0].line, 3);
    assert!(diagnostics[0].message.starts_with("[info]"));
    assert!(diagnostics[0].message.contains("DebugCounter"));
    assert_eq!(diagnostics[0].rule, RULE);
}

#[test]
fn flags_plain_assignment_without_read() {
    let diagnostics = check(
        "ScriptName Example\n\nInt Property Score Auto\n\nFunction Reset()\n  Score = 0\nEndFunction\n",
    );

    assert_eq!(diagnostics.len(), 1);
    assert!(diagnostics[0].message.contains("Score"));
}

#[test]
fn ignores_property_that_is_also_read() {
    let diagnostics = check(
        "ScriptName Example\n\nInt Property DebugCounter Auto\n\nFunction Tick()\n  DebugCounter += 1\n  Debug.Trace(DebugCounter)\nEndFunction\n",
    );

    assert!(diagnostics.is_empty());
}

#[test]
fn ignores_property_used_only_as_a_read() {
    let diagnostics = check(
        "ScriptName Example\n\nInt Property MaxHits Auto\n\nFunction Tick()\n  Debug.Trace(MaxHits)\nEndFunction\n",
    );

    assert!(diagnostics.is_empty());
}

#[test]
fn ignores_property_that_is_never_referenced() {
    let diagnostics = check(
        "ScriptName Example\n\nInt Property Leftover Auto\n\nFunction Tick()\nEndFunction\n",
    );

    assert!(diagnostics.is_empty());
}

#[test]
fn matches_usage_case_insensitively() {
    let diagnostics = check(
        "ScriptName Example\n\nInt Property DebugCounter Auto\n\nFunction Tick()\n  debugcounter = 1\n  Debug.Trace(DEBUGCOUNTER)\nEndFunction\n",
    );

    assert!(diagnostics.is_empty());
}

#[test]
fn treats_self_qualified_assignment_as_a_write() {
    let diagnostics = check(
        "ScriptName Example\n\nInt Property Score Auto\n\nFunction Reset()\n  self.Score = 0\nEndFunction\n",
    );

    assert_eq!(diagnostics.len(), 1);
}

#[test]
fn treats_self_qualified_read_as_a_read() {
    let diagnostics = check(
        "ScriptName Example\n\nInt Property Score Auto\n\nFunction Reset()\n  self.Score = 0\n  Debug.Trace(self.Score)\nEndFunction\n",
    );

    assert!(diagnostics.is_empty());
}

#[test]
fn flags_write_only_array_property() {
    let diagnostics = check(
        "ScriptName Example\n\nInt[] Property Values Auto\n\nFunction Reset()\n  Values = new Int[1]\nEndFunction\n",
    );

    assert_eq!(diagnostics.len(), 1);
    assert!(diagnostics[0].message.contains("Values"));
}

#[test]
fn skips_property_marked_external() {
    let diagnostics = check(
        "ScriptName Example\n\nInt Property DebugCounter Auto ; @external\n\nFunction Tick()\n  DebugCounter += 1\nEndFunction\n",
    );

    assert!(diagnostics.is_empty());
}

#[test]
fn does_not_treat_default_initializer_as_a_script_write() {
    let diagnostics = check("ScriptName Example\n\nInt Property MyValue = 1 Auto\n");

    assert!(diagnostics.is_empty());
}

#[test]
fn does_not_flag_unrelated_locals() {
    let diagnostics =
        check("ScriptName Example\n\nFunction DoThing()\n  Int DebugCounter = 1\n  DebugCounter = 2\nEndFunction\n");

    assert!(diagnostics.is_empty());
}

#[test]
fn does_not_crash_on_unparseable_source() {
    let diagnostics = check("ScriptName Example\n\nInt Property MyValue = \"unterminated\n");
    assert!(diagnostics.is_empty());
}
