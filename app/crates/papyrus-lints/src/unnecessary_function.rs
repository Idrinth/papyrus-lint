//! Flags a `Function` whose body consists of exactly one statement, since
//! it adds an indirection without doing enough on its own to justify a
//! separate declaration — a caller could just as well inline that one
//! statement instead.
//!
//! Only `Function`s are checked. `Event`s are always left alone: they're
//! declared by the engine rather than the script's own author, so a
//! single-statement handler may well be forwarding to shared logic used by
//! other events too, which is a reasonable reason for it to exist on its
//! own. A `Function` named `Fragment_<digits>` (e.g. `Fragment_0`) is left
//! alone for the same reason: CreationKit generates that name and calls it
//! directly, so it isn't a wrapper the script's own author could inline
//! away. A parameterless `Function` returning one of Papyrus's primitive
//! scalar types (`Int`, `Float`, `Bool`, `String`) is left alone too: that
//! shape is how a script exposes a named constant to the outside world
//! (e.g. `Int Function GetFooThreshold() Global` `Return 5`
//! `EndFunction`) without baking the value into every instance as a
//! `Property`/`Variable`, which is a deliberate design rather than an
//! indirection worth inlining away. This works from the parsed AST, so a
//! script that doesn't parse cleanly is left unchecked rather than guessed
//! at.
//!
//! [`repair`] handles the specific, common shape of a single-statement
//! function that's nothing but a "pure forwarding" wrapper around another
//! call — see its own docs for exactly what that requires. It rewrites
//! every call site to call straight through to the wrapped function
//! instead, but leaves the wrapper's own declaration in place: this crate
//! never sees whether some other script still calls it directly, so
//! [`check`] keeps flagging it afterward rather than this silently
//! deleting something another file may depend on.

use std::collections::HashMap;

use papyrus_parser::ast::{Expr, FunctionDecl, IfBranch, Param, Script, Stmt, TypeName};
use papyrus_parser::token::{Token, TokenKind};

use crate::visitor::{AstLint, LintVisitor, Store, VisitCtx};
use crate::Diagnostic;
use crate::token_walk::{line_starts, matching_close_paren};

/// This lint's [`Diagnostic::rule`] id, for `@disable` line comments.
pub const RULE: &str = "unnecessary-function";

#[derive(Default)]
struct Collect {
    store: Store,
}

impl AstLint for Collect {
    fn store(&mut self) -> &mut Store {
        &mut self.store
    }

    fn visit_function(&mut self, function: &FunctionDecl, _ctx: &mut VisitCtx<'_>) {
        if function.is_event || function.body.len() != 1 || is_fragment_function(&function.name) {
            return;
        }
        if function.params.is_empty() && returns_simple_type(&function.return_type) {
            return;
        }
        self.store.emit(
            function.line,
            1,
            format!(
                "[info] Function '{}' contains only a single statement; consider inlining it \
                 at its call site(s) instead of keeping it as a separate function",
                function.name
            ),
            RULE,
        );
    }
}

pub fn visitor() -> LintVisitor {
    LintVisitor::Ast(Box::new(Collect::default()))
}

/// Checks `source` for `Function`s whose body is exactly one statement long.
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

/// Whether `name` is a CreationKit-generated fragment function name
/// (`Fragment_` followed by one or more ASCII digits, case-insensitively),
/// which the engine calls directly rather than the script's own author.
fn is_fragment_function(name: &str) -> bool {
    name.get(..9)
        .is_some_and(|prefix| prefix.eq_ignore_ascii_case("Fragment_"))
        && !name[9..].is_empty()
        && name[9..].bytes().all(|byte| byte.is_ascii_digit())
}

/// Whether `return_type` is one of Papyrus's primitive scalar types
/// (`Int`, `Float`, `Bool`, `String`), rather than an array or an
/// object/`Form` type — the shape a parameterless function must return to
/// be read as a named constant instead of an inlinable wrapper.
fn returns_simple_type(return_type: &Option<TypeName>) -> bool {
    return_type.as_ref().is_some_and(|type_name| {
        !type_name.is_array
            && matches!(
                type_name.name.to_lowercase().as_str(),
                "int" | "float" | "bool" | "string"
            )
    })
}

/// Rewrites every call site of a "pure forwarding" wrapper function into a
/// direct call to whatever it wraps, leaving the wrapper's own declaration
/// untouched (see the module docs for why).
///
/// A `Function` only qualifies as a wrapper here when: its body is exactly
/// one statement, that statement is itself nothing but a call (`B(...)`,
/// not an assignment/return/condition/...); every one of the wrapper's own
/// parameters is passed into that call, by name and in the same order, as
/// a bare identifier (so `Function A(Int x, Bool y)` must call `B(x, y)`,
/// not `B(y, x)` or `B(x, true)`); none of the wrapper's parameters carries
/// a default value (a call site omitting one to fall back on it would
/// silently pick up whatever default `B` has instead, which may differ or
/// not exist); and no other function or event anywhere in the script
/// (including a state override) shares its name, since a call site could
/// then be reaching a different implementation this pass never looked at.
/// The wrapped call itself must be a bare `Name(...)` call or a single
/// `Object.Name(...)` member call whose object is `Self` or a plain
/// identifier (e.g. a property) — a deeper chain, an indexed/computed
/// callee, or a call to `Parent.Name(...)` (a different, per-instance
/// target) is left alone rather than guessed at.
///
/// Once a wrapper qualifies, every call to it anywhere in the script
/// (nested inside expressions, conditions, other calls' arguments, ...) is
/// rewritten to call the wrapped function directly instead, keeping that
/// call site's own argument list exactly as written — safe precisely
/// because the wrapper forwards every parameter unchanged. A call site
/// that passes any argument by name (`A(argB = 1)`) is left alone even
/// then: after rewriting, that name would be resolved against the wrapped
/// function's own parameters, which may not share the wrapper's names.
pub fn repair(
    source: &str,
    ast: Option<&papyrus_parser::ast::Script>,
    tokens: Option<&[papyrus_parser::token::Token]>,
    config: &crate::config::Config,
) -> String {
    let _ = (ast, tokens, config);

    let (Ok(script), Ok(tokens)) = (
        papyrus_parser::parse(source),
        papyrus_parser::tokenize(source),
    ) else {
        return source.to_string();
    };
    let ctx = SourceContext {
        tokens: &tokens,
        line_starts: line_starts(source),
        source,
    };

    let mut name_counts: HashMap<String, usize> = HashMap::new();
    for function in all_functions(&script) {
        *name_counts.entry(function.name.to_lowercase()).or_insert(0) += 1;
    }

    let mut wrapped_callees: HashMap<String, String> = HashMap::new();
    for function in all_functions(&script) {
        if function.is_event || function.body.len() != 1 || is_fragment_function(&function.name) {
            continue;
        }
        let key = function.name.to_lowercase();
        if name_counts.get(&key).copied().unwrap_or(0) != 1 {
            continue;
        }
        if function.params.iter().any(|param| param.default.is_some()) {
            continue;
        }
        let Stmt::Expr { value, .. } = &function.body[0] else {
            continue;
        };
        let Expr::Call {
            callee,
            args,
            line,
            col,
        } = value
        else {
            continue;
        };
        if !forwards_params(args, &function.params) {
            continue;
        }
        let Some(text) = callee_source_text(callee, *line, *col, &ctx) else {
            continue;
        };
        wrapped_callees.insert(key, text);
    }

    if wrapped_callees.is_empty() {
        return source.to_string();
    }

    let mut edits = Vec::new();
    for function in all_functions(&script) {
        collect_call_site_edits(&function.body, &wrapped_callees, &ctx, &mut edits);
    }
    if edits.is_empty() {
        return source.to_string();
    }
    edits.sort_by_key(|edit| std::cmp::Reverse(edit.0));

    let mut repaired = source.to_string();
    for (start, end, replacement) in edits {
        repaired.replace_range(start..end, &replacement);
    }
    repaired
}

/// Bundles the pieces [`repair`]'s helpers need to translate an AST node
/// back into a source span: the token stream, each line's starting byte
/// offset (see [`line_starts`]/[`token_offset`]), and the source text
/// itself.
struct SourceContext<'a> {
    tokens: &'a [Token],
    line_starts: Vec<usize>,
    source: &'a str,
}

/// Whether `args` is exactly `params`, forwarded by name and in order, as
/// bare identifiers — the only shape [`repair`] trusts enough to reuse a
/// call site's own arguments unchanged against the wrapped call.
fn forwards_params(args: &[Expr], params: &[Param]) -> bool {
    args.len() == params.len()
        && args.iter().zip(params).all(|(arg, param)| {
            matches!(arg, Expr::Identifier(name) if name.eq_ignore_ascii_case(&param.name))
        })
}

/// The lowercased simple name a call's `callee` invokes, and how many
/// tokens (immediately preceding the call's own opening paren) that name
/// spans: `1` for a bare `Name(...)` call, `3` for a single
/// `Object.Name(...)` member call (the token sequence `<object> Dot
/// <property>`), where `<object>` is `Self` or a plain identifier.
/// Anything less direct (a deeper member chain, `Parent.Name(...)`, an
/// indexed/cast/computed callee, ...) returns `None` rather than being
/// guessed at.
fn simple_callee(callee: &Expr) -> Option<(String, usize)> {
    match callee {
        Expr::Identifier(name) => Some((name.to_lowercase(), 1)),
        Expr::Member { object, property }
            if matches!(object.as_ref(), Expr::Identifier(_) | Expr::Self_) =>
        {
            Some((property.to_lowercase(), 3))
        }
        _ => None,
    }
}

/// The source text of `callee` itself (e.g. `B` or `Self.B`), recovered by
/// locating the call's own opening paren (`line`/`col`, the position
/// `Expr::Call` records for it) among `ctx`'s tokens and slicing back the
/// number of tokens [`simple_callee`] says the callee spans.
fn callee_source_text(
    callee: &Expr,
    line: usize,
    col: usize,
    ctx: &SourceContext,
) -> Option<String> {
    let (_, token_span) = simple_callee(callee)?;
    let open_index = open_paren_index(ctx.tokens, line, col)?;
    let start_index = open_index.checked_sub(token_span)?;
    let start_offset = token_offset(&ctx.line_starts, &ctx.tokens[start_index]);
    let open_offset = token_offset(&ctx.line_starts, &ctx.tokens[open_index]);
    let text = ctx.source[start_offset..open_offset].trim_end();
    (!text.is_empty()).then(|| text.to_string())
}

/// Walks every statement in `body` (and any nested `If`/`While` body,
/// condition, or expression) looking for a call site to rewrite; see
/// [`collect_call_site_edits_in_expr`].
fn collect_call_site_edits(
    body: &[Stmt],
    wrapped_callees: &HashMap<String, String>,
    ctx: &SourceContext,
    edits: &mut Vec<(usize, usize, String)>,
) {
    for stmt in body {
        match stmt {
            Stmt::If {
                branches,
                else_body,
                ..
            } => {
                for IfBranch {
                    condition, body, ..
                } in branches
                {
                    collect_call_site_edits_in_expr(condition, wrapped_callees, ctx, edits);
                    collect_call_site_edits(body, wrapped_callees, ctx, edits);
                }
                collect_call_site_edits(else_body, wrapped_callees, ctx, edits);
            }
            Stmt::While {
                condition, body, ..
            } => {
                collect_call_site_edits_in_expr(condition, wrapped_callees, ctx, edits);
                collect_call_site_edits(body, wrapped_callees, ctx, edits);
            }
            Stmt::VarDecl(decl) => {
                if let Some(value) = &decl.value {
                    collect_call_site_edits_in_expr(value, wrapped_callees, ctx, edits);
                }
            }
            Stmt::Assign { target, value, .. } => {
                collect_call_site_edits_in_expr(target, wrapped_callees, ctx, edits);
                collect_call_site_edits_in_expr(value, wrapped_callees, ctx, edits);
            }
            Stmt::Expr { value, .. } => {
                collect_call_site_edits_in_expr(value, wrapped_callees, ctx, edits);
            }
            Stmt::Return { value, .. } => {
                if let Some(value) = value {
                    collect_call_site_edits_in_expr(value, wrapped_callees, ctx, edits);
                }
            }
        }
    }
}

/// Looks for a call site to rewrite at `expr` itself, then recurses into
/// every sub-expression that could contain another one (a call's own
/// callee/arguments, either side of a binary/unary/member/index
/// expression, a cast's value, or a `new` array's size).
fn collect_call_site_edits_in_expr(
    expr: &Expr,
    wrapped_callees: &HashMap<String, String>,
    ctx: &SourceContext,
    edits: &mut Vec<(usize, usize, String)>,
) {
    match expr {
        Expr::Call {
            callee,
            args,
            line,
            col,
        } => {
            if let Some(edit) =
                build_call_site_edit(callee, args, *line, *col, wrapped_callees, ctx)
            {
                edits.push(edit);
            }
            collect_call_site_edits_in_expr(callee, wrapped_callees, ctx, edits);
            for arg in args {
                collect_call_site_edits_in_expr(arg, wrapped_callees, ctx, edits);
            }
        }
        Expr::NamedArg { value, .. } => {
            collect_call_site_edits_in_expr(value, wrapped_callees, ctx, edits);
        }
        Expr::Binary { left, right, .. } => {
            collect_call_site_edits_in_expr(left, wrapped_callees, ctx, edits);
            collect_call_site_edits_in_expr(right, wrapped_callees, ctx, edits);
        }
        Expr::Unary { operand, .. } => {
            collect_call_site_edits_in_expr(operand, wrapped_callees, ctx, edits);
        }
        Expr::Member { object, .. } => {
            collect_call_site_edits_in_expr(object, wrapped_callees, ctx, edits);
        }
        Expr::Index { object, index } => {
            collect_call_site_edits_in_expr(object, wrapped_callees, ctx, edits);
            collect_call_site_edits_in_expr(index, wrapped_callees, ctx, edits);
        }
        Expr::Cast { value, .. } | Expr::Is { value, .. } => {
            collect_call_site_edits_in_expr(value, wrapped_callees, ctx, edits);
        }
        Expr::NewArray { size, .. } => {
            collect_call_site_edits_in_expr(size, wrapped_callees, ctx, edits);
        }
        Expr::Literal(_) | Expr::Identifier(_) | Expr::Self_ | Expr::Parent => {}
        Expr::NewStruct { .. } => {}
    }
}

/// Builds the `(start, end, replacement)` edit rewriting a single call site
/// (`callee(args)`, whose call's own opening paren is at `line`/`col`)
/// into the wrapped call it should reach instead, or `None` if it doesn't
/// resolve to one of `wrapped_callees` at all, or uses a named argument
/// (see [`repair`]'s own docs for why that blocks the rewrite).
fn build_call_site_edit(
    callee: &Expr,
    args: &[Expr],
    line: usize,
    col: usize,
    wrapped_callees: &HashMap<String, String>,
    ctx: &SourceContext,
) -> Option<(usize, usize, String)> {
    let (name, token_span) = simple_callee(callee)?;
    let replacement_callee = wrapped_callees.get(&name)?;
    if args.iter().any(|arg| matches!(arg, Expr::NamedArg { .. })) {
        return None;
    }

    let open_index = open_paren_index(ctx.tokens, line, col)?;
    let close_index = matching_close_paren(ctx.tokens, open_index)?;
    let callee_start_index = open_index.checked_sub(token_span)?;

    let callee_start_offset = token_offset(&ctx.line_starts, &ctx.tokens[callee_start_index]);
    let args_start_offset = token_offset(&ctx.line_starts, &ctx.tokens[open_index]) + 1;
    let close_offset = token_offset(&ctx.line_starts, &ctx.tokens[close_index]);
    let args_text = ctx.source[args_start_offset..close_offset].trim();

    Some((
        callee_start_offset,
        close_offset + 1,
        format!("{replacement_callee}({args_text})"),
    ))
}

/// Finds the token index of the `(` at exactly `line`/`col` — the position
/// `Expr::Call` records for its own opening paren.
fn open_paren_index(tokens: &[Token], line: usize, col: usize) -> Option<usize> {
    tokens
        .iter()
        .position(|token| token.line == line && token.col == col && token.kind == TokenKind::LParen)
}

fn token_offset(line_starts: &[usize], token: &Token) -> usize {
    line_starts[token.line - 1] + token.col - 1
}

#[cfg(test)]
#[path = "unnecessary_function_tests.rs"]
mod tests;
