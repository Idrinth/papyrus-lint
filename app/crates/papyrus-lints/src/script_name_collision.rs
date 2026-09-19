//! Flags a script-level `Property` or variable (declared directly on the
//! script, outside any function) whose name matches (case-insensitively)
//! the name of the script it's declared in, since Papyrus doesn't allow a
//! declared identifier to collide with the script's own type name — such a
//! script fails to compile.
//!
//! Works from the parsed AST rather than raw tokens, since a script's own
//! declared name, properties, and variables are already tracked there. A
//! local variable declared inside a function isn't checked here — see
//! [`crate::local_variable_shadowing`] for shadowing concerns local to a
//! function body. A script that doesn't parse cleanly is left unchecked
//! rather than guessed at.

use crate::Diagnostic;

/// This lint's [`Diagnostic::rule`] id, for `@disable` line comments.
pub const RULE: &str = "script-name-collision";

pub fn visitor() -> crate::visitor::LintVisitor {
    crate::visitor::from_ast(lint_issues)
}

/// Checks `source` for a script-level `Property` or variable declaration
/// whose name matches (case-insensitively) the enclosing script's own
/// declared name. Flagged as an `[error]`, since Papyrus rejects such a
/// script at compile time.
#[allow(dead_code)] // unit tests; collect_diagnostics uses visitor()
pub fn check(
    source: &str,
    ast: Option<&papyrus_parser::ast::Script>,
    tokens: Option<&[papyrus_parser::token::Token]>,
    config: &crate::config::Config,
    external: &mut impl crate::external_signatures::ExternalSignatures,
) -> Vec<Diagnostic> {
    crate::visitor::run(visitor(), source, ast, tokens, config, external)
}

fn lint_issues(
    source: &str,
    ast: Option<&papyrus_parser::ast::Script>,
    tokens: Option<&[papyrus_parser::token::Token]>,
    config: &crate::config::Config,
    external: &mut dyn crate::external_signatures::ExternalSignatures,
) -> Vec<Diagnostic> {
    let _ = (source, tokens, config, external);

    let Some(script) = ast else {
        return Vec::new();
    };

    let mut diagnostics = Vec::new();
    for property in &script.properties {
        if property.name.eq_ignore_ascii_case(&script.name) {
            diagnostics.push(Diagnostic {
                line: property.line,
                column: 1,
                message: format!(
                    "[error] Property '{}' may not share its name with the script it's declared in ('{}')",
                    property.name, script.name
                ),
                rule: RULE,
            });
        }
    }
    for variable in &script.variables {
        if variable.name.eq_ignore_ascii_case(&script.name) {
            diagnostics.push(Diagnostic {
                line: variable.line,
                column: 1,
                message: format!(
                    "[error] Variable '{}' may not share its name with the script it's declared in ('{}')",
                    variable.name, script.name
                ),
                rule: RULE,
            });
        }
    }
    diagnostics
}

#[cfg(test)]
#[path = "script_name_collision_tests.rs"]
mod tests;
