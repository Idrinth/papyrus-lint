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
//! [`crate::external_signatures::ExternalSignatures`]; without one (see
//! [`check`]), this never finds anything to flag. Only functions declared
//! directly on the script are checked, matching
//! [`crate::function_override`]'s treatment of `State`-based overrides as
//! a separate mechanism from `Extends`.

use papyrus_parser::ast::Script;

use crate::external_signatures::ExternalSignatures;
use crate::Diagnostic;

/// This lint's [`Diagnostic::rule`] id, for `@disable` line comments.
pub const RULE: &str = "argument-naming";

#[allow(dead_code)] // not dispatched from collect_diagnostics yet
pub fn visitor() -> crate::visitor::LintVisitor {
    crate::visitor::LintVisitor::ast()
}

/// Checks `source` for overridden functions whose parameter names drift
/// from the inherited declaration. Since resolving the `Extends` chain
/// always requires looking outside `source`, this alone never finds
/// anything to flag; see [`check_with`].
pub fn check(
    source: &str,
    ast: Option<&papyrus_parser::ast::Script>,
    tokens: Option<&[papyrus_parser::token::Token]>,
    config: &crate::config::Config,
    external: &mut impl crate::external_signatures::ExternalSignatures,
) -> Vec<Diagnostic> {
    let _ = (source, tokens, config);
    check_with(ast, external)
}

/// Like [`check`], but resolves the script's `Extends` chain through
/// `external`, comparing each function declared on `source` against the
/// same-named function declared somewhere along that chain (if any), and
/// flagging parameter names that differ case-insensitively at the same
/// position. A parameter beyond the shorter of the two declarations' count
/// (a signature that doesn't even match in length) isn't compared.
pub fn check_with<E: ExternalSignatures>(
    ast: Option<&Script>,
    external: &mut E,
) -> Vec<Diagnostic> {
    let Some(script) = ast else {
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
#[path = "argument_naming_tests.rs"]
mod tests;
