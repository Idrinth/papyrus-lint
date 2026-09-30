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

/// Adds `; @override` on headers [`check`] would flag — which, without a
/// resolver, is none at all. See [`repair_with`].
#[allow(dead_code)]
pub fn repair(
    source: &str,
    ast: Option<&papyrus_parser::ast::Script>,
    tokens: Option<&[papyrus_parser::token::Token]>,
    config: &crate::config::Config,
) -> String {
    let _ = (ast, tokens, config);
    repair_with(source, &mut crate::external_signatures::NoExternalSignatures)
}

/// Appends `; @override` to every top-level override header [`check_with`]
/// flags through `external`. An existing trailing comment is extended with
/// `@override` rather than a second `;`. Scripts that don't parse, or that
/// have nothing to flag, are returned unchanged.
pub fn repair_with<E: ExternalSignatures + ?Sized>(source: &str, external: &mut E) -> String {
    let ast = papyrus_parser::parse(source).ok();
    let lines_to_mark: std::collections::HashSet<usize> = check_with(source, ast.as_ref(), external)
        .into_iter()
        .map(|diagnostic| diagnostic.line)
        .collect();
    if lines_to_mark.is_empty() {
        return source.to_string();
    }

    let mut result = String::with_capacity(source.len() + lines_to_mark.len() * 12);
    let mut rest = source;
    let mut line_number = 1usize;
    while !rest.is_empty() {
        let (line_and_ending, remainder) = match rest.find('\n') {
            Some(index) => (&rest[..=index], &rest[index + 1..]),
            None => (rest, ""),
        };
        if lines_to_mark.contains(&line_number) {
            let (line, ending) = match line_and_ending.strip_suffix('\n') {
                Some(line) => (line, "\n"),
                None => (line_and_ending, ""),
            };
            result.push_str(&add_override_to_line(line));
            result.push_str(ending);
        } else {
            result.push_str(line_and_ending);
        }
        rest = remainder;
        line_number += 1;
    }
    result
}

fn add_override_to_line(line: &str) -> String {
    let (content, trailing_cr) = match line.strip_suffix('\r') {
        Some(stripped) => (stripped, "\r"),
        None => (line, ""),
    };
    if line_has_override(content) {
        return line.to_string();
    }
    let separator = if crate::unused_nodiscard::line_comment_text(content).is_some() {
        " "
    } else {
        " ; "
    };
    format!("{content}{separator}@override{trailing_cr}")
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
