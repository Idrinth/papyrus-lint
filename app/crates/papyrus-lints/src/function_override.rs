//! Flags a function declared on this script that shares its name with a
//! function declared on the script's `Extends` chain, since the local
//! declaration silently replaces the inherited one whenever this script's
//! type is used. This is often intentional (e.g. overriding an `Event
//! OnInit()` handler is the normal way to specialize a parent script's
//! behavior), so it's flagged as an `[info]` rather than a `[warning]`: a
//! useful thing to be aware of, not a likely mistake.
//!
//! Unlike the other lints in this crate, this can never be answered from
//! `source` alone — the parent script's declared functions live in a
//! different file. Like [`crate::argument_types`] and
//! [`crate::return_types`], it reuses
//! [`crate::argument_types::ExternalSignatures`] so a caller that can
//! resolve other scripts (e.g. the desktop app's `FunctionTable`) supplies
//! that; without one (see [`check`]), this never finds anything to flag.
//!
//! Only functions declared directly on the script are checked, not ones
//! declared inside a `State` block — overriding a base state's function
//! from a named state is Papyrus's separate state-based override
//! mechanism, not `Extends` inheritance.

use crate::argument_types::{ExternalSignatures, NoExternalSignatures};
use crate::Diagnostic;

/// This lint's [`Diagnostic::rule`] id, for `@disable` line comments.
pub const RULE: &str = "function-override";

/// Checks `source` for functions that override an inherited one. Since
/// resolving the `Extends` chain always requires looking outside `source`,
/// this alone never finds anything to flag; see [`check_with`].
pub fn check(source: &str) -> Vec<Diagnostic> {
    check_with(source, &mut NoExternalSignatures)
}

/// Like [`check`], but resolves the script's `Extends` chain through
/// `external`, flagging any function declared on `source` whose name is
/// also declared somewhere along that chain.
pub fn check_with<E: ExternalSignatures>(source: &str, external: &mut E) -> Vec<Diagnostic> {
    let Ok(script) = papyrus_parser::parse(source) else {
        return Vec::new();
    };
    let Some(extends) = &script.extends else {
        return Vec::new();
    };

    script
        .functions
        .iter()
        .filter(|function| external.lookup(extends, &function.name).is_some())
        .map(|function| Diagnostic {
            line: function.line,
            column: 1,
            message: format!(
                "[info] Function '{}' overrides an inherited function declared on '{}' or one of its ancestors",
                function.name, extends
            ),
            rule: RULE,
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::argument_types::ParamInfo;

    struct FakeExternal;

    impl ExternalSignatures for FakeExternal {
        fn lookup(&mut self, type_name: &str, function_name: &str) -> Option<Vec<ParamInfo>> {
            if type_name.eq_ignore_ascii_case("ParentScript")
                && function_name.eq_ignore_ascii_case("DoThing")
            {
                Some(Vec::new())
            } else {
                None
            }
        }
    }

    #[test]
    fn without_external_never_flags_anything() {
        let diagnostics =
            check("ScriptName Example Extends ParentScript\n\nFunction DoThing()\nEndFunction\n");

        assert!(diagnostics.is_empty());
    }

    #[test]
    fn flags_a_function_that_overrides_an_inherited_one() {
        let diagnostics = check_with(
            "ScriptName Example Extends ParentScript\n\nFunction DoThing()\nEndFunction\n",
            &mut FakeExternal,
        );

        assert_eq!(diagnostics.len(), 1);
        assert_eq!(diagnostics[0].line, 3);
        assert!(diagnostics[0].message.starts_with("[info]"));
        assert!(diagnostics[0].message.contains("'DoThing'"));
        assert!(diagnostics[0].message.contains("'ParentScript'"));
    }

    #[test]
    fn does_not_flag_a_function_with_no_matching_inherited_name() {
        let diagnostics = check_with(
            "ScriptName Example Extends ParentScript\n\nFunction SomethingElse()\nEndFunction\n",
            &mut FakeExternal,
        );

        assert!(diagnostics.is_empty());
    }

    #[test]
    fn does_not_flag_anything_on_a_script_without_extends() {
        let diagnostics = check_with(
            "ScriptName Example\n\nFunction DoThing()\nEndFunction\n",
            &mut FakeExternal,
        );

        assert!(diagnostics.is_empty());
    }

    #[test]
    fn does_not_flag_a_function_declared_only_inside_a_state() {
        let diagnostics = check_with(
            "ScriptName Example Extends ParentScript\n\nState Loud\n    Function DoThing()\n    EndFunction\nEndState\n",
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

    #[test]
    fn inherited_function_lookup_is_case_insensitive() {
        let diagnostics = check_with(
            "ScriptName Example Extends parentscript\n\nFunction dOtHiNg()\nEndFunction\n",
            &mut FakeExternal,
        );

        assert_eq!(diagnostics.len(), 1);
        assert_eq!(diagnostics[0].rule, RULE);
        assert!(diagnostics[0].message.contains("'dOtHiNg'"));
    }

    #[test]
    fn flags_each_matching_top_level_declaration() {
        let diagnostics = check_with(
            "ScriptName Example Extends ParentScript\n\nFunction DoThing()\nEndFunction\n\nEvent DoThing()\nEndEvent\n",
            &mut FakeExternal,
        );

        let locations: Vec<_> = diagnostics
            .iter()
            .map(|diagnostic| (diagnostic.line, diagnostic.column))
            .collect();
        assert_eq!(locations, vec![(3, 1), (6, 1)]);
    }

    #[test]
    fn top_level_override_is_still_flagged_when_a_state_also_overrides_it() {
        let diagnostics = check_with(
            "ScriptName Example Extends ParentScript\n\nFunction DoThing()\nEndFunction\n\nState Loud\n    Function DoThing()\n    EndFunction\nEndState\n",
            &mut FakeExternal,
        );

        assert_eq!(diagnostics.len(), 1);
        assert_eq!(diagnostics[0].line, 3);
    }
}
