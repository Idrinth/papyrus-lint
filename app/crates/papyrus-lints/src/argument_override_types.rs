//! Flags a function or event declared on this script whose parameter count
//! or parameter types don't match the corresponding parameters of the
//! same-named function declared on the script's `Extends` chain.
//!
//! Papyrus lets a child script's declaration silently replace an inherited
//! one of the same name, with no requirement that the two signatures
//! actually agree (see [`crate::function_override`]). A caller holding a
//! value typed as the parent script still resolves the call against the
//! *parent's* declared parameter list, so an override with a different
//! parameter count or type either fails to compile against such a
//! reference or, once positional binding papers over it, silently receives
//! arguments meant for a differently-shaped signature. Keeping an
//! override's parameter count and types in sync with the inherited
//! declaration avoids that trap even though Papyrus itself doesn't require
//! it — the same reasoning as [`crate::argument_naming`], applied to
//! parameter count/type instead of parameter name.
//!
//! Like [`crate::function_override`]/[`crate::argument_naming`], this can
//! never be answered from `source` alone and reuses
//! [`crate::external_signatures::ExternalSignatures`]; without one (see
//! [`check`]), this never finds anything to flag. Only functions declared
//! directly on the script are checked, matching
//! [`crate::function_override`]'s treatment of `State`-based overrides as a
//! separate mechanism from `Extends`. Types are compared exactly
//! (case-insensitively), not with the widening/subtype leniency
//! [`crate::argument_types`] allows at call sites, since a call resolved
//! against the parent's reference still binds against the parent's exact
//! declared type regardless of what the override itself accepts.

use papyrus_parser::ast::{Script, TypeName};

use crate::argument_types::format_type;
use crate::external_signatures::ExternalSignatures;
use crate::Diagnostic;

/// This lint's [`Diagnostic::rule`] id, for `@disable` line comments.
pub const RULE: &str = "argument-override-types";

#[allow(dead_code)] // not dispatched from collect_diagnostics yet
pub fn visitor() -> crate::visitor::LintVisitor {
    crate::visitor::LintVisitor::ast()
}

/// Checks `source` for overridden functions/events whose parameter count or
/// parameter types drift from the inherited declaration. Since resolving
/// the `Extends` chain always requires looking outside `source`, this alone
/// never finds anything to flag; see [`check_with`].
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
/// `external`, comparing each function/event declared on `source` against
/// the same-named function declared somewhere along that chain (if any). A
/// parameter count mismatch is reported as a single diagnostic for the
/// whole declaration; a matching count is then compared parameter by
/// parameter for a type mismatch at the same position.
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
        let kind = if function.is_event {
            "Event"
        } else {
            "Function"
        };

        if function.params.len() != parent_params.len() {
            diagnostics.push(Diagnostic {
                line: function.line,
                column: 1,
                message: format!(
                    "[error] {kind} '{}' declares {} but the inherited declaration on '{}' declares {}",
                    function.name,
                    param_count(function.params.len()),
                    extends,
                    param_count(parent_params.len()),
                ),
                rule: RULE,
            });
            continue;
        }

        for (index, (local, parent)) in function.params.iter().zip(&parent_params).enumerate() {
            if !type_names_match(&local.type_name, &parent.type_name) {
                diagnostics.push(Diagnostic {
                    line: function.line,
                    column: 1,
                    message: format!(
                        "[error] Parameter {} of {kind} '{}' is declared {} but the inherited declaration on '{}' declares {}",
                        index + 1,
                        function.name,
                        format_type(&local.type_name),
                        extends,
                        format_type(&parent.type_name),
                    ),
                    rule: RULE,
                });
            }
        }
    }

    diagnostics
}

/// Papyrus type names are case-insensitive, so `Bool` and `bool` name the
/// same type even though they'd otherwise fail a derived `PartialEq` on
/// [`TypeName`], which compares `name` byte-for-byte.
fn type_names_match(a: &TypeName, b: &TypeName) -> bool {
    a.is_array == b.is_array && a.name.eq_ignore_ascii_case(&b.name)
}

fn param_count(count: usize) -> String {
    if count == 1 {
        "1 parameter".to_string()
    } else {
        format!("{count} parameters")
    }
}

#[cfg(test)]
#[path = "argument_override_types_tests.rs"]
mod tests;
