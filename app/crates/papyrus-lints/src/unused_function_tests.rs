use super::*;

use crate::external_signatures::{ExternalSignatures, ParamInfo};

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
fn flags_a_trailing_private_function_with_no_call_sites() {
    let diagnostics = check(
        "ScriptName Example\n\nFunction HelperNeverCalled(Int x) ; @private\n    Debug.Trace(x)\nEndFunction\n",
    );

    assert_eq!(diagnostics.len(), 1);
    assert_eq!(diagnostics[0].line, 3);
    assert_eq!(diagnostics[0].rule, RULE);
    assert!(diagnostics[0].message.starts_with("[info]"));
    assert!(diagnostics[0].message.contains("'HelperNeverCalled'"));
}

#[test]
fn flags_a_preceding_private_annotation() {
    let diagnostics = check(
        "ScriptName Example\n\n; @private\nFunction HelperNeverCalled(Int x)\n    Debug.Trace(x)\nEndFunction\n",
    );

    assert_eq!(diagnostics.len(), 1);
    assert!(diagnostics[0].message.contains("'HelperNeverCalled'"));
}

#[test]
fn flags_a_preceding_internal_annotation() {
    let diagnostics = check(
        "ScriptName Example\n\n; @internal helper\nFunction HelperNeverCalled()\nEndFunction\n",
    );

    assert_eq!(diagnostics.len(), 1);
}

#[test]
fn flags_a_starfield_internal_function() {
    let source = "ScriptName Example\n\nFunction HelperNeverCalled() Internal\nEndFunction\n";
    let ast =
        papyrus_parser::parse_with_mode(source, papyrus_parser::parser::GameEdition::Starfield)
            .ok();
    let tokens = papyrus_parser::tokenize(source).ok();
    let diagnostics = super::check(
        source,
        ast.as_ref(),
        tokens.as_deref(),
        &crate::config::Config::default(),
        &mut crate::external_signatures::NoExternalSignatures,
    );

    assert_eq!(diagnostics.len(), 1);
}

#[test]
fn does_not_flag_a_public_function_with_no_call_sites() {
    let diagnostics = check("ScriptName Example\n\nFunction PublicApi()\nEndFunction\n");

    assert!(diagnostics.is_empty());
}

#[test]
fn does_not_flag_a_protected_function_with_no_call_sites() {
    let diagnostics =
        check("ScriptName Example\n\nFunction MaybeUsedByChild() ; @protected\nEndFunction\n");

    assert!(diagnostics.is_empty());
}

#[test]
fn does_not_flag_a_private_function_called_in_the_same_script() {
    let source = "ScriptName Example\n\nFunction Helper(Int x) ; @private\n    Debug.Trace(x)\nEndFunction\n\nFunction Run()\n    Helper(1)\nEndFunction\n";

    assert!(check(source).is_empty());
}

#[test]
fn recognizes_a_self_qualified_call_case_insensitively() {
    let source = "ScriptName Example\n\nFunction Helper() ; @private\nEndFunction\n\nFunction Run()\n    self.helper()\nEndFunction\n";

    assert!(check(source).is_empty());
}

#[test]
fn does_not_treat_another_objects_call_as_a_use() {
    let source = "ScriptName Example\n\nFunction Helper() ; @private\nEndFunction\n\nFunction Run(Example other)\n    other.Helper()\nEndFunction\n";

    assert_eq!(check(source).len(), 1);
}

#[test]
fn does_not_treat_a_parent_qualified_call_as_a_use() {
    let source = "ScriptName Example Extends ParentScript\n\nFunction Helper() ; @private\nEndFunction\n\nFunction Run()\n    Parent.Helper()\nEndFunction\n";

    assert_eq!(check(source).len(), 1);
}

#[test]
fn does_not_flag_an_event() {
    let diagnostics = check("ScriptName Example\n\nEvent OnInit() ; @private\nEndEvent\n");

    assert!(diagnostics.is_empty());
}

#[test]
fn does_not_flag_a_native_function() {
    let diagnostics = check("ScriptName Example\n\nFunction Helper() Native ; @private\n");

    assert!(diagnostics.is_empty());
}

#[test]
fn does_not_flag_a_fragment_function() {
    let diagnostics =
        check("ScriptName Example\n\nFunction Fragment_0() ; @private\nEndFunction\n");

    assert!(diagnostics.is_empty());
}

#[test]
fn flags_an_unused_private_function_declared_in_a_state() {
    let diagnostics = check(
        "ScriptName Example\n\nState Active\n    Function Helper() ; @private\n    EndFunction\nEndState\n",
    );

    assert_eq!(diagnostics.len(), 1);
    assert!(diagnostics[0].message.contains("'Helper'"));
}

#[test]
fn honors_line_and_file_disables() {
    let line_disabled = crate::lint(
        "ScriptName Example\n\nFunction Helper() ; @private @disable unused-function\nEndFunction\n",
        &crate::config::Config::default(),
    );
    let file_disabled = crate::lint(
        "; @disable-file unused-function\nScriptName Example\n\nFunction Helper() ; @private\nEndFunction\n",
        &crate::config::Config::default(),
    );

    assert!(line_disabled
        .iter()
        .all(|diagnostic| diagnostic.rule != RULE));
    assert!(file_disabled
        .iter()
        .all(|diagnostic| diagnostic.rule != RULE));
}

#[test]
fn honors_the_config_off_switch() {
    let mut config = crate::config::Config::default();
    config.rules.unused_function = false;

    let diagnostics = crate::lint(
        "ScriptName Example\n\nFunction Helper() ; @private\nEndFunction\n",
        &config,
    );

    assert!(diagnostics.iter().all(|diagnostic| diagnostic.rule != RULE));
}

struct FakeParentLookup;

impl ExternalSignatures for FakeParentLookup {
    fn lookup(&mut self, type_name: &str, function_name: &str) -> Option<Vec<ParamInfo>> {
        if type_name.eq_ignore_ascii_case("ParentScript")
            && function_name.eq_ignore_ascii_case("Helper")
        {
            Some(Vec::new())
        } else {
            None
        }
    }
}

#[test]
fn does_not_flag_a_private_override_of_a_parent_function() {
    let source = "ScriptName Example Extends ParentScript\n\nFunction Helper() ; @private\nEndFunction\n\nFunction Other() ; @private\nEndFunction\n";
    let ast = papyrus_parser::parse(source).ok();
    let tokens = papyrus_parser::tokenize(source).ok();
    let diagnostics = super::check(
        source,
        ast.as_ref(),
        tokens.as_deref(),
        &crate::config::Config::default(),
        &mut FakeParentLookup,
    );

    assert_eq!(diagnostics.len(), 1);
    assert!(diagnostics[0].message.contains("'Other'"));
}

#[test]
fn returns_no_diagnostics_without_an_ast() {
    assert!(check("ScriptName Example\n\nFunction Helper() ; @private\n").is_empty());
}
