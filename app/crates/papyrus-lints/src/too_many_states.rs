//! Dispatch wrapper around [`crate::state_count`]'s named-state-count check.

use crate::state_count;
use crate::Diagnostic;

/// This lint's [`Diagnostic::rule`] id, for `@disable` line comments.
#[allow(dead_code)]
pub const RULE: &str = state_count::TOO_MANY_STATES_RULE;

#[allow(dead_code)] // not dispatched from collect_diagnostics yet
pub fn visitor() -> crate::visitor::LintVisitor {
    crate::visitor::LintVisitor::ast()
}

/// See [`state_count::check_too_many_states_with`].
pub fn check(
    source: &str,
    ast: Option<&papyrus_parser::ast::Script>,
    tokens: Option<&[papyrus_parser::token::Token]>,
    config: &crate::config::Config,
    external: &mut impl crate::external_signatures::ExternalSignatures,
) -> Vec<Diagnostic> {
    let _ = (source, tokens, config);
    state_count::check_too_many_states_with(ast, external)
}
