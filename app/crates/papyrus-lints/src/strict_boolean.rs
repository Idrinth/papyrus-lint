//! Flags `If`/`ElseIf`/`While` conditions that aren't already boolean,
//! instead of relying on Papyrus's implicit conversion to `Bool`.

use papyrus_parser::ast::{Expr, FunctionDecl, IfBranch, Literal, Stmt};
use papyrus_parser::types::{infer_type, TypeEnv};

use crate::Diagnostic;

/// This lint's [`Diagnostic::rule`] id, for `@disable` line comments.
pub const RULE: &str = "strict-boolean";

/// Checks every `If`/`ElseIf`/`While` condition in `source` and flags the
/// ones that don't resolve to `Bool`.
///
/// A condition whose type can't be determined locally (a function call or a
/// member access on another script, for instance) is left unflagged rather
/// than risk a false positive. When `allow_bool_like_int` is `true` (see
/// [`crate::config::Config::bool_like_int`]), a condition that's exactly
/// the `Int` literal `1` or `0` is also left unflagged, since that's a
/// common "bool-like" idiom; any other `Int` value is still flagged.
/// Flagged as a `[warning]`.
pub fn check(
    source: &str,
    ast: Option<&papyrus_parser::ast::Script>,
    tokens: Option<&[papyrus_parser::token::Token]>,
    config: &crate::config::Config,
    external: &mut impl crate::external_signatures::ExternalSignatures,
) -> Vec<Diagnostic> {
    let _ = (source, tokens, external);
    let allow_bool_like_int = config.bool_like_int;

    let Some(script) = ast else {
        return Vec::new();
    };

    let mut env = TypeEnv::for_script(script);
    let mut diagnostics = Vec::new();

    for function in script.functions.iter().chain(
        script
            .states
            .iter()
            .flat_map(|state| state.functions.iter()),
    ) {
        check_function(function, &mut env, allow_bool_like_int, &mut diagnostics);
    }

    diagnostics
}

fn check_function(
    function: &FunctionDecl,
    env: &mut TypeEnv,
    allow_bool_like_int: bool,
    diagnostics: &mut Vec<Diagnostic>,
) {
    env.with_function_scope(function, |scoped| {
        check_body(&function.body, scoped, allow_bool_like_int, diagnostics);
    });
}

fn check_body(
    body: &[Stmt],
    env: &TypeEnv,
    allow_bool_like_int: bool,
    diagnostics: &mut Vec<Diagnostic>,
) {
    for stmt in body {
        match stmt {
            Stmt::If {
                branches,
                else_body,
                ..
            } => {
                for IfBranch {
                    condition,
                    body,
                    line,
                    col,
                } in branches
                {
                    check_condition(
                        condition,
                        *line,
                        *col,
                        env,
                        allow_bool_like_int,
                        diagnostics,
                    );
                    check_body(body, env, allow_bool_like_int, diagnostics);
                }
                check_body(else_body, env, allow_bool_like_int, diagnostics);
            }
            Stmt::While {
                condition,
                body,
                line,
                col,
            } => {
                check_condition(
                    condition,
                    *line,
                    *col,
                    env,
                    allow_bool_like_int,
                    diagnostics,
                );
                check_body(body, env, allow_bool_like_int, diagnostics);
            }
            Stmt::VarDecl(_) | Stmt::Assign { .. } | Stmt::Expr { .. } | Stmt::Return { .. } => {}
        }
    }
}

/// Whether `expr` is exactly the `Int` literal `1` or `0`, the "bool-like"
/// idiom [`check`] allows past when `allow_bool_like_int` is set.
fn is_bool_like_int(expr: &Expr) -> bool {
    matches!(expr, Expr::Literal(Literal::Int { value: 0 | 1, .. }))
}

fn check_condition(
    condition: &Expr,
    line: usize,
    column: usize,
    env: &TypeEnv,
    allow_bool_like_int: bool,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let Some(type_name) = infer_type(condition, env) else {
        return;
    };

    if !type_name.is_array && type_name.name.eq_ignore_ascii_case("bool") {
        return;
    }

    if allow_bool_like_int && is_bool_like_int(condition) {
        return;
    }

    let found = if type_name.is_array {
        format!("{}[]", type_name.name)
    } else {
        type_name.name
    };

    diagnostics.push(Diagnostic {
        line,
        column,
        message: format!(
            "[warning] Condition must be a boolean value or expression, found '{found}'"
        ),
        rule: RULE,
    });
}

#[cfg(test)]
#[path = "strict_boolean_tests.rs"]
mod tests;
