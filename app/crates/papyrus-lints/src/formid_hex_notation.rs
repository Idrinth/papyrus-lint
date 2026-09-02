//! Flags a FormID literal that isn't written in hexadecimal notation when
//! it's compared against a `GetFormID()` call, or passed as the FormID
//! argument to `Game.GetFormFromFile`. Hexadecimal is the convention used
//! everywhere else a FormID appears (the Creation Kit, xEdit, mod
//! documentation), and a stray decimal literal is easy to mistype or
//! overlook next to genuinely hex-written ones.
//!
//! Like [`crate::forbidden_functions`], this works on lexer tokens rather
//! than the parsed AST, since the calls it looks for (`GetFormID()`
//! comparisons, `Game.GetFormFromFile` arguments) are easier to match as a
//! flat token sequence than to walk out of an `Expr` tree. Each
//! `TokenKind::IntLiteral` token carries the [`IntFormat`] it was written
//! with, the same distinction `papyrus_parser::ast::Literal::Int` carries
//! into the parsed AST, so this lint reads it straight off the token
//! instead of re-scanning the literal's source text. Only a literal
//! directly adjacent to the comparison operator or the call's argument
//! list is checked; one reached indirectly through a variable assigned
//! earlier is left unflagged rather than guessed at.

use papyrus_parser::lexer::Lexer;
use papyrus_parser::token::{IntFormat, Keyword, Token, TokenKind};

use crate::Diagnostic;

/// This lint's [`Diagnostic::rule`] id, for `@disable` line comments.
pub const RULE: &str = "formid-hex-notation";

/// Checks `source` for a non-hexadecimal FormID literal compared against
/// `GetFormID()` or passed to `Game.GetFormFromFile`.
pub fn check(source: &str) -> Vec<Diagnostic> {
    let Ok(tokens) = Lexer::new(source).tokenize() else {
        return Vec::new();
    };

    let mut diagnostics = Vec::new();
    for i in 0..tokens.len() {
        if is_get_form_id_call(&tokens, i) {
            check_get_form_id_comparison(&tokens, i, &mut diagnostics);
        }
        if is_game_get_form_from_file_call(&tokens, i) {
            check_get_form_from_file_argument(&tokens, i, &mut diagnostics);
        }
    }
    diagnostics
}

fn is_identifier(token: &Token, name: &str) -> bool {
    matches!(&token.kind, TokenKind::Identifier(actual) if actual.eq_ignore_ascii_case(name))
}

fn is_comparison(kind: &TokenKind) -> bool {
    matches!(
        kind,
        TokenKind::Eq
            | TokenKind::NotEq
            | TokenKind::Gt
            | TokenKind::Lt
            | TokenKind::GtEq
            | TokenKind::LtEq
    )
}

/// Whether `tokens[index]` starts a no-argument `GetFormID()` call.
fn is_get_form_id_call(tokens: &[Token], index: usize) -> bool {
    is_identifier(&tokens[index], "GetFormID")
        && matches!(
            tokens.get(index + 1).map(|t| &t.kind),
            Some(TokenKind::LParen)
        )
        && matches!(
            tokens.get(index + 2).map(|t| &t.kind),
            Some(TokenKind::RParen)
        )
}

/// Whether `tokens[index]` starts a `GetFormFromFile(...)` call qualified
/// by the literal `Game` singleton, the same way [`crate::forbidden_functions`]
/// only matches a `global` rule's function through its literal script
/// name (`Game` is never subclassed, so this is the only way the call
/// resolves to it).
fn is_game_get_form_from_file_call(tokens: &[Token], index: usize) -> bool {
    if !is_identifier(&tokens[index], "GetFormFromFile") {
        return false;
    }
    if !matches!(
        tokens.get(index + 1).map(|t| &t.kind),
        Some(TokenKind::LParen)
    ) {
        return false;
    }
    if index < 2 || !matches!(tokens[index - 1].kind, TokenKind::Dot) {
        return false;
    }
    is_identifier(&tokens[index - 2], "Game")
}

/// Flags a `GetFormID()` call directly compared (`==`, `!=`, `<`, `<=`,
/// `>`, `>=`) against a non-hexadecimal integer literal, checking both
/// `GetFormID() == 0x...` and `0x... == GetFormID()` orderings.
fn check_get_form_id_comparison(
    tokens: &[Token],
    call_index: usize,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let call_end = call_index + 2;
    const CONTEXT: &str = "compared against GetFormID()";

    if let Some(op) = tokens.get(call_end + 1) {
        if is_comparison(&op.kind) {
            let mut literal_index = call_end + 2;
            if matches!(
                tokens.get(literal_index).map(|t| &t.kind),
                Some(TokenKind::Minus)
            ) {
                literal_index += 1;
            }
            if let Some(literal) = tokens.get(literal_index) {
                flag_if_decimal(literal, CONTEXT, diagnostics);
            }
        }
    }

    let receiver_start = skip_receiver_backward(tokens, call_index);
    if receiver_start >= 2 {
        if let Some(op) = tokens.get(receiver_start - 1) {
            if is_comparison(&op.kind) {
                if let Some(literal) = tokens.get(receiver_start - 2) {
                    flag_if_decimal(literal, CONTEXT, diagnostics);
                }
            }
        }
    }
}

/// Walks backward from `call_index` (the `GetFormID` identifier) over the
/// member-access chain that calls it (`akActor.GetFormID`, `Self.GetFormID`,
/// a bare `GetFormID`, ...), returning the index the chain starts at.
fn skip_receiver_backward(tokens: &[Token], call_index: usize) -> usize {
    let mut index = call_index;
    loop {
        if index == 0 {
            return index;
        }
        match &tokens[index - 1].kind {
            TokenKind::Dot | TokenKind::Identifier(_) => index -= 1,
            TokenKind::Keyword(Keyword::Self_) | TokenKind::Keyword(Keyword::Parent) => {
                return index - 1;
            }
            _ => return index,
        }
    }
}

/// Flags the FormID argument passed to a qualified `Game.GetFormFromFile`
/// call, whether passed positionally or by Papyrus's named-argument syntax
/// (`Game.GetFormFromFile(auiFormID = ...)`). Only flags when the literal
/// is the entire argument, not part of a larger expression this lint can't
/// interpret.
fn check_get_form_from_file_argument(
    tokens: &[Token],
    call_index: usize,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let mut index = call_index + 2; // past the identifier and its `(`
    if let (Some(name), Some(assign)) = (tokens.get(index), tokens.get(index + 1)) {
        if matches!(name.kind, TokenKind::Identifier(_)) && matches!(assign.kind, TokenKind::Assign)
        {
            index += 2;
        }
    }
    if matches!(tokens.get(index).map(|t| &t.kind), Some(TokenKind::Minus)) {
        index += 1;
    }

    let Some(literal) = tokens.get(index) else {
        return;
    };
    if !matches!(
        tokens.get(index + 1).map(|t| &t.kind),
        Some(TokenKind::Comma) | Some(TokenKind::RParen)
    ) {
        return;
    }
    flag_if_decimal(literal, "passed to Game.GetFormFromFile", diagnostics);
}

fn flag_if_decimal(literal: &Token, context: &str, diagnostics: &mut Vec<Diagnostic>) {
    let TokenKind::IntLiteral(value, format) = literal.kind else {
        return;
    };
    if format == IntFormat::Hexadecimal {
        return;
    }
    diagnostics.push(Diagnostic {
        line: literal.line,
        column: literal.col,
        message: format!(
            "[warning] FormID {context} is written in decimal ({value}) instead of \
             hexadecimal ({value:#X}); hexadecimal is the convention used everywhere else \
             FormIDs appear, and a decimal literal here is easy to mistype or overlook next \
             to correctly hex-written ones"
        ),
        rule: RULE,
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn flags_decimal_formid_compared_after_get_form_id() {
        let diagnostics = check(
            "ScriptName Example\n\nFunction Test(Actor akActor)\n    If akActor.GetFormID() == 76935\n    EndIf\nEndFunction\n",
        );

        assert_eq!(diagnostics.len(), 1);
        assert_eq!(diagnostics[0].line, 4);
        assert_eq!(diagnostics[0].rule, RULE);
        assert!(diagnostics[0].message.starts_with("[warning]"));
        assert!(diagnostics[0].message.contains("76935"));
        assert!(diagnostics[0].message.contains("0x12C87"));
    }

    #[test]
    fn flags_decimal_formid_compared_before_get_form_id() {
        let diagnostics = check(
            "ScriptName Example\n\nFunction Test(Actor akActor)\n    If 76935 == akActor.GetFormID()\n    EndIf\nEndFunction\n",
        );

        assert_eq!(diagnostics.len(), 1);
        assert_eq!(diagnostics[0].line, 4);
    }

    #[test]
    fn does_not_flag_hex_formid_in_either_order() {
        let diagnostics = check(
            "ScriptName Example\n\nFunction Test(Actor akActor)\n    If akActor.GetFormID() == 0x00012C87\n        If 0X12c87 == akActor.GetFormID()\n        EndIf\n    EndIf\nEndFunction\n",
        );

        assert!(diagnostics.is_empty());
    }

    #[test]
    fn flags_every_comparison_operator() {
        let source = "ScriptName Example\n\nFunction Test(Actor akActor)\n    If akActor.GetFormID() == 1\n    EndIf\n    If akActor.GetFormID() != 2\n    EndIf\n    If akActor.GetFormID() > 3\n    EndIf\n    If akActor.GetFormID() < 4\n    EndIf\n    If akActor.GetFormID() >= 5\n    EndIf\n    If akActor.GetFormID() <= 6\n    EndIf\nEndFunction\n";

        assert_eq!(check(source).len(), 6);
    }

    #[test]
    fn flags_unqualified_and_self_receivers() {
        let diagnostics = check(
            "ScriptName Example Extends Actor\n\nFunction Test()\n    If GetFormID() == 76935\n    EndIf\n    If Self.GetFormID() == 76935\n    EndIf\nEndFunction\n",
        );

        assert_eq!(diagnostics.len(), 2);
    }

    #[test]
    fn does_not_flag_get_form_id_compared_to_a_runtime_value() {
        let diagnostics = check(
            "ScriptName Example\n\nFunction Test(Actor akActor, Int aiOther)\n    If akActor.GetFormID() == aiOther\n    EndIf\n    If akActor.GetFormID() == GetOtherFormID()\n    EndIf\nEndFunction\n",
        );

        assert!(diagnostics.is_empty());
    }

    #[test]
    fn does_not_flag_two_get_form_id_calls_compared_to_each_other() {
        let diagnostics = check(
            "ScriptName Example\n\nFunction Test(Actor akA, Actor akB)\n    If akA.GetFormID() == akB.GetFormID()\n    EndIf\nEndFunction\n",
        );

        assert!(diagnostics.is_empty());
    }

    #[test]
    fn does_not_flag_get_form_id_used_outside_a_comparison() {
        let diagnostics = check(
            "ScriptName Example\n\nFunction Test(Actor akActor)\n    Int id = akActor.GetFormID()\nEndFunction\n",
        );

        assert!(diagnostics.is_empty());
    }

    #[test]
    fn flags_decimal_formid_passed_positionally_to_get_form_from_file() {
        let diagnostics = check(
            "ScriptName Example\n\nFunction Test()\n    Form theForm = Game.GetFormFromFile(76935, \"Skyrim.esm\")\nEndFunction\n",
        );

        assert_eq!(diagnostics.len(), 1);
        assert_eq!(diagnostics[0].line, 4);
        assert!(diagnostics[0].message.contains("Game.GetFormFromFile"));
    }

    #[test]
    fn flags_decimal_formid_passed_by_name_to_get_form_from_file() {
        let diagnostics = check(
            "ScriptName Example\n\nFunction Test()\n    Form theForm = Game.GetFormFromFile(auiFormID = 76935, asPluginName = \"Skyrim.esm\")\nEndFunction\n",
        );

        assert_eq!(diagnostics.len(), 1);
    }

    #[test]
    fn does_not_flag_hex_formid_passed_to_get_form_from_file() {
        let diagnostics = check(
            "ScriptName Example\n\nFunction Test()\n    Form theForm = Game.GetFormFromFile(0x00012C87, \"Skyrim.esm\")\nEndFunction\n",
        );

        assert!(diagnostics.is_empty());
    }

    #[test]
    fn does_not_flag_get_form_from_file_with_a_runtime_formid() {
        let diagnostics = check(
            "ScriptName Example\n\nFunction Test(Int aiFormID)\n    Form theForm = Game.GetFormFromFile(aiFormID, \"Skyrim.esm\")\nEndFunction\n",
        );

        assert!(diagnostics.is_empty());
    }

    #[test]
    fn does_not_flag_a_same_named_function_on_an_unrelated_script() {
        let diagnostics = check(
            "ScriptName Example\n\nFunction Test(MyScript akOther)\n    akOther.GetFormFromFile(76935, \"Skyrim.esm\")\nEndFunction\n",
        );

        assert!(diagnostics.is_empty());
    }

    #[test]
    fn does_not_crash_on_unparseable_source() {
        assert!(check("ScriptName Example\n\nFunction Test(\nEndFunction\n").is_empty());
    }
}
