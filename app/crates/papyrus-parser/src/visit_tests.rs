use super::*;
use crate::token::TokenKind;
use crate::{parse, tokenize};

struct ExprCounter(usize);

impl Visitor for ExprCounter {
    fn visit_expr(&mut self, expr: &Expr) {
        self.0 += 1;
        walk_expr(self, expr);
    }
}

struct TokenCounter(usize);

impl TokenVisitor for TokenCounter {
    fn visit_token(&mut self, token: &Token, index: usize, tokens: &[Token]) {
        let _ = (index, tokens);
        if matches!(token.kind, TokenKind::Identifier(_)) {
            self.0 += 1;
        }
    }
}

#[test]
fn ast_visitor_walks_fallout4_structs_groups_and_new_struct() {
    let script = crate::parse_with_mode(
        "ScriptName Fallout4Visit\n\n\
         Struct Coordinates\n    Float X\n    Float Y = 1.0\nEndStruct\n\n\
         Group Settings CollapsedOnBase\n    Int Property MaxCount = 5 Auto\nEndGroup\n\n\
         Function Test()\n    Coordinates local = new Coordinates\nEndFunction\n",
        crate::parser::GameEdition::Fallout4,
    )
    .unwrap();
    let mut counter = ExprCounter(0);
    counter.visit_script(&script);
    // The struct member's default value, the grouped property's default
    // value, and the `new Coordinates` struct instantiation: one visited
    // expression each.
    assert_eq!(counter.0, 3);
}

#[test]
fn ast_visitor_walks_nested_expressions() {
    let script =
        parse("ScriptName Example\nFunction Add(Int a = 1)\n    Return a + 2\nEndFunction\n")
            .unwrap();
    let mut counter = ExprCounter(0);
    counter.visit_script(&script);
    assert_eq!(counter.0, 4);
}

#[test]
fn token_visitor_walks_every_token() {
    let tokens = tokenize("ScriptName Example\n").unwrap();
    let mut counter = TokenCounter(0);
    counter.visit_tokens(&tokens);
    assert_eq!(counter.0, 1);
}
