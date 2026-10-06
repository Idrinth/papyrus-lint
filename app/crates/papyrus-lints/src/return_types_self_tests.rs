//! `Return self` for a return type naming the script or one of its
//! `Extends` ancestors. See [`crate::argument_types::SelfScript`].

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
fn accepts_returning_self_for_the_parent_type_without_a_resolver() {
    let diagnostics = check(
        r#"
ScriptName ChildScript Extends ParentScript

ParentScript Function Test()
    Return self
EndFunction
"#,
    );

    assert!(diagnostics.is_empty(), "{diagnostics:?}");
}

#[test]
fn still_flags_returning_self_for_a_primitive_return_type() {
    let diagnostics = check(
        r#"
ScriptName ChildScript Extends ParentScript

Int Function Test()
    Return self
EndFunction
"#,
    );

    assert_eq!(diagnostics.len(), 1);
    assert!(diagnostics[0].message.contains("returns ChildScript"));
}
