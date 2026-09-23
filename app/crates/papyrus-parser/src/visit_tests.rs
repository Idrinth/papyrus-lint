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

#[derive(Default)]
struct NodeCounter {
    imports: usize,
    properties: usize,
    variables: usize,
    states: usize,
    functions: usize,
    params: usize,
    statements: usize,
    branches: usize,
    types: usize,
}

impl Visitor for NodeCounter {
    fn visit_import(&mut self, _: &ImportDecl) {
        self.imports += 1;
    }

    fn visit_property(&mut self, property: &PropertyDecl) {
        self.properties += 1;
        walk_property(self, property);
    }

    fn visit_variable(&mut self, variable: &VariableDecl) {
        self.variables += 1;
        walk_variable(self, variable);
    }

    fn visit_state(&mut self, state: &StateDecl) {
        self.states += 1;
        walk_state(self, state);
    }

    fn visit_function(&mut self, function: &FunctionDecl) {
        self.functions += 1;
        walk_function(self, function);
    }

    fn visit_param(&mut self, param: &Param) {
        self.params += 1;
        walk_param(self, param);
    }

    fn visit_stmt(&mut self, statement: &Stmt) {
        self.statements += 1;
        walk_stmt(self, statement);
    }

    fn visit_if_branch(&mut self, branch: &IfBranch) {
        self.branches += 1;
        walk_if_branch(self, branch);
    }

    fn visit_type_name(&mut self, _: &TypeName) {
        self.types += 1;
    }
}

#[test]
fn ast_visitor_reaches_every_declaration_and_statement_shape() {
    let script = parse(
        "ScriptName Visitor\n\
         Import Utility\n\
         Int Property Limit = 3 Auto\n\
         String label\n\
         Function Run(Int amount = 1)\n\
             Int current = amount\n\
             current += 1\n\
             Debug.Trace(label)\n\
             If current > Limit\n\
                 Return\n\
             ElseIf current == Limit\n\
                 Return current\n\
             Else\n\
                 While current < Limit\n\
                     current += 1\n\
                 EndWhile\n\
             EndIf\n\
         EndFunction\n\
         State Active\n\
             Event OnBeginState()\n\
             EndEvent\n\
         EndState\n",
    )
    .unwrap();
    let mut counter = NodeCounter::default();

    counter.visit_script(&script);

    assert_eq!(counter.imports, 1);
    assert_eq!(counter.properties, 1);
    assert_eq!(counter.variables, 2); // script variable and function local
    assert_eq!(counter.states, 1);
    assert_eq!(counter.functions, 2); // top-level function and state event
    assert_eq!(counter.params, 1);
    assert_eq!(counter.statements, 8);
    assert_eq!(counter.branches, 2);
    assert_eq!(counter.types, 4);
}
