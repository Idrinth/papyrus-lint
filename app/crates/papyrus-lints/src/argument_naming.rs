//! Flags a function declared on this script whose parameter names don't
//! match (case-insensitively) the corresponding parameter names of the
//! same-named function declared on the script's `Extends` chain.
//!
//! Papyrus resolves a named-argument call (`func(argB = 1)`) against the
//! declared type of the reference it's called through, not the runtime
//! type of the object behind it. So a caller holding a value typed as the
//! parent script, calling an overridden function by parameter name, binds
//! those names against the *parent's* declaration — a child override that
//! renamed a parameter silently receives the argument meant for a
//! differently-named one (or the call fails to compile at all against a
//! parent-typed reference). Keeping overridden parameter names in sync
//! avoids that trap even though Papyrus itself doesn't require it.
//!
//! Like [`crate::function_override`], this can never be answered from
//! `source` alone and reuses
//! [`crate::argument_types::ExternalSignatures`]; without one (see
//! [`check`]), this never finds anything to flag. Only functions declared
//! directly on the script are checked, matching
//! [`crate::function_override`]'s treatment of `State`-based overrides as
//! a separate mechanism from `Extends`.

use crate::argument_types::{ExternalSignatures, NoExternalSignatures};
use crate::Diagnostic;

/// This lint's [`Diagnostic::rule`] id, for `@disable` line comments.
pub const RULE: &str = "argument-naming";

/// Checks `source` for overridden functions whose parameter names drift
/// from the inherited declaration. Since resolving the `Extends` chain
/// always requires looking outside `source`, this alone never finds
/// anything to flag; see [`check_with`].
pub fn check(source: &str) -> Vec<Diagnostic> {
    check_with(source, &mut NoExternalSignatures)
}

/// Like [`check`], but resolves the script's `Extends` chain through
/// `external`, comparing each function declared on `source` against the
/// same-named function declared somewhere along that chain (if any), and
/// flagging parameter names that differ case-insensitively at the same
/// position. A parameter beyond the shorter of the two declarations' count
/// (a signature that doesn't even match in length) isn't compared.
pub fn check_with<E: ExternalSignatures>(source: &str, external: &mut E) -> Vec<Diagnostic> {
    let Ok(script) = papyrus_parser::parse(source) else {
        return Vec::new();
    };
    let Some(extends) = &script.extends else {
        return Vec::new();
    };

    let mut diagnostics = Vec::new();
    for function in &script.functions {
        let Some(parent_params) = external.lookup(extends, &function.name) else {
            continue;
        };

        for (index, (local, parent)) in function.params.iter().zip(&parent_params).enumerate() {
            if !local.name.eq_ignore_ascii_case(&parent.name) {
                diagnostics.push(Diagnostic {
                    line: function.line,
                    column: 1,
                    message: format!(
                        "[warning] Parameter {} of '{}' is named '{}' but the inherited declaration on '{}' names it '{}'",
                        index + 1,
                        function.name,
                        local.name,
                        extends,
                        parent.name
                    ),
                    rule: RULE,
                });
            }
        }
    }

    diagnostics
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::argument_types::ParamInfo;
    use papyrus_parser::ast::TypeName;

    struct FakeExternal;

    impl ExternalSignatures for FakeExternal {
        fn lookup(&mut self, type_name: &str, function_name: &str) -> Option<Vec<ParamInfo>> {
            if type_name.eq_ignore_ascii_case("ParentScript")
                && function_name.eq_ignore_ascii_case("DoThing")
            {
                Some(vec![
                    ParamInfo {
                        name: "akTarget".to_string(),
                        type_name: TypeName {
                            name: "ObjectReference".to_string(),
                            is_array: false,
                        },
                    },
                    ParamInfo {
                        name: "aiCount".to_string(),
                        type_name: TypeName {
                            name: "Int".to_string(),
                            is_array: false,
                        },
                    },
                ])
            } else {
                None
            }
        }
    }

    #[test]
    fn without_external_never_flags_anything() {
        let diagnostics = check(
            "ScriptName Example Extends ParentScript\n\nFunction DoThing(ObjectReference akRef, Int aiCount)\nEndFunction\n",
        );

        assert!(diagnostics.is_empty());
    }

    #[test]
    fn flags_a_renamed_parameter_on_an_override() {
        let diagnostics = check_with(
            "ScriptName Example Extends ParentScript\n\nFunction DoThing(ObjectReference akRef, Int aiCount)\nEndFunction\n",
            &mut FakeExternal,
        );

        assert_eq!(diagnostics.len(), 1);
        assert_eq!(diagnostics[0].line, 3);
        assert!(diagnostics[0].message.starts_with("[warning]"));
        assert!(diagnostics[0].message.contains("Parameter 1 of 'DoThing'"));
        assert!(diagnostics[0].message.contains("named 'akRef'"));
        assert!(diagnostics[0]
            .message
            .contains("names it 'akTarget'"));
    }

    #[test]
    fn does_not_flag_parameter_names_that_only_differ_in_case() {
        let diagnostics = check_with(
            "ScriptName Example Extends ParentScript\n\nFunction DoThing(ObjectReference AKTARGET, Int aiCount)\nEndFunction\n",
            &mut FakeExternal,
        );

        assert!(diagnostics.is_empty());
    }

    #[test]
    fn flags_each_mismatched_parameter_separately() {
        let diagnostics = check_with(
            "ScriptName Example Extends ParentScript\n\nFunction DoThing(ObjectReference akRef, Int aiTotal)\nEndFunction\n",
            &mut FakeExternal,
        );

        assert_eq!(diagnostics.len(), 2);
        assert!(diagnostics[0].message.contains("Parameter 1"));
        assert!(diagnostics[1].message.contains("Parameter 2"));
    }

    #[test]
    fn does_not_flag_a_function_with_no_matching_inherited_name() {
        let diagnostics = check_with(
            "ScriptName Example Extends ParentScript\n\nFunction SomethingElse(Int aiCount)\nEndFunction\n",
            &mut FakeExternal,
        );

        assert!(diagnostics.is_empty());
    }

    #[test]
    fn does_not_flag_anything_on_a_script_without_extends() {
        let diagnostics = check_with(
            "ScriptName Example\n\nFunction DoThing(ObjectReference akRef, Int aiCount)\nEndFunction\n",
            &mut FakeExternal,
        );

        assert!(diagnostics.is_empty());
    }

    #[test]
    fn does_not_flag_a_function_declared_only_inside_a_state() {
        let diagnostics = check_with(
            "ScriptName Example Extends ParentScript\n\nState Loud\n    Function DoThing(ObjectReference akRef, Int aiCount)\n    EndFunction\nEndState\n",
            &mut FakeExternal,
        );

        assert!(diagnostics.is_empty());
    }

    #[test]
    fn does_not_compare_parameters_beyond_the_shorter_declarations_count() {
        let diagnostics = check_with(
            "ScriptName Example Extends ParentScript\n\nFunction DoThing(ObjectReference akTarget)\nEndFunction\n",
            &mut FakeExternal,
        );

        assert!(diagnostics.is_empty());
    }

    #[test]
    fn does_not_crash_on_unparseable_source() {
        let diagnostics = check_with(
            "ScriptName Example Extends ParentScript\n\nFunction DoThing(\nEndFunction\n",
            &mut FakeExternal,
        );

        assert!(diagnostics.is_empty());
    }
}
