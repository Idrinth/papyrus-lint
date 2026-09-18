use super::*;
use crate::external_signatures::{ExternalSignatures, NoExternalSignatures, ParamInfo};

fn check(source: &str) -> Vec<Diagnostic> {
    check_with(source, &mut NoExternalSignatures)
}

fn check_with(source: &str, external: &mut impl ExternalSignatures) -> Vec<Diagnostic> {
    let ast = papyrus_parser::parse(source).ok();
    let tokens = papyrus_parser::tokenize(source).ok();
    super::check(
        source,
        ast.as_ref(),
        tokens.as_deref(),
        &crate::config::Config::default(),
        external,
    )
}

#[test]
fn flags_a_discarded_local_nodiscard_call() {
    let diagnostics = check(
        "ScriptName Example\n\nInt Function RegisterFoo() ; @nodiscard\n    Return 1\nEndFunction\n\nFunction Test()\n    RegisterFoo()\nEndFunction\n",
    );

    assert_eq!(diagnostics.len(), 1);
    assert_eq!((diagnostics[0].line, diagnostics[0].column), (8, 5));
    assert!(diagnostics[0].message.contains("RegisterFoo"));
    assert_eq!(diagnostics[0].rule, RULE);
}

#[test]
fn flags_a_nodiscard_comment_on_the_line_above_the_header() {
    let diagnostics = check(
        "ScriptName Example\n\n; @nodiscard\nInt Function RegisterFoo()\n    Return 1\nEndFunction\n\nFunction Test()\n    RegisterFoo()\nEndFunction\n",
    );

    assert_eq!(diagnostics.len(), 1);
    assert!(diagnostics[0].message.contains("RegisterFoo"));
}

#[test]
fn matching_is_case_insensitive() {
    let diagnostics = check(
        "ScriptName Example\n\nInt Function RegisterFoo() ; @NoDiscard\n    Return 1\nEndFunction\n\nFunction Test()\n    registerfoo()\nEndFunction\n",
    );

    assert_eq!(diagnostics.len(), 1);
}

#[test]
fn nodiscardable_is_not_treated_as_nodiscard() {
    let diagnostics = check(
        "ScriptName Example\n\nInt Function RegisterFoo() ; @nodiscardable\n    Return 1\nEndFunction\n\nFunction Test()\n    RegisterFoo()\nEndFunction\n",
    );

    assert!(diagnostics.is_empty());
}

#[test]
fn ignores_nodiscard_results_that_are_used() {
    let diagnostics = check(
        "ScriptName Example\n\nInt Function RegisterFoo() ; @nodiscard\n    Return 1\nEndFunction\n\nFunction Test()\n    Int value = RegisterFoo()\n    value = RegisterFoo()\n    Return RegisterFoo()\n    UseValue(RegisterFoo())\n    If RegisterFoo()\n    EndIf\nEndFunction\n",
    );

    assert!(diagnostics.is_empty());
}

#[test]
fn flags_a_nodiscard_call_whose_result_only_feeds_a_discarded_comparison() {
    let diagnostics = check(
        "ScriptName Example\n\nInt Function RegisterFoo() ; @nodiscard\n    Return 1\nEndFunction\n\nFunction Test()\n    RegisterFoo() > 0\nEndFunction\n",
    );

    assert_eq!(diagnostics.len(), 1);
    assert!(diagnostics[0].message.contains("RegisterFoo"));
}

#[test]
fn ignores_unmarked_non_getter_calls() {
    let diagnostics = check(
        "ScriptName Example\n\nInt Function RegisterFoo()\n    Return 1\nEndFunction\n\nFunction Test()\n    RegisterFoo()\n    DoSomething()\nEndFunction\n",
    );

    assert!(diagnostics.is_empty());
}

#[test]
fn flags_a_qualified_call_resolved_through_external_signatures() {
    let source = "ScriptName Example\n\nFunction Test()\n    Helpers.RegisterFoo()\nEndFunction\n";
    let mut external = FakeExternal {
        script: "Helpers",
        function: "RegisterFoo",
    };
    let diagnostics = check_with(source, &mut external);

    assert_eq!(diagnostics.len(), 1);
    assert!(diagnostics[0].message.contains("RegisterFoo"));
}

#[test]
fn flags_an_inherited_nodiscard_call_through_the_current_script() {
    let source = "ScriptName Child\n\nFunction Test()\n    RegisterFoo()\nEndFunction\n";
    let mut external = FakeExternal {
        script: "Child",
        function: "RegisterFoo",
    };
    let diagnostics = check_with(source, &mut external);

    assert_eq!(diagnostics.len(), 1);
}

#[test]
fn flags_a_call_through_a_typed_property() {
    let source = "ScriptName Example\n\nHelpers Property Helper Auto\n\nFunction Test()\n    Helper.RegisterFoo()\nEndFunction\n";
    let mut external = FakeExternal {
        script: "Helpers",
        function: "RegisterFoo",
    };
    let diagnostics = check_with(source, &mut external);

    assert_eq!(diagnostics.len(), 1);
}

#[test]
fn ignores_a_call_through_an_unrelated_external_script() {
    let source = "ScriptName Example\n\nFunction Test()\n    Other.RegisterFoo()\nEndFunction\n";
    let mut external = FakeExternal {
        script: "Helpers",
        function: "RegisterFoo",
    };
    let diagnostics = check_with(source, &mut external);

    assert!(diagnostics.is_empty());
}

#[test]
fn honors_a_disable_comment_on_the_discarded_call() {
    let diagnostics = crate::lint(
        "ScriptName Example\n\nInt Function RegisterFoo() ; @nodiscard\n    Return 1\nEndFunction\n\nFunction Test()\n    RegisterFoo() ; @disable unused-nodiscard\nEndFunction\n",
        &crate::config::Config::default(),
    );

    assert!(diagnostics
        .iter()
        .all(|diagnostic| diagnostic.rule != RULE));
}

#[test]
fn supports_multiline_calls() {
    let diagnostics = check(
        "ScriptName Example\n\nInt Function RegisterFoo(Int a) ; @nodiscard\n    Return a\nEndFunction\n\nFunction Test()\n    RegisterFoo(\\\n        1)\nEndFunction\n",
    );

    assert_eq!(diagnostics.len(), 1);
    assert_eq!(diagnostics[0].line, 8);
}

struct FakeExternal {
    script: &'static str,
    function: &'static str,
}

impl ExternalSignatures for FakeExternal {
    fn lookup(&mut self, _type_name: &str, _function_name: &str) -> Option<Vec<ParamInfo>> {
        None
    }

    fn is_nodiscard_function(&mut self, type_name: &str, function_name: &str) -> Option<bool> {
        Some(
            type_name.eq_ignore_ascii_case(self.script)
                && function_name.eq_ignore_ascii_case(self.function),
        )
    }
}
