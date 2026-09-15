//! Flags a `SetValue` call on a `GlobalVariable`-like receiver whose sole
//! argument adds something to that very same receiver's own `GetValue()`
//! (in either operand order):
//!
//! ```papyrus
//! gv.SetValue(gv.GetValue() + x)
//! gv.SetValue(x + gv.GetValue())
//! ```
//!
//! `GlobalVariable` exposes that exact operation as a single native call:
//! `Mod(afValue)` adds `afValue` to the global's current value (and returns
//! the new value), so either call above does the same thing as `gv.Mod(x)`
//! with one native call and one addition instead of two native calls and an
//! addition.
//!
//! Like the "Slow function usage" lint (`slow_functions`), a call's receiver
//! can't generally be resolved back to a `GlobalVariable`-typed script (the
//! lexer/parser have no type/symbol resolution), so this matches
//! structurally instead: a `SetValue` call standing alone as its own
//! statement, taking exactly one argument that's a top-level `+` between
//! `<receiver>.GetValue()` and some other expression, where `<receiver>` is
//! the exact same receiver (see [`receiver_key`]) the `SetValue` call itself
//! targets. Anything less direct — a different operator, `SetValueInt`, a
//! `GetValue()` nested deeper than the addition's own two operands, a
//! receiver that isn't a simple identifier/`Self`/member chain, ... — is
//! left unflagged rather than guessed at.
//!
//! [`repair`] rewrites each flagged call in place, replacing `SetValue(...)`
//! with `Mod(x)` and leaving the receiver qualifier (and everything else in
//! the file) untouched: the replacement only spans from the `SetValue`
//! identifier itself through the call's closing paren, so it doesn't need to
//! reconstruct the receiver's own source text. `x`'s own source text is
//! recovered by locating the argument's top-level `+` token (the last one at
//! paren/bracket depth 0 — the same one Papyrus's left-associative parsing
//! would treat as the outermost operator) and slicing whichever side isn't
//! the `GetValue()` call, so `x` keeps its original formatting verbatim.

use papyrus_parser::ast::{BinaryOp, Expr, FunctionDecl, IfBranch, Script, Stmt};
use papyrus_parser::token::{Token, TokenKind};

use crate::Diagnostic;

/// This lint's [`Diagnostic::rule`] id, for `@disable` line comments.
pub const RULE: &str = "global-variable-increment";

/// Checks every `SetValue` call in `source` for the `SetValue(GetValue() +
/// x)`/`SetValue(x + GetValue())` pattern described above.
pub fn check(source: &str) -> Vec<Diagnostic> {
    let Ok(script) = papyrus_parser::parse(source) else {
        return Vec::new();
    };

    let mut diagnostics = Vec::new();
    for function in all_functions(&script) {
        check_body(&function.body, &mut diagnostics);
    }
    diagnostics
}

fn all_functions(script: &Script) -> impl Iterator<Item = &FunctionDecl> {
    script.functions.iter().chain(
        script
            .states
            .iter()
            .flat_map(|state| state.functions.iter()),
    )
}

/// Rewrites every `SetValue(GetValue() + x)`/`SetValue(x + GetValue())` call
/// [`check`] would flag into `Mod(x)`. See the module docs for how each
/// edit's span and `x`'s source text are recovered.
pub fn repair(source: &str) -> String {
    let (Ok(script), Ok(tokens)) = (
        papyrus_parser::parse(source),
        papyrus_parser::tokenize(source),
    ) else {
        return source.to_string();
    };
    let line_starts = line_starts(source);

    let mut edits = Vec::new();
    for function in all_functions(&script) {
        collect_edits(&function.body, &tokens, &line_starts, source, &mut edits);
    }
    edits.sort_by(|a, b| b.0.cmp(&a.0));

    let mut repaired = source.to_string();
    for (start, end, replacement) in edits {
        repaired.replace_range(start..end, &replacement);
    }
    repaired
}

fn collect_edits(
    body: &[Stmt],
    tokens: &[Token],
    line_starts: &[usize],
    source: &str,
    edits: &mut Vec<(usize, usize, String)>,
) {
    for stmt in body {
        match stmt {
            Stmt::If {
                branches,
                else_body,
                ..
            } => {
                for IfBranch { body, .. } in branches {
                    collect_edits(body, tokens, line_starts, source, edits);
                }
                collect_edits(else_body, tokens, line_starts, source, edits);
            }
            Stmt::While { body, .. } => collect_edits(body, tokens, line_starts, source, edits),
            Stmt::Expr { value, .. } => {
                if let Some(edit) = build_edit(value, tokens, line_starts, source) {
                    edits.push(edit);
                }
            }
            Stmt::VarDecl(_) | Stmt::Assign { .. } | Stmt::Return { .. } => {}
        }
    }
}

/// Builds the `(start, end, replacement)` edit for `expr` if it's a
/// `SetValue(GetValue() + x)`/`SetValue(x + GetValue())` call matching
/// [`check_setvalue_call`]'s own pattern.
fn build_edit(
    expr: &Expr,
    tokens: &[Token],
    line_starts: &[usize],
    source: &str,
) -> Option<(usize, usize, String)> {
    let Expr::Call {
        callee,
        args,
        line,
        col,
    } = expr
    else {
        return None;
    };
    if args.len() != 1 {
        return None;
    }
    let Expr::Member { object, property } = callee.as_ref() else {
        return None;
    };
    if !property.eq_ignore_ascii_case("SetValue") {
        return None;
    }
    let receiver = receiver_key(object)?;
    let Expr::Binary {
        left,
        op: BinaryOp::Add,
        right,
    } = &args[0]
    else {
        return None;
    };
    let left_is_getvalue = is_getvalue_on(left, &receiver);
    let right_is_getvalue = is_getvalue_on(right, &receiver);
    if !left_is_getvalue && !right_is_getvalue {
        return None;
    }

    // `line`/`col` are the position of the call's own opening paren (see
    // `Parser::parse_postfix`), so this is unambiguous even with more than
    // one `SetValue` call on the same line.
    let open_index = tokens.iter().position(|token| {
        token.line == *line && token.col == *col && token.kind == TokenKind::LParen
    })?;
    let close_index = matching_close_paren(tokens, open_index)?;
    let setvalue_index = open_index.checked_sub(1)?;

    let argument_start = token_offset(line_starts, &tokens[open_index]) + 1;
    let close_offset = token_offset(line_starts, &tokens[close_index]);
    let setvalue_start = token_offset(line_starts, &tokens[setvalue_index]);

    // The last `+` at paren/bracket depth 0: for a left-associative chain of
    // `+`/`-` operators, that's always the one the top-level `Binary` above
    // was actually built from (see the module docs).
    let mut depth = 0i32;
    let mut plus_index = None;
    for (index, token) in tokens
        .iter()
        .enumerate()
        .take(close_index)
        .skip(open_index + 1)
    {
        match token.kind {
            TokenKind::LParen | TokenKind::LBracket => depth += 1,
            TokenKind::RParen | TokenKind::RBracket => depth -= 1,
            TokenKind::Plus if depth == 0 => plus_index = Some(index),
            _ => {}
        }
    }
    let plus_index = plus_index?;
    let plus_offset = token_offset(line_starts, &tokens[plus_index]);

    let left_text = source[argument_start..plus_offset].trim();
    let right_text = source[plus_offset + 1..close_offset].trim();
    let x_text = if left_is_getvalue {
        right_text
    } else {
        left_text
    };
    if x_text.is_empty() {
        return None;
    }

    Some((setvalue_start, close_offset + 1, format!("Mod({x_text})")))
}

fn matching_close_paren(tokens: &[Token], open_index: usize) -> Option<usize> {
    let mut depth = 0usize;
    for (index, token) in tokens.iter().enumerate().skip(open_index) {
        match token.kind {
            TokenKind::LParen => depth += 1,
            TokenKind::RParen => {
                depth -= 1;
                if depth == 0 {
                    return Some(index);
                }
            }
            _ => {}
        }
    }
    None
}

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

fn token_offset(line_starts: &[usize], token: &Token) -> usize {
    line_starts[token.line - 1] + token.col - 1
}

fn check_body(body: &[Stmt], diagnostics: &mut Vec<Diagnostic>) {
    for stmt in body {
        match stmt {
            Stmt::If {
                branches,
                else_body,
                ..
            } => {
                for IfBranch { body, .. } in branches {
                    check_body(body, diagnostics);
                }
                check_body(else_body, diagnostics);
            }
            Stmt::While { body, .. } => check_body(body, diagnostics),
            Stmt::Expr { value, line } => check_setvalue_call(value, *line, diagnostics),
            Stmt::VarDecl(_) | Stmt::Assign { .. } | Stmt::Return { .. } => {}
        }
    }
}

fn check_setvalue_call(expr: &Expr, line: usize, diagnostics: &mut Vec<Diagnostic>) {
    let Expr::Call { callee, args, .. } = expr else {
        return;
    };
    if args.len() != 1 {
        return;
    }
    let Expr::Member { object, property } = callee.as_ref() else {
        return;
    };
    if !property.eq_ignore_ascii_case("SetValue") {
        return;
    }
    let Some(receiver) = receiver_key(object) else {
        return;
    };
    let Expr::Binary {
        left,
        op: BinaryOp::Add,
        right,
    } = &args[0]
    else {
        return;
    };
    if !is_getvalue_on(left, &receiver) && !is_getvalue_on(right, &receiver) {
        return;
    }
    let Some(display) = receiver_display(object) else {
        return;
    };
    diagnostics.push(Diagnostic {
        line,
        column: 1,
        message: format!(
            "[info] {display}.SetValue({display}.GetValue() + x) is slower than necessary; use \
             `{display}.Mod(x)` instead, which adds x to the current value in one native call"
        ),
        rule: RULE,
    });
}

/// Whether `expr` is exactly `<receiver>.GetValue()` (no arguments) on the
/// receiver identified by `receiver` (see [`receiver_key`]).
fn is_getvalue_on(expr: &Expr, receiver: &str) -> bool {
    let Expr::Call { callee, args, .. } = expr else {
        return false;
    };
    if !args.is_empty() {
        return false;
    }
    let Expr::Member { object, property } = callee.as_ref() else {
        return false;
    };
    property.eq_ignore_ascii_case("GetValue") && receiver_key(object).as_deref() == Some(receiver)
}

/// A canonical, case-insensitive key for a "simple" receiver expression (an
/// identifier, `Self`, or a chain of member accesses built from those), used
/// to tell whether a `SetValue` call's receiver is the same one its
/// argument's `GetValue()` call reads from. Anything less direct (a call, an
/// index, a cast, ...) returns `None` rather than being guessed at.
fn receiver_key(expr: &Expr) -> Option<String> {
    match expr {
        Expr::Identifier(name) => Some(name.to_lowercase()),
        Expr::Self_ => Some("self".to_string()),
        Expr::Member { object, property } => Some(format!(
            "{}.{}",
            receiver_key(object)?,
            property.to_lowercase()
        )),
        _ => None,
    }
}

/// Like [`receiver_key`], but preserves the receiver's original casing for
/// use in a diagnostic message instead of comparison.
fn receiver_display(expr: &Expr) -> Option<String> {
    match expr {
        Expr::Identifier(name) => Some(name.clone()),
        Expr::Self_ => Some("Self".to_string()),
        Expr::Member { object, property } => {
            Some(format!("{}.{}", receiver_display(object)?, property))
        }
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn flags_getvalue_plus_other_operand() {
        let diagnostics = check(
            "ScriptName Example\n\nFunction Test(GlobalVariable gv, Float x)\n    gv.SetValue(gv.GetValue() + x)\nEndFunction\n",
        );

        assert_eq!(diagnostics.len(), 1);
        assert_eq!(diagnostics[0].line, 4);
        assert_eq!(diagnostics[0].column, 1);
        assert_eq!(diagnostics[0].rule, RULE);
        assert!(diagnostics[0].message.starts_with("[info]"));
        assert!(diagnostics[0].message.contains("gv.Mod(x)"));
    }

    #[test]
    fn flags_other_operand_plus_getvalue() {
        let diagnostics = check(
            "ScriptName Example\n\nFunction Test(GlobalVariable gv, Float x)\n    gv.SetValue(x + gv.GetValue())\nEndFunction\n",
        );

        assert_eq!(diagnostics.len(), 1);
    }

    #[test]
    fn flags_case_insensitively() {
        let diagnostics = check(
            "ScriptName Example\n\nFunction Test(GlobalVariable gv, Float x)\n    gv.setvalue(gv.getvalue() + x)\nEndFunction\n",
        );

        assert_eq!(diagnostics.len(), 1);
    }

    #[test]
    fn does_not_flag_setvalueint() {
        let diagnostics = check(
            "ScriptName Example\n\nFunction Test(GlobalVariable gv, Int x)\n    gv.SetValueInt(gv.GetValueInt() + x)\nEndFunction\n",
        );

        assert!(diagnostics.is_empty());
    }

    #[test]
    fn does_not_flag_a_different_operator() {
        let diagnostics = check(
            "ScriptName Example\n\nFunction Test(GlobalVariable gv, Float x)\n    gv.SetValue(gv.GetValue() - x)\nEndFunction\n",
        );

        assert!(diagnostics.is_empty());
    }

    #[test]
    fn does_not_flag_a_getvalue_call_on_a_different_receiver() {
        let diagnostics = check(
            "ScriptName Example\n\nFunction Test(GlobalVariable gv, GlobalVariable other)\n    gv.SetValue(other.GetValue() + 1.0)\nEndFunction\n",
        );

        assert!(diagnostics.is_empty());
    }

    #[test]
    fn does_not_flag_a_plain_setvalue_with_no_addition() {
        let diagnostics = check(
            "ScriptName Example\n\nFunction Test(GlobalVariable gv)\n    gv.SetValue(1.0)\nEndFunction\n",
        );

        assert!(diagnostics.is_empty());
    }

    #[test]
    fn does_not_flag_addition_of_two_unrelated_values() {
        let diagnostics = check(
            "ScriptName Example\n\nFunction Test(GlobalVariable gv, Float a, Float b)\n    gv.SetValue(a + b)\nEndFunction\n",
        );

        assert!(diagnostics.is_empty());
    }

    #[test]
    fn resolves_self_and_chained_member_receivers() {
        let diagnostics = check(
            "ScriptName Example\n\nFunction Test(SomeQuest quest, Float x)\n    Self.SetValue(Self.GetValue() + x)\n    quest.MyGlobal.SetValue(quest.MyGlobal.GetValue() + x)\nEndFunction\n",
        );

        assert_eq!(diagnostics.len(), 2);
        assert!(diagnostics.iter().any(|d| d.message.contains("Self.Mod")));
        assert!(diagnostics
            .iter()
            .any(|d| d.message.contains("quest.MyGlobal.Mod")));
    }

    #[test]
    fn does_not_flag_a_receiver_that_is_neither_an_identifier_self_nor_member_chain() {
        let diagnostics = check(
            "ScriptName Example\n\nFunction Test(GlobalVariable[] gvs, Int i, Float x)\n    gvs[i].SetValue(gvs[i].GetValue() + x)\nEndFunction\n",
        );

        assert!(diagnostics.is_empty());
    }

    #[test]
    fn checks_functions_declared_in_states_too() {
        let diagnostics = check(
            "ScriptName Example\n\nState Active\n    Function Test(GlobalVariable gv, Float x)\n        gv.SetValue(gv.GetValue() + x)\n    EndFunction\nEndState\n",
        );

        assert_eq!(diagnostics.len(), 1);
    }

    #[test]
    fn checks_nested_if_while_and_else_bodies() {
        let diagnostics = check(
            "ScriptName Example\n\nFunction Test(GlobalVariable gv, Float x, Bool cond)\n    While cond\n        If cond\n            gv.SetValue(gv.GetValue() + x)\n        Else\n            gv.SetValue(x + gv.GetValue())\n        EndIf\n    EndWhile\nEndFunction\n",
        );

        assert_eq!(diagnostics.len(), 2);
    }

    #[test]
    fn does_not_flag_a_call_with_more_than_one_argument() {
        let diagnostics = check(
            "ScriptName Example\n\nFunction Test(GlobalVariable gv, Float x)\n    gv.SetValue(gv.GetValue() + x, x)\nEndFunction\n",
        );

        assert!(diagnostics.is_empty());
    }

    #[test]
    fn does_not_flag_an_unqualified_setvalue_call() {
        let diagnostics = check(
            "ScriptName Example\n\nFunction Test(Float x)\n    SetValue(GetValue() + x)\nEndFunction\n",
        );

        assert!(diagnostics.is_empty());
    }

    #[test]
    fn does_not_crash_on_unparseable_source() {
        assert!(check("ScriptName Example\n\nFunction Test(\nEndFunction\n").is_empty());
    }

    #[test]
    fn receiver_key_and_display_reject_expressions_that_are_not_a_simple_chain() {
        let not_a_receiver = Expr::Literal(papyrus_parser::ast::Literal::int(1));
        assert_eq!(receiver_key(&not_a_receiver), None);
        assert_eq!(receiver_display(&not_a_receiver), None);
    }

    #[test]
    fn repairs_getvalue_plus_other_operand() {
        let repaired = repair(
            "ScriptName Example\n\nFunction Test(GlobalVariable gv, Float x)\n    gv.SetValue(gv.GetValue() + x)\nEndFunction\n",
        );

        assert_eq!(
            repaired,
            "ScriptName Example\n\nFunction Test(GlobalVariable gv, Float x)\n    gv.Mod(x)\nEndFunction\n"
        );
        assert!(check(&repaired).is_empty());
    }

    #[test]
    fn repairs_other_operand_plus_getvalue() {
        let repaired = repair(
            "ScriptName Example\n\nFunction Test(GlobalVariable gv, Float x)\n    gv.SetValue(x + gv.GetValue())\nEndFunction\n",
        );

        assert_eq!(
            repaired,
            "ScriptName Example\n\nFunction Test(GlobalVariable gv, Float x)\n    gv.Mod(x)\nEndFunction\n"
        );
    }

    #[test]
    fn repairs_case_insensitively_and_preserves_the_receivers_own_casing() {
        let repaired = repair(
            "ScriptName Example\n\nFunction Test(GlobalVariable gv, Float x)\n    gv.setvalue(gv.getvalue() + x)\nEndFunction\n",
        );

        assert_eq!(
            repaired,
            "ScriptName Example\n\nFunction Test(GlobalVariable gv, Float x)\n    gv.Mod(x)\nEndFunction\n"
        );
    }

    #[test]
    fn repairs_self_and_chained_member_receivers() {
        let repaired = repair(
            "ScriptName Example\n\nFunction Test(SomeQuest quest, Float x)\n    Self.SetValue(Self.GetValue() + x)\n    quest.MyGlobal.SetValue(quest.MyGlobal.GetValue() + x)\nEndFunction\n",
        );

        assert_eq!(
            repaired,
            "ScriptName Example\n\nFunction Test(SomeQuest quest, Float x)\n    Self.Mod(x)\n    quest.MyGlobal.Mod(x)\nEndFunction\n"
        );
    }

    #[test]
    fn repairs_a_complex_addend_and_keeps_its_own_spacing() {
        let repaired = repair(
            "ScriptName Example\n\nFunction Test(GlobalVariable gv)\n    gv.SetValue(gv.GetValue() + GetAmount(1, 2))\nEndFunction\n",
        );

        assert_eq!(
            repaired,
            "ScriptName Example\n\nFunction Test(GlobalVariable gv)\n    gv.Mod(GetAmount(1, 2))\nEndFunction\n"
        );
    }

    #[test]
    fn repairs_multiple_calls_and_nested_bodies() {
        let repaired = repair(
            "ScriptName Example\n\nFunction Test(GlobalVariable gv, Float x, Bool cond)\n    While cond\n        If cond\n            gv.SetValue(gv.GetValue() + x)\n        Else\n            gv.SetValue(x + gv.GetValue())\n        EndIf\n    EndWhile\nEndFunction\n",
        );

        assert_eq!(
            repaired,
            "ScriptName Example\n\nFunction Test(GlobalVariable gv, Float x, Bool cond)\n    While cond\n        If cond\n            gv.Mod(x)\n        Else\n            gv.Mod(x)\n        EndIf\n    EndWhile\nEndFunction\n"
        );
    }

    #[test]
    fn repair_leaves_setvalueint_untouched() {
        let source =
            "ScriptName Example\n\nFunction Test(GlobalVariable gv, Int x)\n    gv.SetValueInt(gv.GetValueInt() + x)\nEndFunction\n";

        assert_eq!(repair(source), source);
    }

    #[test]
    fn repair_leaves_a_different_operator_untouched() {
        let source =
            "ScriptName Example\n\nFunction Test(GlobalVariable gv, Float x)\n    gv.SetValue(gv.GetValue() - x)\nEndFunction\n";

        assert_eq!(repair(source), source);
    }

    #[test]
    fn repair_leaves_a_call_with_more_than_one_argument_untouched() {
        let source =
            "ScriptName Example\n\nFunction Test(GlobalVariable gv, Float x)\n    gv.SetValue(gv.GetValue() + x, x)\nEndFunction\n";

        assert_eq!(repair(source), source);
    }

    #[test]
    fn repair_leaves_unrelated_lines_and_other_statements_untouched() {
        let source = "ScriptName Example\n\nFunction Test(GlobalVariable gv, GlobalVariable other, Float x)\n    gv.SetValue(1.0)\n    other.SetValue(gv.GetValue() + x)\nEndFunction\n";

        assert_eq!(repair(source), source);
    }

    #[test]
    fn does_not_crash_repairing_unparseable_source() {
        let source = "ScriptName Example\n\nFunction Test(\nEndFunction\n";
        assert_eq!(repair(source), source);
    }
}
