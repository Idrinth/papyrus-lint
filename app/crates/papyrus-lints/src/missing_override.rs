//! Flags a top-level function or event that overrides an inherited member
//! without a `; @override` annotation on the header or the line above it.
//!
//! Papyrus has no language-level `override` keyword. The existing
//! [`crate::function_override`] rule only *informs* that a local name
//! collides with an ancestor; this rule requires the explicit marker so
//! accidental collisions are harder to miss. The annotation style matches
//! `@nodiscard` / `@deprecated` / `@private`.
//!
//! Like [`crate::function_override`], this cannot be answered from
//! `source` alone — parent declarations live in other files — so it reuses
//! [`crate::external_signatures::ExternalSignatures`]. Without a resolver
//! (see [`check`]), this never finds anything to flag.
//!
//! Only functions declared directly on the script are checked, not ones
//! declared inside a `State` block — overriding a base state's function
//! from a named state is Papyrus's separate state-based override
//! mechanism, not `Extends` inheritance. Disabled by default; a project
//! opts in via `rules.missing_override`.

use papyrus_parser::ast::{FunctionDecl, Script};
use papyrus_parser::comment_annotations::parse_line_annotations;

use crate::external_signatures::ExternalSignatures;
use crate::visitor::{AstLint, LintVisitor, Store, VisitCtx};
use crate::Diagnostic;

/// This lint's [`Diagnostic::rule`] id, for `@disable` line comments.
pub const RULE: &str = "missing-override";

#[derive(Default)]
struct Collect {
    store: Store,
    extends: Option<String>,
    lines: Vec<String>,
}

impl AstLint for Collect {
    fn store(&mut self) -> &mut Store {
        &mut self.store
    }

    fn begin(&mut self, ctx: &mut VisitCtx<'_>) {
        self.lines = ctx.source.split('\n').map(str::to_string).collect();
    }

    fn visit_script(&mut self, script: &Script, _ctx: &mut VisitCtx<'_>) {
        self.extends = script.extends.clone();
    }

    fn visit_function(&mut self, function: &FunctionDecl, ctx: &mut VisitCtx<'_>) {
        if function.state.is_some() {
            return;
        }
        let Some(extends) = &self.extends else {
            return;
        };
        if ctx.external.lookup(extends, &function.name).is_none() {
            return;
        }
        if header_has_override(&self.lines, function.line) {
            return;
        }
        let kind = if function.is_event { "Event" } else { "Function" };
        self.store.emit(
            function.line,
            1,
            format!(
                "[info] {kind} '{}' overrides an inherited {} on '{}' or one of its ancestors without a `; @override` annotation",
                function.name,
                kind.to_ascii_lowercase(),
                extends
            ),
            RULE,
        );
    }
}

pub fn visitor() -> LintVisitor {
    LintVisitor::Ast(Box::new(Collect::default()))
}

/// Checks `source` for inherited overrides missing `; @override`. Since
/// resolving the `Extends` chain always requires looking outside `source`,
/// this alone never finds anything to flag; see [`check_with`].
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

/// Like [`check`], but resolves the script's `Extends` chain through
/// `external`.
#[allow(dead_code)] // unit tests; collect_diagnostics uses visitor()
pub fn check_with<E: ExternalSignatures + ?Sized>(
    source: &str,
    ast: Option<&Script>,
    external: &mut E,
) -> Vec<Diagnostic> {
    let Some(script) = ast else {
        return Vec::new();
    };
    let Some(extends) = &script.extends else {
        return Vec::new();
    };
    let lines: Vec<&str> = source.split('\n').collect();

    script
        .functions
        .iter()
        .filter(|function| function.state.is_none())
        .filter(|function| external.lookup(extends, &function.name).is_some())
        .filter(|function| !header_has_override_refs(&lines, function.line))
        .map(|function| {
            let kind = if function.is_event { "Event" } else { "Function" };
            Diagnostic {
                line: function.line,
                column: 1,
                message: format!(
                    "[info] {kind} '{}' overrides an inherited {} on '{}' or one of its ancestors without a `; @override` annotation",
                    function.name,
                    kind.to_ascii_lowercase(),
                    extends
                ),
                rule: RULE,
            }
        })
        .collect()
}

fn header_has_override(lines: &[String], line: usize) -> bool {
    header_has_override_refs(
        &lines.iter().map(String::as_str).collect::<Vec<_>>(),
        line,
    )
}

fn header_has_override_refs(lines: &[&str], line: usize) -> bool {
    if line == 0 {
        return false;
    }
    let index = line.saturating_sub(1);
    line_has_override(lines.get(index).copied().unwrap_or(""))
        || index
            .checked_sub(1)
            .and_then(|prev| lines.get(prev).copied())
            .is_some_and(line_has_override)
}

fn line_has_override(line: &str) -> bool {
    parse_line_annotations(line)
        .iter()
        .any(|annotation| annotation.name.eq_ignore_ascii_case("override"))
}

#[cfg(test)]
#[path = "missing_override_tests.rs"]
mod tests;
