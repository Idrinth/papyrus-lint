//! Black-box tests for the crate-level lint and repair entry points.

use papyrus_lints::{
    argument_types::{ExternalSignatures, ParamInfo},
    lint, lint_with_external_arguments, repair, Config, Diagnostic,
};

#[test]
fn lint_reports_multiple_enabled_rules_through_the_public_api() {
    let source = "ScriptName Example  \n\nFunction Run(Int left,Int right)\nEndFunction\n";

    let diagnostics = lint(source, &Config::default());

    assert!(diagnostics
        .iter()
        .any(|diagnostic| diagnostic.rule == "trailing-whitespace"));
    assert!(diagnostics
        .iter()
        .any(|diagnostic| diagnostic.rule == "comma-spacing"));
}

#[test]
fn repair_is_idempotent_and_clears_fixable_diagnostics() {
    let source = "ScriptName Example  \r\n\r\nFunction Run(Int left,Int right)\r\nEndFunction\r\n";
    let config = Config::default();

    let repaired = repair(source, &config);

    assert_eq!(
        repaired,
        "ScriptName Example\r\n\r\nFunction Run(Int Left, Int Right)\r\nEndFunction\r\n"
    );
    assert_eq!(repair(&repaired, &config), repaired);
    assert!(lint(&repaired, &config).iter().all(|diagnostic| {
        !matches!(
            diagnostic.rule,
            "trailing-whitespace" | "comma-spacing" | "identifier-casing"
        )
    }));
}

#[test]
fn diagnostic_serialization_exposes_the_structured_output_contract() {
    let diagnostic = Diagnostic {
        line: 7,
        column: 11,
        message: "[warning] Example finding".to_string(),
        rule: "example-rule",
    };

    assert_eq!(
        serde_json::to_value(diagnostic).unwrap(),
        serde_json::json!({
            "line": 7,
            "column": 11,
            "message": "[warning] Example finding",
            "rule": "example-rule",
        })
    );
}

struct MissingScriptResolver;

impl ExternalSignatures for MissingScriptResolver {
    fn lookup(&mut self, _type_name: &str, _function_name: &str) -> Option<Vec<ParamInfo>> {
        None
    }

    fn script_exists(&mut self, type_name: &str) -> bool {
        type_name.eq_ignore_ascii_case("KnownScript")
    }
}

#[test]
fn external_resolver_is_used_by_the_public_lint_entry_point() {
    let source = "ScriptName Example\n\nFunction Run()\n    MissingScript.DoThing()\n    KnownScript.DoThing()\nEndFunction\n";

    let diagnostics =
        lint_with_external_arguments(source, &Config::default(), &mut MissingScriptResolver);
    let unresolved: Vec<_> = diagnostics
        .iter()
        .filter(|diagnostic| diagnostic.rule == "unresolved-script")
        .collect();

    assert_eq!(unresolved.len(), 1);
    assert_eq!((unresolved[0].line, unresolved[0].column), (4, 26));
    assert!(unresolved[0].message.contains("MissingScript"));
}
