//! Flags private / internal functions that nothing in the script calls.
//!
//! `unnecessary-function` only looks at body size. This rule looks at
//! call-site count: a function marked `; @private` / `; @internal`, or
//! given Starfield's `Private`/`Internal` flag, that is never invoked from
//! the same script is dead code. Public and protected functions are left
//! alone — they may be the project's API, called from another script or
//! from a mod outside the linted set.
//!
//! `Event`s, `Native` functions, and CreationKit `Fragment_<digits>`
//! entry points are exempt, matching `unnecessary-function`. A function
//! that overrides a parent script's identically-named function is exempt
//! too when that parent can be resolved, because the parent (or the
//! engine dispatching through it) is the call site. Cross-script calls
//! inside the project are not counted yet; same-script resolution is
//! enough to catch the common unused-helper case without forcing public
//! APIs to look used.
//!
//! This works from the parsed AST, so a script that doesn't parse cleanly
//! is left unchecked rather than guessed at.

use std::collections::HashSet;

use papyrus_parser::ast::{AccessLevel, Expr, FunctionDecl, Script};
use papyrus_parser::comment_annotations::parse_line_annotations;

use crate::external_signatures::ExternalSignatures;
use crate::visitor::{AstLint, LintVisitor, Store, VisitCtx};
use crate::Diagnostic;

/// This lint's [`Diagnostic::rule`] id, for `@disable` line comments.
pub const RULE: &str = "unused-function";

struct RecordedFunction {
    name: String,
    line: usize,
}

#[derive(Default)]
struct Collect {
    store: Store,
    lines: Vec<String>,
    functions: Vec<RecordedFunction>,
    called: HashSet<String>,
}

impl AstLint for Collect {
    fn store(&mut self) -> &mut Store {
        &mut self.store
    }

    fn begin(&mut self, ctx: &mut VisitCtx<'_>) {
        self.lines = ctx.source.split('\n').map(str::to_string).collect();
    }

    fn visit_function(&mut self, function: &FunctionDecl, ctx: &mut VisitCtx<'_>) {
        if !is_candidate(function, &self.lines) {
            return;
        }
        if overrides_parent(function, ctx.ast, ctx.external) {
            return;
        }
        self.functions.push(RecordedFunction {
            name: function.name.clone(),
            line: function.line,
        });
    }

    fn visit_expr(&mut self, expr: &Expr, _ctx: &mut VisitCtx<'_>) {
        let Expr::Call { callee, .. } = expr else {
            return;
        };
        if let Some(name) = local_call_name(callee) {
            self.called.insert(name.to_ascii_lowercase());
        }
    }

    fn finish(&mut self, _ctx: &mut VisitCtx<'_>) {
        for function in &self.functions {
            if self.called.contains(&function.name.to_ascii_lowercase()) {
                continue;
            }
            self.store.emit(
                function.line,
                1,
                format!(
                    "[info] Function '{}' is private/internal but never called",
                    function.name
                ),
                RULE,
            );
        }
    }
}

pub fn visitor() -> LintVisitor {
    LintVisitor::Ast(Box::new(Collect::default()))
}

/// Checks `source` for private/internal `Function`s that nothing in the
/// same script calls. Flagged as an `[info]`.
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

fn is_candidate(function: &FunctionDecl, lines: &[String]) -> bool {
    if function.is_event || function.is_native || is_fragment_function(&function.name) {
        return false;
    }
    function.access_level == AccessLevel::Private || has_internal_annotation(function.line, lines)
}

/// Whether `name` is a CreationKit-generated fragment function name
/// (`Fragment_` followed by one or more ASCII digits, case-insensitively),
/// which the engine calls directly rather than the script's own author.
fn is_fragment_function(name: &str) -> bool {
    name.get(..9)
        .is_some_and(|prefix| prefix.eq_ignore_ascii_case("Fragment_"))
        && !name[9..].is_empty()
        && name[9..].bytes().all(|byte| byte.is_ascii_digit())
}

fn has_internal_annotation(function_line: usize, lines: &[String]) -> bool {
    if function_line == 0 {
        return false;
    }
    let header_index = function_line - 1;
    if lines
        .get(header_index)
        .is_some_and(|line| line_marks_internal(line))
    {
        return true;
    }
    let mut index = header_index;
    while index > 0 {
        index -= 1;
        let Some(line) = lines.get(index) else {
            break;
        };
        if line.trim().is_empty() {
            continue;
        }
        if !is_line_comment(line) {
            break;
        }
        if line_marks_internal(line) {
            return true;
        }
    }
    false
}

fn is_line_comment(line: &str) -> bool {
    let trimmed = line.trim_start();
    trimmed.starts_with(';') && !trimmed.starts_with(";/")
}

fn line_marks_internal(line: &str) -> bool {
    parse_line_annotations(line).iter().any(|annotation| {
        matches!(
            annotation.name.to_ascii_lowercase().as_str(),
            "private" | "internal"
        )
    })
}

fn local_call_name(callee: &Expr) -> Option<&str> {
    match callee {
        Expr::Identifier(name) => Some(name),
        Expr::Member { object, property } if matches!(object.as_ref(), Expr::Self_) => {
            Some(property)
        }
        _ => None,
    }
}

fn overrides_parent(
    function: &FunctionDecl,
    script: Option<&Script>,
    external: &mut dyn ExternalSignatures,
) -> bool {
    let Some(parent) = script.and_then(|script| script.extends.as_deref()) else {
        return false;
    };
    external.lookup(parent, &function.name).is_some()
}

#[cfg(test)]
#[path = "unused_function_tests.rs"]
mod tests;
