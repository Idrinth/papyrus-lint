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

use std::collections::{HashMap, HashSet};

use papyrus_parser::ast::{FunctionDecl, Script, Stmt};
use papyrus_parser::token::{Keyword, Token, TokenKind};

use crate::external_signatures::{ExternalSignatures, NoExternalSignatures, ParamInfo};
use crate::token_walk::line_starts;
use crate::visitor::{AstLint, LintVisitor, Store, VisitCtx};
use crate::Diagnostic;

/// This lint's [`Diagnostic::rule`] id, for `@disable` line comments.
pub const RULE: &str = "argument-naming";

#[derive(Default)]
struct Collect {
    store: Store,
    extends: Option<String>,
}

impl AstLint for Collect {
    fn store(&mut self) -> &mut Store {
        &mut self.store
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
        let Some(parent_params) = ctx.external.lookup(extends, &function.name) else {
            return;
        };

        for (index, (local, parent)) in function.params.iter().zip(&parent_params).enumerate() {
            if local.name.eq_ignore_ascii_case(&parent.name) {
                continue;
            }
            self.store.emit(
                function.line,
                1,
                format!(
                    "[warning] Parameter {} of '{}' is named '{}' but the inherited declaration on '{}' names it '{}'",
                    index + 1,
                    function.name,
                    local.name,
                    extends,
                    parent.name
                ),
                RULE,
            );
        }
    }
}

pub fn visitor() -> LintVisitor {
    LintVisitor::Ast(Box::new(Collect::default()))
}

/// Checks `source` for overridden functions whose parameter names drift
/// from the inherited declaration. Since resolving the `Extends` chain
/// always requires looking outside `source`, this alone never finds
/// anything to flag; see [`check_with`].
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
/// `external`, comparing each function declared on `source` against the
/// same-named function declared somewhere along that chain (if any), and
/// flagging parameter names that differ case-insensitively at the same
/// position. A parameter beyond the shorter of the two declarations' count
/// (a signature that doesn't even match in length) isn't compared.
#[allow(dead_code)] // unit tests; collect_diagnostics uses visitor()
pub fn check_with<E: ExternalSignatures + ?Sized>(
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

/// Does nothing: without an [`ExternalSignatures`] resolver there is no
/// inherited declaration to rename toward. See [`repair_with`].
#[allow(dead_code)]
pub fn repair(
    source: &str,
    ast: Option<&papyrus_parser::ast::Script>,
    tokens: Option<&[papyrus_parser::token::Token]>,
    config: &crate::config::Config,
) -> String {
    let _ = (ast, tokens, config);
    repair_with(source, &mut NoExternalSignatures)
}

/// Renames each overridden parameter [`check_with`] would flag to the
/// inherited name, including references inside that function's body.
///
/// A rename whose new name is already a parameter or local in that
/// function is skipped, unless another rename in the same function moves
/// that name away (a swap or rotation). [`check_with`] still reports the
/// mismatch, so a skipped rename can be corrected by hand. Member names
/// (`OtherRef.akRef`) and named-argument labels (`Helper(akRef = akRef)`)
/// are not references to the parameter and are left unchanged.
pub fn repair_with<E: ExternalSignatures + ?Sized>(source: &str, external: &mut E) -> String {
    let Ok(script) = papyrus_parser::parse(source) else {
        return source.to_string();
    };
    let Some(extends) = &script.extends else {
        return source.to_string();
    };
    let Ok(tokens) = papyrus_parser::tokenize(source) else {
        return source.to_string();
    };
    let line_starts = line_starts(source);
    let mut edits = Vec::new();

    for function in &script.functions {
        let Some(parent_params) = external.lookup(extends, &function.name) else {
            continue;
        };
        let renames = parameter_renames(function, &parent_params);
        if renames.is_empty() {
            continue;
        }
        let Some(start_index) = tokens.iter().position(|token| token.line == function.line) else {
            continue;
        };
        let end_index = tokens
            .iter()
            .enumerate()
            .skip(start_index)
            .find(|(_, token)| {
                matches!(
                    token.kind,
                    TokenKind::Keyword(Keyword::EndFunction | Keyword::EndEvent)
                )
            })
            .map(|(index, _)| index)
            .unwrap_or(tokens.len().saturating_sub(1));

        let mut skip_function_name = false;
        for index in start_index..=end_index {
            let token = &tokens[index];
            match &token.kind {
                TokenKind::Keyword(Keyword::Function | Keyword::Event) => {
                    skip_function_name = true;
                }
                TokenKind::Identifier(_) if skip_function_name => {
                    skip_function_name = false;
                }
                TokenKind::Identifier(name) => {
                    if is_member_access(neighbor_kind(&tokens, index, false))
                        || is_named_argument_label(
                            neighbor_kind(&tokens, index, false),
                            neighbor_kind(&tokens, index, true),
                        )
                    {
                        continue;
                    }
                    if let Some(replacement) = renames.get(&name.to_ascii_lowercase()) {
                        if replacement != name {
                            let start = line_starts[token.line - 1] + token.col - 1;
                            edits.push((start, start + name.len(), replacement.clone()));
                        }
                    }
                }
                _ => {}
            }
        }
    }

    edits.sort_by_key(|edit| std::cmp::Reverse(edit.0));
    let mut repaired = source.to_string();
    for (start, end, replacement) in edits {
        repaired.replace_range(start..end, &replacement);
    }
    repaired
}

/// Drifted parameters mapped from their current name (compared
/// case-insensitively) to the inherited spelling.
///
/// The inherited name is omitted when it is already declared on a
/// parameter or local that no rename in this function moves away.
/// Applying it would turn the warning into a duplicate-name compile error.
fn parameter_renames(
    function: &FunctionDecl,
    parent_params: &[ParamInfo],
) -> HashMap<String, String> {
    let declared = declared_names(function);
    let candidates: Vec<(&str, &str)> = function
        .params
        .iter()
        .zip(parent_params)
        .filter(|(local, parent)| !local.name.eq_ignore_ascii_case(&parent.name))
        .map(|(local, parent)| (local.name.as_str(), parent.name.as_str()))
        .collect();

    // A rename may land on a name another rename vacates. Dropping one
    // can stop that name from being vacated, so repeat until stable.
    let mut kept = vec![true; candidates.len()];
    let mut changed = true;
    while changed {
        changed = false;
        for (index, (_, new_name)) in candidates.iter().enumerate() {
            if !kept[index] {
                continue;
            }
            let target = new_name.to_ascii_lowercase();
            let vacated = candidates.iter().enumerate().any(|(other, (old_name, _))| {
                kept[other] && old_name.eq_ignore_ascii_case(&target)
            });
            if declared.contains(&target) && !vacated {
                kept[index] = false;
                changed = true;
            }
        }
    }

    let mut renames = HashMap::new();
    for (index, (old_name, new_name)) in candidates.into_iter().enumerate() {
        if kept[index] {
            renames.insert(old_name.to_ascii_lowercase(), new_name.to_string());
        }
    }
    renames
}

fn declared_names(function: &FunctionDecl) -> HashSet<String> {
    let mut names = HashSet::new();
    for param in &function.params {
        names.insert(param.name.to_ascii_lowercase());
    }
    collect_declared_locals(&function.body, &mut names);
    names
}

/// Papyrus locals are function-scoped, so a declaration nested in `If` or
/// `While` still collides with a parameter of the same name.
fn collect_declared_locals(body: &[Stmt], names: &mut HashSet<String>) {
    for stmt in body {
        match stmt {
            Stmt::VarDecl(decl) => {
                names.insert(decl.name.to_ascii_lowercase());
            }
            Stmt::If {
                branches,
                else_body,
                ..
            } => {
                for branch in branches {
                    collect_declared_locals(&branch.body, names);
                }
                collect_declared_locals(else_body, names);
            }
            Stmt::While { body, .. } => collect_declared_locals(body, names),
            _ => {}
        }
    }
}

fn is_member_access(previous: Option<&TokenKind>) -> bool {
    matches!(previous, Some(TokenKind::Dot))
}

/// `Helper(akRef = akRef)` and `Helper(other, akRef = akRef)`. Only a
/// single `=` counts; `==` and `+=` are ordinary parameter uses.
fn is_named_argument_label(previous: Option<&TokenKind>, next: Option<&TokenKind>) -> bool {
    matches!(previous, Some(TokenKind::LParen | TokenKind::Comma))
        && matches!(next, Some(TokenKind::Assign))
}

/// Previous (`forward == false`) or next token, ignoring newlines and
/// comment annotations. Those are not part of the member-access or
/// named-argument syntax this check is looking at.
fn neighbor_kind(tokens: &[Token], mut index: usize, forward: bool) -> Option<&TokenKind> {
    loop {
        index = if forward {
            index.checked_add(1)?
        } else {
            index.checked_sub(1)?
        };
        match &tokens.get(index)?.kind {
            TokenKind::Newline | TokenKind::CommentAnnotation(_) => {}
            kind => return Some(kind),
        }
    }
}

#[cfg(test)]
#[path = "argument_naming_tests.rs"]
mod tests;
