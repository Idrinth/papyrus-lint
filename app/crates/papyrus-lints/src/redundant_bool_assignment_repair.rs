//! Byte-span helpers used by [`super::repair`].
use papyrus_parser::ast::{BinaryOp, Expr, Literal, UnaryOp};
use papyrus_parser::token::{Keyword, Token, TokenKind};

use crate::token_walk::line_starts;

use super::{Classified, Hit};

pub(super) fn build_hit(classified: &Classified<'_>, source: &str, tokens: &[Token]) -> Option<Hit> {
    let starts = line_starts(source);
    let end_line = end_if_line(tokens, classified.if_line)?;
    let start = *starts.get(classified.if_line.saturating_sub(1))?;
    let end = if end_line < starts.len() {
        *starts.get(end_line).unwrap_or(&source.len())
    } else {
        source.len()
    };
    if start >= end || end > source.len() {
        return None;
    }
    let indent = line_indent(source, start);
    let target = target_text(source, &starts, tokens, classified.if_line, classified.target)?;
    let condition = condition_text(source, &starts, tokens, classified.if_line)?;
    let rhs = build_rhs(&condition, classified.inverted, classified.condition);
    let replacement = format!("{indent}{target} = {rhs}\n");
    Some(Hit {
        if_line: classified.if_line,
        start,
        end,
        replacement,
    })
}

pub(super) fn end_if_line(tokens: &[Token], if_line: usize) -> Option<usize> {
    let start = tokens.iter().position(|token| {
        token.line == if_line && matches!(token.kind, TokenKind::Keyword(Keyword::If))
    })?;
    let mut depth = 0i32;
    for token in &tokens[start..] {
        match token.kind {
            TokenKind::Keyword(Keyword::If) => depth += 1,
            TokenKind::Keyword(Keyword::EndIf) => {
                depth -= 1;
                if depth == 0 {
                    return Some(token.line);
                }
            }
            _ => {}
        }
    }
    None
}

pub(super) fn line_indent(source: &str, line_start: usize) -> String {
    let rest = &source[line_start..];
    let end = rest
        .char_indices()
        .find(|(_, ch)| *ch != ' ' && *ch != '\t')
        .map(|(index, _)| index)
        .unwrap_or(rest.len());
    rest[..end].to_string()
}

pub(super) fn condition_text(
    source: &str,
    starts: &[usize],
    tokens: &[Token],
    if_line: usize,
) -> Option<String> {
    let if_index = tokens.iter().position(|token| {
        token.line == if_line && matches!(token.kind, TokenKind::Keyword(Keyword::If))
    })?;
    let first = tokens.get(if_index + 1)?;
    if first.line != if_line {
        return None;
    }
    let mut last_index = if_index + 1;
    for (index, token) in tokens.iter().enumerate().skip(if_index + 1) {
        if token.line != if_line
            || matches!(
                token.kind,
                TokenKind::Newline | TokenKind::Eof | TokenKind::CommentAnnotation(_)
            )
        {
            break;
        }
        last_index = index;
    }
    let start = token_offset(starts, first);
    let end = lexeme_end(source, token_offset(starts, &tokens[last_index]));
    source.get(start..end).map(|text| text.trim().to_string())
}

pub(super) fn target_text(
    source: &str,
    starts: &[usize],
    tokens: &[Token],
    if_line: usize,
    target: &Expr,
) -> Option<String> {
    let assign_index = tokens
        .iter()
        .position(|token| token.line > if_line && matches!(token.kind, TokenKind::Assign))?;
    let assign_line = tokens[assign_index].line;
    let mut first = None;
    let mut last_before = None;
    for (index, token) in tokens.iter().enumerate() {
        if token.line != assign_line {
            if first.is_some() {
                break;
            }
            continue;
        }
        if matches!(token.kind, TokenKind::Assign) {
            break;
        }
        if first.is_none() {
            first = Some(index);
        }
        last_before = Some(index);
    }
    let first = first?;
    let last = last_before?;
    let start = token_offset(starts, &tokens[first]);
    let end = lexeme_end(source, token_offset(starts, &tokens[last]));
    let text = source.get(start..end)?.trim().to_string();
    if text.is_empty() {
        return render_target(target);
    }
    Some(text)
}

pub(super) fn render_target(expr: &Expr) -> Option<String> {
    match expr {
        Expr::Identifier(name) => Some(name.clone()),
        Expr::Self_ => Some("Self".to_string()),
        Expr::Member { object, property } => {
            Some(format!("{}.{}", render_target(object)?, property))
        }
        Expr::Index { object, index } => {
            let object = render_target(object)?;
            let index = match index.as_ref() {
                Expr::Identifier(name) => name.clone(),
                Expr::Literal(Literal::Int { value, .. }) => value.to_string(),
                _ => return None,
            };
            Some(format!("{object}[{index}]"))
        }
        _ => None,
    }
}

pub(super) fn build_rhs(condition: &str, inverted: bool, ast: &Expr) -> String {
    let trimmed = condition.trim();
    if !inverted {
        return trimmed.to_string();
    }
    if let Expr::Unary {
        op: UnaryOp::Not, ..
    } = ast
    {
        if let Some(inner) = strip_one_not(trimmed) {
            return strip_outer_parens(inner.trim()).to_string();
        }
    }
    if let Some(inverted_cmp) = invert_simple_comparison(trimmed, ast) {
        return inverted_cmp;
    }
    render_negated(trimmed)
}

pub(super) fn invert_simple_comparison(text: &str, ast: &Expr) -> Option<String> {
    let Expr::Binary { op, .. } = ast else {
        return None;
    };
    let (original, replacement) = match op {
        BinaryOp::Eq => ("==", "!="),
        BinaryOp::NotEq => ("!=", "=="),
        BinaryOp::Gt => (">", "<="),
        BinaryOp::Lt => ("<", ">="),
        BinaryOp::GtEq => (">=", "<"),
        BinaryOp::LtEq => ("<=", ">"),
        _ => return None,
    };
    let bytes = text.as_bytes();
    let mut depth = 0i32;
    let mut index = 0usize;
    let mut found: Option<(usize, usize)> = None;
    while index < bytes.len() {
        match bytes[index] {
            b'(' | b'[' => {
                depth += 1;
                index += 1;
            }
            b')' | b']' => {
                depth -= 1;
                index += 1;
            }
            _ if depth == 0 && text[index..].starts_with(original) => {
                let end = index + original.len();
                let before_ok = index == 0 || !is_ident_byte(bytes[index - 1]);
                let after_ok = end >= bytes.len() || !is_ident_byte(bytes[end]);
                let not_prefix_of_ge_le = !(original.len() == 1
                    && matches!(bytes.get(end), Some(b'=')));
                if before_ok && after_ok && not_prefix_of_ge_le {
                    if found.is_some() {
                        return None;
                    }
                    found = Some((index, end));
                }
                index = end;
            }
            _ => index += 1,
        }
    }
    let (start, end) = found?;
    Some(format!("{}{}{}", &text[..start], replacement, &text[end..]))
}

pub(super) fn is_ident_byte(byte: u8) -> bool {
    byte == b'_' || byte.is_ascii_alphanumeric()
}

pub(super) fn render_negated(text: &str) -> String {
    let current = strip_outer_parens(text.trim());
    if let Some(inner) = strip_one_not(current) {
        return strip_outer_parens(inner.trim()).to_string();
    }
    if needs_parens(current) {
        format!("! ({current})")
    } else {
        format!("! {current}")
    }
}

pub(super) fn strip_outer_parens(text: &str) -> &str {
    let text = text.trim();
    if !(text.starts_with('(') && text.ends_with(')')) {
        return text;
    }
    let bytes = text.as_bytes();
    let mut depth = 0i32;
    for (index, byte) in bytes.iter().enumerate() {
        match byte {
            b'(' => depth += 1,
            b')' => {
                depth -= 1;
                if depth == 0 {
                    return if index == bytes.len() - 1 {
                        text[1..index].trim()
                    } else {
                        text
                    };
                }
            }
            _ => {}
        }
    }
    text
}

pub(super) fn strip_one_not(text: &str) -> Option<&str> {
    let rest = text.trim().strip_prefix('!')?;
    if rest.starts_with('=') {
        return None;
    }
    Some(rest)
}

pub(super) fn needs_parens(text: &str) -> bool {
    let bytes = text.as_bytes();
    let mut depth = 0i32;
    let mut index = 0usize;
    while index < bytes.len() {
        match bytes[index] {
            b'(' | b'[' => {
                depth += 1;
                index += 1;
            }
            b')' | b']' => {
                depth -= 1;
                index += 1;
            }
            b'&' | b'|' | b'=' | b'>' | b'<' | b'+' | b'*' | b'/' | b'%' | b'-' if depth == 0 => {
                return true;
            }
            b'!' if depth == 0 && bytes.get(index + 1) == Some(&b'=') => return true,
            _ => index += 1,
        }
    }
    false
}

pub(super) fn apply_edits(source: &str, hits: &[Hit]) -> String {
    let mut repaired = source.to_string();
    let mut last_start = usize::MAX;
    for hit in hits {
        if hit.end > repaired.len() || hit.start > hit.end || hit.end > last_start {
            continue;
        }
        repaired.replace_range(hit.start..hit.end, &hit.replacement);
        last_start = hit.start;
    }
    repaired
}

pub(super) fn token_offset(line_starts: &[usize], token: &Token) -> usize {
    line_starts
        .get(token.line.saturating_sub(1))
        .copied()
        .unwrap_or(0)
        + token.col.saturating_sub(1)
}

pub(super) fn lexeme_end(source: &str, start: usize) -> usize {
    let bytes = source.as_bytes();
    if start >= bytes.len() {
        return start;
    }
    match bytes[start] {
        b'"' => {
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
        b'0'..=b'9' => {
            let mut index = start + 1;
            while index < bytes.len() && bytes[index].is_ascii_digit() {
                index += 1;
            }
            index
        }
        b'_' | b'a'..=b'z' | b'A'..=b'Z' => {
            let mut index = start + 1;
            while index < bytes.len()
                && (bytes[index] == b'_' || bytes[index].is_ascii_alphanumeric())
            {
                index += 1;
            }
            index
        }
        b'=' | b'!' | b'>' | b'<' | b'+' | b'-' | b'*' | b'/' | b'%'
            if bytes.get(start + 1) == Some(&b'=') =>
        {
            start + 2
        }
        b'&' | b'|' => (start + 2).min(bytes.len()),
        _ => start + 1,
    }
}
