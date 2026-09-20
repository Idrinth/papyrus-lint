//! Prefers Papyrus's named-argument call syntax (`func(argB = 1)`) over
//! positional arguments, according to the configured [`NamedArguments`]
//! policy.
//!
//! Parameter names (and which parameters carry a default value) are only
//! known for functions declared in the script being linted, so only a call
//! resolved to a local function (a bare call, or `self.Func(...)`) is
//! checked; a call to a function declared on another script is left
//! unflagged rather than guessed at. A named argument is always accepted
//! regardless of policy — this lint only ever nudges a positional argument
//! toward the named form, never the reverse.

use std::collections::HashMap;

use papyrus_parser::ast::{Expr, FunctionDecl, Script};
use papyrus_parser::token::{Keyword, Token, TokenKind};
use serde::{Deserialize, Serialize};

use crate::visitor::{AstLint, LintVisitor, Store, VisitCtx};
use crate::Diagnostic;

/// This lint's [`Diagnostic::rule`] id, for `@disable` line comments.
pub const RULE: &str = "named-arguments";

#[derive(Default)]
struct Collect {
    store: Store,
    locals: Option<LocalFunctions>,
}

impl AstLint for Collect {
    fn store(&mut self) -> &mut Store {
        &mut self.store
    }

    fn visit_script(&mut self, script: &Script, _ctx: &mut VisitCtx<'_>) {
        self.locals = Some(LocalFunctions::from_script(script));
    }

    fn visit_expr(&mut self, expr: &Expr, ctx: &mut VisitCtx<'_>) {
        let setting = ctx.config.named_arguments;
        if setting == NamedArguments::Never {
            return;
        }
        let Some(locals) = self.locals.as_ref() else {
            return;
        };
        let Expr::Call {
            callee,
            args,
            line,
            col,
        } = expr
        else {
            return;
        };
        let Some((name, params)) = resolve_local(callee, locals) else {
            return;
        };
        check_call(*line, *col, &name, params, args, setting, &mut self.store);
    }
}

pub fn visitor() -> LintVisitor {
    LintVisitor::Ast(Box::new(Collect::default()))
}

/// How strongly this lint prefers named arguments over positional ones.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum NamedArguments {
    /// Every argument on a call resolved to a local function must be
    /// passed by name.
    Always,
    /// Only an argument filling a parameter that has a default value must
    /// be passed by name; arguments filling a required (no-default)
    /// parameter may stay positional.
    InsteadOfDefaults,
    /// No preference: positional arguments are never flagged.
    #[default]
    Never,
}

/// A declared function parameter's name and whether it has a default
/// value, as needed to resolve both positional and named arguments
/// (`func(argB = 1)`) against it.
#[derive(Clone)]
struct ParamInfo {
    name: String,
    has_default: bool,
}

/// Parameters of the functions declared in the script being linted, keyed
/// by lowercased name. A name declared more than once (e.g. overridden in
/// a state) with a differing signature (including which parameters have
/// defaults, since that decides what this lint requires) is stored as
/// `None`, since which declaration applies at a given call site can't be
/// determined here — such calls are then skipped rather than checked
/// against a possibly-wrong signature.
#[derive(Default)]
struct LocalFunctions {
    by_name: HashMap<String, Option<Vec<ParamInfo>>>,
}

impl LocalFunctions {
    /// Groups every function declared in `script` (including those inside
    /// states) by lowercased name, resolving each to its parameters (or
    /// `None` if declared more than once with differing signatures).
    fn from_script(script: &Script) -> Self {
        let mut grouped: HashMap<String, Vec<&FunctionDecl>> = HashMap::new();
        for function in all_functions(script) {
            grouped
                .entry(function.name.to_ascii_lowercase())
                .or_default()
                .push(function);
        }

        let by_name = grouped
            .into_iter()
            .map(|(name, decls)| {
                let first: Vec<ParamInfo> = decls[0]
                    .params
                    .iter()
                    .map(|p| ParamInfo {
                        name: p.name.clone(),
                        has_default: p.default.is_some(),
                    })
                    .collect();
                let consistent = decls.iter().all(|decl| {
                    decl.params.len() == first.len()
                        && decl.params.iter().zip(&first).all(|(p, expected)| {
                            p.name.eq_ignore_ascii_case(&expected.name)
                                && p.default.is_some() == expected.has_default
                        })
                });
                (name, consistent.then_some(first))
            })
            .collect();

        LocalFunctions { by_name }
    }

    /// Looks up `name`'s parameters, case-insensitively.
    fn lookup(&self, name: &str) -> Option<&[ParamInfo]> {
        self.by_name.get(&name.to_ascii_lowercase())?.as_deref()
    }
}

/// Iterates every function declared directly on a script, plus every
/// function declared in each of its states.
fn all_functions(script: &Script) -> impl Iterator<Item = &FunctionDecl> {
    script.functions.iter().chain(
        script
            .states
            .iter()
            .flat_map(|state| state.functions.iter()),
    )
}

/// Checks `source` for positional call arguments that `setting` prefers to
/// see passed by name instead.
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

/// Resolves `callee` to a local function's name and parameters: either a
/// bare call (`Func(...)`) or one explicitly qualified with `self`
/// (`self.Func(...)`). A call on anything else (another script's property,
/// `Parent`, an array element, ...) can't be resolved to a known parameter
/// list here and is left unchecked.
fn resolve_local<'a>(
    callee: &Expr,
    locals: &'a LocalFunctions,
) -> Option<(String, &'a [ParamInfo])> {
    match callee {
        Expr::Identifier(name) => locals.lookup(name).map(|params| (name.clone(), params)),
        Expr::Member { object, property } if matches!(**object, Expr::Self_) => locals
            .lookup(property)
            .map(|params| (property.clone(), params)),
        _ => None,
    }
}

/// Flags each of `args` that `setting` prefers to see passed by name
/// instead of positionally, based on its matching entry in `params`. An
/// argument already passed by name, or one beyond the declared parameter
/// count, is never flagged.
fn check_call(
    line: usize,
    col: usize,
    function_name: &str,
    params: &[ParamInfo],
    args: &[Expr],
    setting: NamedArguments,
    store: &mut Store,
) {
    for (index, arg) in args.iter().enumerate() {
        if matches!(arg, Expr::NamedArg { .. }) {
            continue;
        }
        let Some(param) = params.get(index) else {
            break;
        };
        let should_flag = match setting {
            NamedArguments::Always => true,
            NamedArguments::InsteadOfDefaults => param.has_default,
            NamedArguments::Never => false,
        };
        if !should_flag {
            continue;
        }

        store.emit(
            line,
            col,
            format!(
                "[warning] Argument {} to '{}' should be passed as a named argument ({} = ...)",
                index + 1,
                function_name,
                param.name
            ),
            RULE,
        );
    }
}

/// Rewrites each positional call argument `setting` prefers to see passed
/// by name (per [`check`]) into its named form, e.g. `Greet("hi")` becomes
/// `Greet(name = "hi")`. Works from the same locally declared parameter
/// names/defaults `check` resolves calls against, so it flags (and fixes)
/// exactly the arguments `check` reports and nothing else; unparseable
/// source is returned unchanged, the same way `check` reports nothing for
/// it.
pub fn repair(
    source: &str,
    ast: Option<&papyrus_parser::ast::Script>,
    tokens: Option<&[papyrus_parser::token::Token]>,
    config: &crate::config::Config,
) -> String {
    let _ = (ast, tokens);
    let setting = config.named_arguments;

    if setting == NamedArguments::Never {
        return source.to_string();
    }
    let (Ok(script), Ok(tokens)) = (
        papyrus_parser::parse_for_game(config.game, source),
        papyrus_parser::tokenize(source),
    ) else {
        return source.to_string();
    };

    let locals = LocalFunctions::from_script(&script);
    let insertions = collect_insertions(&tokens, &locals, setting);
    if insertions.is_empty() {
        return source.to_string();
    }

    let line_starts = line_starts(source);
    let mut repaired = String::with_capacity(source.len() + insertions.len() * 8);
    let mut previous = 0;
    for (line, col, text) in insertions {
        let offset = line_starts[line - 1] + col - 1;
        repaired.push_str(&source[previous..offset]);
        repaired.push_str(&text);
        previous = offset;
    }
    repaired.push_str(&source[previous..]);
    repaired
}

/// Tracks one call's argument list while [`collect_insertions`] scans
/// tokens: the paren depth its own arguments live at (so a nested call's
/// tokens are never mistaken for this call's own), the parameters to check
/// them against (`None` for a call `resolve_call_at` couldn't resolve to a
/// local function, tracked purely so its depth doesn't get lost), which
/// argument position is current, and whether the next token starts a new
/// argument (right after `(` or a top-level `,`).
struct CallContext<'a> {
    depth: usize,
    params: Option<&'a [ParamInfo]>,
    arg_index: usize,
    at_arg_start: bool,
}

/// Scans `tokens` for call sites [`resolve_call_at`] resolves to a local
/// function and returns, for each positional argument `setting` prefers
/// named, the `(line, col, text)` to splice `text` in at ahead of that
/// argument's first token — mirroring [`check_call`]'s own flagging
/// decision exactly, just against tokens instead of the parsed `Expr::Call`
/// nodes `check`/`walk_expr` use, since only tokens carry the per-argument
/// source position this needs.
fn collect_insertions(
    tokens: &[Token],
    locals: &LocalFunctions,
    setting: NamedArguments,
) -> Vec<(usize, usize, String)> {
    let mut insertions = Vec::new();
    let mut depth = 0usize;
    let mut stack: Vec<CallContext> = Vec::new();

    for (index, token) in tokens.iter().enumerate() {
        match &token.kind {
            TokenKind::LParen => {
                depth += 1;
                let params = resolve_call_at(tokens, index, locals);
                if params.is_some() {
                    stack.push(CallContext {
                        depth,
                        params,
                        arg_index: 0,
                        at_arg_start: true,
                    });
                }
            }
            TokenKind::RParen => {
                if stack.last().is_some_and(|top| top.depth == depth) {
                    stack.pop();
                }
                depth = depth.saturating_sub(1);
            }
            TokenKind::Comma => {
                if let Some(top) = stack.last_mut() {
                    if top.depth == depth {
                        top.arg_index += 1;
                        top.at_arg_start = true;
                    }
                }
            }
            _ => {
                let Some(top) = stack.last_mut() else {
                    continue;
                };
                if top.depth != depth || !top.at_arg_start {
                    continue;
                }
                top.at_arg_start = false;
                let already_named = matches!(token.kind, TokenKind::Identifier(_))
                    && matches!(
                        tokens.get(index + 1).map(|next| &next.kind),
                        Some(TokenKind::Assign)
                    );
                if already_named {
                    continue;
                }
                let Some(params) = top.params else {
                    continue;
                };
                let Some(param) = params.get(top.arg_index) else {
                    continue;
                };
                let should_flag = match setting {
                    NamedArguments::Always => true,
                    NamedArguments::InsteadOfDefaults => param.has_default,
                    NamedArguments::Never => false,
                };
                if should_flag {
                    insertions.push((token.line, token.col, format!("{} = ", param.name)));
                }
            }
        }
    }
    insertions
}

/// Resolves the call whose argument list opens at `tokens[lparen_index]`
/// (an `LParen`) to a local function's parameters, the same way
/// [`resolve_local`] resolves an already-parsed `Expr::Call`'s callee: a
/// bare `Func(` not itself the target of a member access, or `Self.Func(`.
/// Anything else (a call through another object, `Parent`, ...) resolves to
/// `None`.
fn resolve_call_at<'a>(
    tokens: &[Token],
    lparen_index: usize,
    locals: &'a LocalFunctions,
) -> Option<&'a [ParamInfo]> {
    let name_index = lparen_index.checked_sub(1)?;
    let TokenKind::Identifier(name) = &tokens.get(name_index)?.kind else {
        return None;
    };

    let preceding = name_index.checked_sub(1).and_then(|i| tokens.get(i));
    // A `Function`/`Event` keyword directly before the identifier means
    // this `(` opens that declaration's own parameter list, not a call —
    // the only other place a local function's name is directly followed
    // by `(`.
    if preceding.is_some_and(|t| {
        matches!(
            t.kind,
            TokenKind::Keyword(Keyword::Function) | TokenKind::Keyword(Keyword::Event)
        )
    }) {
        return None;
    }

    let dot_index = name_index
        .checked_sub(1)
        .filter(|&i| matches!(tokens.get(i).map(|t| &t.kind), Some(TokenKind::Dot)));
    let Some(dot_index) = dot_index else {
        return locals.lookup(name);
    };

    let is_self = dot_index
        .checked_sub(1)
        .and_then(|i| tokens.get(i))
        .is_some_and(|t| matches!(t.kind, TokenKind::Keyword(Keyword::Self_)));
    if is_self {
        locals.lookup(name)
    } else {
        None
    }
}

/// Byte offset of the start of each line in `source` (index 0 is always
/// `0`), so a token's 1-indexed `(line, col)` position (`col` itself a
/// byte offset within its line) can be converted into a byte offset into
/// `source` as a whole.
fn line_starts(source: &str) -> Vec<usize> {
    std::iter::once(0)
        .chain(
            source
                .bytes()
                .enumerate()
                .filter_map(|(index, byte)| (byte == b'\n').then_some(index + 1)),
        )
        .collect()
}

#[cfg(test)]
#[path = "named_arguments_tests.rs"]
mod tests;
