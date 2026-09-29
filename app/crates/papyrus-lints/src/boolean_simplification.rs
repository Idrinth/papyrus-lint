//! Flags a `Bool` compared with `==` or `!=` against a `true`/`false`
//! literal. `flag == true` / `flag != false` are just `flag`, and
//! `flag == false` / `flag != true` are just `! flag`.
//!
//! Like [`crate::strict_boolean`], this only rewrites an operand whose
//! type is known locally (see [`papyrus_parser::types::infer_type`]). A
//! call, a member access, or a non-`Bool` operand is left alone rather
//! than guessed at. Comparisons inside a full property Get/Set block are
//! not checked: the parser skips those bodies, so they never reach the AST.

use papyrus_parser::ast::{
    BinaryOp, Expr, FunctionDecl, Literal, Script, Stmt, TypeName,
};
use papyrus_parser::token::{Keyword, Token, TokenKind};
use papyrus_parser::types::{infer_type, TypeEnv};

use crate::token_walk::line_starts;
use crate::visitor::{AstLint, LintVisitor, Store, VisitCtx};
use crate::Diagnostic;

/// This lint's [`Diagnostic::rule`] id, for `@disable` line comments.
pub const RULE: &str = "boolean-simplification";

#[derive(Default)]
struct Collect {
    store: Store,
}

impl AstLint for Collect {
    fn store(&mut self) -> &mut Store {
        &mut self.store
    }

    fn finish(&mut self, ctx: &mut VisitCtx<'_>) {
        let (Some(script), Some(tokens)) = (ctx.ast, ctx.tokens) else {
            return;
        };
        for hit in find_hits(ctx.source, script, tokens) {
            self.store
                .emit(hit.line, hit.column, hit.message, RULE);
        }
    }
}

pub fn visitor() -> LintVisitor {
    LintVisitor::Ast(Box::new(Collect::default()))
}

/// Checks `source` for redundant `Bool`/`true`/`false` comparisons.
#[allow(dead_code)] // unit tests; collect_diagnostics uses visitor()
pub fn check(
    source: &str,
    ast: Option<&Script>,
    tokens: Option<&[Token]>,
    config: &crate::config::Config,
    external: &mut impl crate::external_signatures::ExternalSignatures,
) -> Vec<Diagnostic> {
    crate::visitor::run(visitor(), source, ast, tokens, config, external)
}

/// Rewrites every comparison [`check`] would flag. A line suppressed with
/// `@disable` / `@disable-file` is left unchanged. Nested comparisons are
/// simplified from the inside out, so `(ready == true) == false` becomes
/// `! ready` in one repair.
pub fn repair(
    source: &str,
    ast: Option<&Script>,
    tokens: Option<&[Token]>,
    config: &crate::config::Config,
) -> String {
    let _ = (ast, tokens, config);
    let mut current = source.to_string();
    for _ in 0..16 {
        let Ok(tokens) = papyrus_parser::tokenize(&current) else {
            return current;
        };
        let Ok(parsed) = papyrus_parser::parse(&current) else {
            return current;
        };
        let disables = crate::disable_comments::Disables::scan(&current);
        let hits: Vec<Hit> = find_hits(&current, &parsed, &tokens)
            .into_iter()
            .filter(|hit| !disables.is_disabled(hit.line, RULE))
            .collect();
        let innermost = innermost_hits(&hits);
        if innermost.is_empty() {
            break;
        }
        let next = apply_edits(&current, &innermost);
        if next == current {
            break;
        }
        current = next;
    }
    current
}

struct Hit {
    line: usize,
    column: usize,
    start: usize,
    end: usize,
    message: String,
    replacement: String,
}

#[derive(Clone, Copy, PartialEq, Eq)]
struct Shape {
    op: BinaryOp,
    left_lit: Option<bool>,
    right_lit: Option<bool>,
}

struct AstComparison {
    shape: Shape,
    operand_is_bool: bool,
}

struct TokenComparison {
    shape: Shape,
    op_index: usize,
    left: std::ops::Range<usize>,
    right: std::ops::Range<usize>,
}

fn find_hits(source: &str, script: &Script, tokens: &[Token]) -> Vec<Hit> {
    let ast_comps = collect_ast_comparisons(script);
    let ignored = unparsed_property_bodies(tokens);
    let token_comps = collect_token_comparisons(tokens, &ignored);
    if ast_comps.len() != token_comps.len() {
        return Vec::new();
    }
    let line_starts = line_starts(source);
    let mut hits = Vec::new();
    for (ast, token) in ast_comps.iter().zip(token_comps.iter()) {
        if ast.shape != token.shape || !ast.operand_is_bool {
            if ast.shape != token.shape {
                return Vec::new();
            }
            continue;
        }
        let Some((start, end)) = comparison_span(source, &line_starts, tokens, token) else {
            continue;
        };
        let replacement = replacement_text(source, &line_starts, tokens, token);
        let message = describe(token.shape);
        hits.push(Hit {
            line: tokens[token.op_index].line,
            column: tokens[token.op_index].col,
            start,
            end,
            message,
            replacement,
        });
    }
    hits
}

fn describe(shape: Shape) -> String {
    if let (Some(left), Some(right)) = (shape.left_lit, shape.right_lit) {
        let value = fold(shape.op, left, right);
        let word = if value { "true" } else { "false" };
        return format!(
            "[info] Redundant comparison of boolean literals; this is always {word}"
        );
    }
    let literal = shape.left_lit.or(shape.right_lit).unwrap_or(true);
    let negate = should_negate(shape.op, literal);
    let op = match shape.op {
        BinaryOp::Eq => "==",
        BinaryOp::NotEq => "!=",
        _ => "==",
    };
    let lit = if literal { "true" } else { "false" };
    let instead = if negate {
        "use the value's negation instead"
    } else {
        "use the value itself instead"
    };
    format!("[info] Redundant Bool comparison '{op} {lit}'; {instead}")
}

fn should_negate(op: BinaryOp, literal_is_true: bool) -> bool {
    match op {
        BinaryOp::Eq => !literal_is_true,
        BinaryOp::NotEq => literal_is_true,
        _ => false,
    }
}

fn fold(op: BinaryOp, left: bool, right: bool) -> bool {
    match op {
        BinaryOp::Eq => left == right,
        BinaryOp::NotEq => left != right,
        _ => false,
    }
}

fn replacement_text(
    source: &str,
    line_starts: &[usize],
    tokens: &[Token],
    token: &TokenComparison,
) -> String {
    if let (Some(left), Some(right)) = (token.shape.left_lit, token.shape.right_lit) {
        let value = fold(token.shape.op, left, right);
        return literal_spelling(source, line_starts, tokens, token, value);
    }
    let literal = token.shape.left_lit.or(token.shape.right_lit).unwrap_or(true);
    let negate = should_negate(token.shape.op, literal);
    let operand = if token.shape.left_lit.is_some() {
        token.right.clone()
    } else {
        token.left.clone()
    };
    let text = token_slice(source, line_starts, tokens, &operand);
    render_operand(text.trim(), negate)
}

fn literal_spelling(
    source: &str,
    line_starts: &[usize],
    tokens: &[Token],
    token: &TokenComparison,
    value: bool,
) -> String {
    for range in [&token.left, &token.right] {
        for index in range.clone() {
            let matches = match &tokens[index].kind {
                TokenKind::Keyword(Keyword::True) => value,
                TokenKind::Keyword(Keyword::False) => !value,
                _ => false,
            };
            if matches {
                let start = token_offset(line_starts, &tokens[index]);
                let end = lexeme_end(source, start);
                return source[start..end].to_string();
            }
        }
    }
    if value { "true" } else { "false" }.to_string()
}

fn render_operand(text: &str, negate: bool) -> String {
    let current = strip_tight_parens(text.trim());
    if !negate {
        return current;
    }
    if let Some(inner) = strip_one_not(current.trim()) {
        return strip_tight_parens(inner.trim());
    }
    if text_needs_parens(&current) {
        if strip_wrapping_parens(&current).is_some() {
            format!("! {current}")
        } else {
            format!("! ({current})")
        }
    } else {
        format!("! {current}")
    }
}

fn strip_tight_parens(text: &str) -> String {
    let mut current = text.trim().to_string();
    while let Some(inner) = strip_wrapping_parens(current.trim()) {
        if text_needs_parens(inner.trim()) {
            break;
        }
        current = inner.trim().to_string();
    }
    current
}

fn strip_wrapping_parens(text: &str) -> Option<&str> {
    let text = text.trim();
    if !text.starts_with('(') || !text.ends_with(')') {
        return None;
    }
    let bytes = text.as_bytes();
    let mut depth = 0i32;
    for (index, byte) in bytes.iter().enumerate() {
        match byte {
            b'(' => depth += 1,
            b')' => {
                depth -= 1;
                if depth == 0 {
                    return (index == bytes.len() - 1).then_some(&text[1..index]);
                }
            }
            _ => {}
        }
    }
    None
}

fn strip_one_not(text: &str) -> Option<&str> {
    let rest = text.trim().strip_prefix('!')?;
    if rest.starts_with('=') {
        return None;
    }
    Some(rest)
}

fn text_needs_parens(text: &str) -> bool {
    let bytes = text.as_bytes();
    let mut index = 0usize;
    let mut depth = 0i32;
    let mut seen_value = false;
    while index < bytes.len() {
        let byte = bytes[index];
        if byte == b'"' {
            index = string_end(bytes, index);
            if depth == 0 {
                seen_value = true;
            }
            continue;
        }
        match byte {
            b'(' | b'[' => {
                depth += 1;
                index += 1;
            }
            b')' | b']' => {
                depth -= 1;
                index += 1;
                if depth == 0 {
                    seen_value = true;
                }
            }
            b'&' | b'|' | b'=' | b'!' | b'>' | b'<' | b'+' | b'*' | b'/' | b'%' | b'-'
                if depth == 0 =>
            {
                let two = matches!(
                    (byte, bytes.get(index + 1)),
                    (b'&', Some(b'&'))
                        | (b'|', Some(b'|'))
                        | (b'=', Some(b'='))
                        | (b'!', Some(b'='))
                        | (b'>', Some(b'='))
                        | (b'<', Some(b'='))
                );
                let is_unary_not = byte == b'!' && !two;
                let is_unary_minus = byte == b'-' && !seen_value;
                if !is_unary_not && !is_unary_minus {
                    return true;
                }
                index += if two { 2 } else { 1 };
            }
            b'a'..=b'z' | b'A'..=b'Z' | b'_' if depth == 0 => {
                let end = word_end(bytes, index);
                let word = &text[index..end];
                if seen_value && (word.eq_ignore_ascii_case("as") || word.eq_ignore_ascii_case("is"))
                {
                    return true;
                }
                seen_value = true;
                index = end;
            }
            b' ' | b'\t' | b'\n' | b'\r' => index += 1,
            _ => {
                if depth == 0 && !byte.is_ascii_whitespace() {
                    seen_value = true;
                }
                index += 1;
            }
        }
    }
    false
}

enum Site<'a> {
    Expr { expr: &'a Expr },
    Function { function: &'a FunctionDecl },
}

fn collect_ast_comparisons(script: &Script) -> Vec<AstComparison> {
    let mut env = TypeEnv::for_script(script);
    let groups = group_property_types(script);
    let mut sites: Vec<(usize, usize, Site<'_>)> = Vec::new();
    let mut order = 0usize;
    for property in &script.properties {
        if let Some(value) = &property.value {
            sites.push((property.line, order, Site::Expr { expr: value }));
            order += 1;
        }
    }
    for variable in &script.variables {
        if let Some(value) = &variable.value {
            sites.push((variable.line, order, Site::Expr { expr: value }));
            order += 1;
        }
    }
    for function in &script.functions {
        sites.push((function.line, order, Site::Function { function }));
        order += 1;
    }
    for state in &script.states {
        for function in &state.functions {
            sites.push((function.line, order, Site::Function { function }));
            order += 1;
        }
    }
    for struct_decl in &script.structs {
        for member in &struct_decl.members {
            if let Some(value) = &member.value {
                sites.push((member.line, order, Site::Expr { expr: value }));
                order += 1;
            }
        }
    }
    for group in &script.groups {
        for property in &group.properties {
            if let Some(value) = &property.value {
                sites.push((property.line, order, Site::Expr { expr: value }));
                order += 1;
            }
        }
    }
    sites.sort_by_key(|(line, seq, _)| (*line, *seq));

    let mut comparisons = Vec::new();
    for (_, _, site) in sites {
        match site {
            Site::Expr { expr } => walk_expr(expr, &env, &groups, &mut comparisons),
            Site::Function { function } => {
                env.enter_function(function);
                for param in &function.params {
                    if let Some(default) = &param.default {
                        walk_expr(default, &env, &groups, &mut comparisons);
                    }
                }
                for stmt in &function.body {
                    walk_stmt(stmt, &env, &groups, &mut comparisons);
                }
                env.leave_function();
            }
        }
    }
    comparisons
}

fn group_property_types(script: &Script) -> std::collections::HashMap<String, TypeName> {
    let mut types = std::collections::HashMap::new();
    for group in &script.groups {
        for property in &group.properties {
            types.insert(property.name.to_ascii_lowercase(), property.type_name.clone());
        }
    }
    types
}

fn walk_stmt(
    stmt: &Stmt,
    env: &TypeEnv,
    groups: &std::collections::HashMap<String, TypeName>,
    out: &mut Vec<AstComparison>,
) {
    match stmt {
        Stmt::VarDecl(variable) => {
            if let Some(value) = &variable.value {
                walk_expr(value, env, groups, out);
            }
        }
        Stmt::Assign { target, value, .. } => {
            walk_expr(target, env, groups, out);
            walk_expr(value, env, groups, out);
        }
        Stmt::Expr { value, .. } => walk_expr(value, env, groups, out),
        Stmt::Return { value, .. } => {
            if let Some(value) = value {
                walk_expr(value, env, groups, out);
            }
        }
        Stmt::If {
            branches,
            else_body,
            ..
        } => {
            for branch in branches {
                walk_expr(&branch.condition, env, groups, out);
                for stmt in &branch.body {
                    walk_stmt(stmt, env, groups, out);
                }
            }
            for stmt in else_body {
                walk_stmt(stmt, env, groups, out);
            }
        }
        Stmt::While {
            condition, body, ..
        } => {
            walk_expr(condition, env, groups, out);
            for stmt in body {
                walk_stmt(stmt, env, groups, out);
            }
        }
        Stmt::LockGuard { body, else_body, .. } => {
            for stmt in body {
                walk_stmt(stmt, env, groups, out);
            }
            for stmt in else_body {
                walk_stmt(stmt, env, groups, out);
            }
        }
    }
}

fn walk_expr(
    expr: &Expr,
    env: &TypeEnv,
    groups: &std::collections::HashMap<String, TypeName>,
    out: &mut Vec<AstComparison>,
) {
    match expr {
        Expr::Literal(_) | Expr::Identifier(_) | Expr::Self_ | Expr::Parent | Expr::NewStruct { .. } => {}
        Expr::Binary { left, op, right } => {
            walk_expr(left, env, groups, out);
            if matches!(op, BinaryOp::Eq | BinaryOp::NotEq) {
                if let Some(comparison) = classify(*op, left, right, env, groups) {
                    out.push(comparison);
                }
            }
            walk_expr(right, env, groups, out);
        }
        Expr::Unary { operand, .. } => walk_expr(operand, env, groups, out),
        Expr::Call { callee, args, .. } => {
            walk_expr(callee, env, groups, out);
            for arg in args {
                walk_expr(arg, env, groups, out);
            }
        }
        Expr::NamedArg { value, .. } => walk_expr(value, env, groups, out),
        Expr::Member { object, .. } => walk_expr(object, env, groups, out),
        Expr::Index { object, index } => {
            walk_expr(object, env, groups, out);
            walk_expr(index, env, groups, out);
        }
        Expr::Cast { value, .. } | Expr::Is { value, .. } => walk_expr(value, env, groups, out),
        Expr::NewArray { size, .. } => walk_expr(size, env, groups, out),
    }
}

fn classify(
    op: BinaryOp,
    left: &Expr,
    right: &Expr,
    env: &TypeEnv,
    groups: &std::collections::HashMap<String, TypeName>,
) -> Option<AstComparison> {
    let left_lit = bool_literal(left);
    let right_lit = bool_literal(right);
    if left_lit.is_none() && right_lit.is_none() {
        return None;
    }
    let operand_is_bool = match (left_lit, right_lit) {
        (Some(_), Some(_)) => true,
        (Some(_), None) => operand_is_bool(right, env, groups),
        (None, Some(_)) => operand_is_bool(left, env, groups),
        (None, None) => false,
    };
    Some(AstComparison {
        shape: Shape {
            op,
            left_lit,
            right_lit,
        },
        operand_is_bool,
    })
}

fn bool_literal(expr: &Expr) -> Option<bool> {
    match expr {
        Expr::Literal(Literal::Bool(value)) => Some(*value),
        _ => None,
    }
}

fn operand_is_bool(
    expr: &Expr,
    env: &TypeEnv,
    groups: &std::collections::HashMap<String, TypeName>,
) -> bool {
    if let Expr::Identifier(name) = expr {
        if let Some(ty) = env.lookup(name) {
            return is_bool(ty);
        }
        return groups
            .get(&name.to_ascii_lowercase())
            .is_some_and(is_bool);
    }
    infer_type(expr, env).is_some_and(|ty| is_bool(&ty))
}

fn is_bool(type_name: &TypeName) -> bool {
    !type_name.is_array && type_name.name.eq_ignore_ascii_case("bool")
}

fn unparsed_property_bodies(tokens: &[Token]) -> Vec<bool> {
    let mut ignored = vec![false; tokens.len()];
    let mut index = 0usize;
    while index < tokens.len() {
        if !matches!(tokens[index].kind, TokenKind::Keyword(Keyword::Property)) {
            index += 1;
            continue;
        }
        let mut header_end = index;
        while header_end < tokens.len()
            && !matches!(tokens[header_end].kind, TokenKind::Newline | TokenKind::Eof)
        {
            header_end += 1;
        }
        let auto = tokens[index..=header_end.min(tokens.len().saturating_sub(1))]
            .iter()
            .any(|token| {
                matches!(
                    token.kind,
                    TokenKind::Keyword(Keyword::Auto | Keyword::AutoReadOnly)
                )
            });
        if auto {
            index = header_end + 1;
            continue;
        }
        let mut end = header_end;
        while end < tokens.len()
            && !matches!(tokens[end].kind, TokenKind::Keyword(Keyword::EndProperty))
        {
            end += 1;
        }
        let ignore_from = header_end + 1;
        let ignore_to = end.min(tokens.len().saturating_sub(1));
        if ignore_from < tokens.len() {
            for slot in &mut ignored[ignore_from..=ignore_to] {
                *slot = true;
            }
        }
        index = end + 1;
    }
    ignored
}

fn collect_token_comparisons(tokens: &[Token], ignored: &[bool]) -> Vec<TokenComparison> {
    let mut comparisons = Vec::new();
    for (index, token) in tokens.iter().enumerate() {
        if ignored.get(index).copied().unwrap_or(false) {
            continue;
        }
        let op = match token.kind {
            TokenKind::Eq => BinaryOp::Eq,
            TokenKind::NotEq => BinaryOp::NotEq,
            _ => continue,
        };
        let left_start = scan_left(tokens, index);
        let (right_start, right_end) = scan_right(tokens, index);
        if left_start >= index || right_start >= right_end {
            continue;
        }
        let left = left_start..index;
        let right = right_start..right_end;
        let left_lit = literal_operand(tokens, &left);
        let right_lit = literal_operand(tokens, &right);
        if left_lit.is_none() && right_lit.is_none() {
            continue;
        }
        comparisons.push(TokenComparison {
            shape: Shape {
                op,
                left_lit,
                right_lit,
            },
            op_index: index,
            left,
            right,
        });
    }
    comparisons
}

fn scan_left(tokens: &[Token], op_index: usize) -> usize {
    let mut depth = 0i32;
    let mut index = op_index;
    let mut start = op_index;
    while index > 0 {
        index -= 1;
        match tokens[index].kind {
            TokenKind::RParen | TokenKind::RBracket => {
                depth += 1;
                start = index;
            }
            TokenKind::LParen | TokenKind::LBracket => {
                if depth == 0 {
                    break;
                }
                depth -= 1;
                start = index;
            }
            _ if depth > 0 => start = index,
            ref kind if is_left_boundary(kind) => break,
            _ => start = index,
        }
    }
    start
}

fn scan_right(tokens: &[Token], op_index: usize) -> (usize, usize) {
    let mut depth = 0i32;
    let start = op_index + 1;
    let mut index = start;
    while index < tokens.len() {
        match tokens[index].kind {
            TokenKind::LParen | TokenKind::LBracket => depth += 1,
            TokenKind::RParen | TokenKind::RBracket => {
                if depth == 0 {
                    break;
                }
                depth -= 1;
            }
            ref kind if depth == 0 && is_right_boundary(kind) => break,
            _ => {}
        }
        index += 1;
    }
    (start, index)
}

fn is_left_boundary(kind: &TokenKind) -> bool {
    matches!(
        kind,
        TokenKind::AndAnd
            | TokenKind::OrOr
            | TokenKind::Newline
            | TokenKind::Eof
            | TokenKind::Comma
            | TokenKind::Assign
            | TokenKind::PlusAssign
            | TokenKind::MinusAssign
            | TokenKind::StarAssign
            | TokenKind::SlashAssign
            | TokenKind::PercentAssign
            | TokenKind::CommentAnnotation(_)
    ) || is_statement_keyword(kind)
}

fn is_right_boundary(kind: &TokenKind) -> bool {
    matches!(kind, TokenKind::Eq | TokenKind::NotEq) || is_left_boundary(kind)
}

fn is_statement_keyword(kind: &TokenKind) -> bool {
    let TokenKind::Keyword(keyword) = kind else {
        return false;
    };
    !matches!(
        keyword,
        Keyword::True
            | Keyword::False
            | Keyword::None
            | Keyword::Self_
            | Keyword::Parent
            | Keyword::New
            | Keyword::As
            | Keyword::Is
            | Keyword::Length
            | Keyword::Hidden
            | Keyword::Conditional
    )
}

fn literal_operand(tokens: &[Token], range: &std::ops::Range<usize>) -> Option<bool> {
    let mut start = range.start;
    let mut end = range.end;
    loop {
        if start >= end {
            return None;
        }
        if matches!(tokens[start].kind, TokenKind::LParen)
            && matches!(tokens[end - 1].kind, TokenKind::RParen)
            && outer_parens_wrap(tokens, start, end)
        {
            start += 1;
            end -= 1;
            continue;
        }
        break;
    }
    if end - start != 1 {
        return None;
    }
    match tokens[start].kind {
        TokenKind::Keyword(Keyword::True) => Some(true),
        TokenKind::Keyword(Keyword::False) => Some(false),
        _ => None,
    }
}

fn outer_parens_wrap(tokens: &[Token], start: usize, end: usize) -> bool {
    if start >= end {
        return false;
    }
    let mut depth = 0i32;
    for (index, token) in tokens.iter().enumerate().take(end).skip(start) {
        match token.kind {
            TokenKind::LParen => depth += 1,
            TokenKind::RParen => {
                depth -= 1;
                if depth == 0 {
                    return index + 1 == end;
                }
            }
            _ => {}
        }
    }
    false
}

fn comparison_span(
    source: &str,
    line_starts: &[usize],
    tokens: &[Token],
    token: &TokenComparison,
) -> Option<(usize, usize)> {
    let start_index = token.left.start;
    let end_index = token.right.end.checked_sub(1)?;
    let start = token_offset(line_starts, tokens.get(start_index)?);
    let end_token_start = token_offset(line_starts, tokens.get(end_index)?);
    let end = lexeme_end(source, end_token_start);
    (start < end).then_some((start, end))
}

fn token_slice(
    source: &str,
    line_starts: &[usize],
    tokens: &[Token],
    range: &std::ops::Range<usize>,
) -> String {
    if range.start >= range.end {
        return String::new();
    }
    let Some(first) = tokens.get(range.start) else {
        return String::new();
    };
    let Some(last) = tokens.get(range.end - 1) else {
        return String::new();
    };
    let start = token_offset(line_starts, first);
    let end = lexeme_end(source, token_offset(line_starts, last));
    source.get(start..end).unwrap_or("").to_string()
}

fn token_offset(line_starts: &[usize], token: &Token) -> usize {
    line_starts
        .get(token.line.saturating_sub(1))
        .copied()
        .unwrap_or(0)
        + token.col.saturating_sub(1)
}

fn lexeme_end(source: &str, start: usize) -> usize {
    let bytes = source.as_bytes();
    if start >= bytes.len() {
        return start;
    }
    match bytes[start] {
        b'"' => string_end(bytes, start),
        b'0'..=b'9' => number_end(bytes, start),
        b'_' | b'a'..=b'z' | b'A'..=b'Z' => word_end(bytes, start),
        b'&' | b'|' => (start + 2).min(bytes.len()),
        b'=' | b'!' | b'>' | b'<' | b'+' | b'-' | b'*' | b'/' | b'%' => {
            if bytes.get(start + 1) == Some(&b'=') {
                start + 2
            } else {
                start + 1
            }
        }
        _ => start + 1,
    }
}

fn string_end(bytes: &[u8], start: usize) -> usize {
    let mut index = start + 1;
    while index < bytes.len() {
        match bytes[index] {
            b'\\' => index += 2,
            b'"' => return index + 1,
            b'\n' => return index,
            _ => index += 1,
        }
    }
    bytes.len()
}

fn number_end(bytes: &[u8], start: usize) -> usize {
    let mut index = start + 1;
    if bytes[start] == b'0' && matches!(bytes.get(index), Some(b'x' | b'X')) {
        index += 1;
        while index < bytes.len() && bytes[index].is_ascii_hexdigit() {
            index += 1;
        }
        return index;
    }
    while index < bytes.len() && bytes[index].is_ascii_digit() {
        index += 1;
    }
    if bytes.get(index) == Some(&b'.') && bytes.get(index + 1).is_some_and(u8::is_ascii_digit) {
        index += 1;
        while index < bytes.len() && bytes[index].is_ascii_digit() {
            index += 1;
        }
    }
    index
}

fn word_end(bytes: &[u8], start: usize) -> usize {
    let mut index = start + 1;
    while index < bytes.len() && (bytes[index] == b'_' || bytes[index].is_ascii_alphanumeric()) {
        index += 1;
    }
    index
}

fn innermost_hits(hits: &[Hit]) -> Vec<&Hit> {
    hits.iter()
        .filter(|hit| {
            !hits.iter().any(|other| {
                other.start >= hit.start
                    && other.end <= hit.end
                    && (other.start != hit.start || other.end != hit.end)
            })
        })
        .collect()
}

fn apply_edits(source: &str, hits: &[&Hit]) -> String {
    let mut ordered: Vec<&Hit> = hits.to_vec();
    ordered.sort_by_key(|hit| std::cmp::Reverse(hit.start));
    let mut repaired = source.to_string();
    for hit in ordered {
        if hit.end > repaired.len() || hit.start > hit.end {
            continue;
        }
        repaired.replace_range(hit.start..hit.end, &hit.replacement);
    }
    repaired
}

#[cfg(test)]
#[path = "boolean_simplification_tests.rs"]
mod tests;
