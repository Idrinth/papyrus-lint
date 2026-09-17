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
#[allow(dead_code)]
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
        .map(|function| {
            let kind = if function.is_event { "Event" } else { "Function" };
            Diagnostic {
                line: function.line,
                column: 1,
                message: format!(
                    "[info] {kind} '{}' overrides an inherited {} declared on '{}' or one of its ancestors",
                    function.name,
                    kind.to_ascii_lowercase(),
                    extends
                ),
                rule: RULE,
            }
        })
        .collect()
}

#[cfg(test)]
#[path = "function_override_tests.rs"]
mod tests;
