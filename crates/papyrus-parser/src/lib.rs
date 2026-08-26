//! Basic AST parser for Bethesda's Papyrus scripting language.
//!
//! This module is the foundation the lint rules build on: a lexer that
//! turns Papyrus source text into tokens, an AST describing a script's
//! structure, and a recursive-descent parser that builds the AST from the
//! token stream.

pub mod ast;
pub mod lexer;
pub mod parser;
pub mod token;

use lexer::{LexError, Lexer};
use parser::{ParseError, Parser};

#[derive(Debug, Clone, PartialEq)]
pub enum PapyrusError {
    Lex(LexError),
    Parse(ParseError),
}

impl std::fmt::Display for PapyrusError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            PapyrusError::Lex(e) => write!(f, "{}:{}: {}", e.line, e.col, e.message),
            PapyrusError::Parse(e) => write!(f, "{}", e),
        }
    }
}

impl From<LexError> for PapyrusError {
    fn from(e: LexError) -> Self {
        PapyrusError::Lex(e)
    }
}

impl From<ParseError> for PapyrusError {
    fn from(e: ParseError) -> Self {
        PapyrusError::Parse(e)
    }
}

/// Parses Papyrus source text into a `Script` AST.
pub fn parse(source: &str) -> Result<ast::Script, PapyrusError> {
    let tokens = Lexer::new(source).tokenize()?;
    let script = Parser::new(tokens).parse_script()?;
    Ok(script)
}

#[cfg(test)]
mod tests {
    use super::ast::*;
    use super::*;

    #[test]
    fn parses_minimal_script() {
        let script = parse("ScriptName MyQuestScript extends Quest Hidden\n").unwrap();
        assert_eq!(script.name, "MyQuestScript");
        assert_eq!(script.extends.as_deref(), Some("Quest"));
        assert!(script.is_hidden);
        assert!(!script.is_conditional);
    }

    #[test]
    fn parses_imports_properties_and_variables() {
        let src = r#"
ScriptName Example extends ObjectReference

Import Utility
Import Debug

Int Property MaxCount = 10 Auto Hidden
Bool Property Enabled Auto
Actor Property PlayerRef Auto

float _cachedValue = 0.0
"#;
        let script = parse(src).unwrap();
        assert_eq!(script.imports, vec!["Utility", "Debug"]);
        assert_eq!(script.properties.len(), 3);

        let max_count = &script.properties[0];
        assert_eq!(max_count.name, "MaxCount");
        assert_eq!(max_count.type_name.name, "Int");
        assert!(max_count.is_auto);
        assert!(max_count.is_hidden);
        assert_eq!(max_count.value, Some(Expr::Literal(Literal::Int(10))));

        assert_eq!(script.variables.len(), 1);
        assert_eq!(script.variables[0].name, "_cachedValue");
        assert_eq!(script.variables[0].type_name.name, "float");
    }

    #[test]
    fn parses_function_with_params_and_body() {
        let src = r#"
ScriptName Example

Int Function Add(Int a, Int b = 1) Global
    Int result = a + b
    Return result
EndFunction
"#;
        let script = parse(src).unwrap();
        assert_eq!(script.functions.len(), 1);
        let f = &script.functions[0];
        assert_eq!(f.name, "Add");
        assert!(f.is_global);
        assert_eq!(f.return_type.as_ref().unwrap().name, "Int");
        assert_eq!(f.params.len(), 2);
        assert_eq!(f.params[1].default, Some(Expr::Literal(Literal::Int(1))));
        assert_eq!(f.body.len(), 2);
        assert!(matches!(f.body[0], Stmt::VarDecl(_)));
        assert!(matches!(f.body[1], Stmt::Return { .. }));
    }

    #[test]
    fn parses_native_function_without_body() {
        let src = "ScriptName Example\n\nFunction DoNative() Native\n\nFunction AfterNative()\nEndFunction\n";
        let script = parse(src).unwrap();
        assert_eq!(script.functions.len(), 2);
        assert!(script.functions[0].is_native);
        assert!(script.functions[0].body.is_empty());
        assert!(!script.functions[1].is_native);
    }

    #[test]
    fn parses_if_elseif_else() {
        let src = r#"
ScriptName Example

Function Check(Int x)
    If x > 10
        Debug.Trace("big")
    ElseIf x > 0
        Debug.Trace("small")
    Else
        Debug.Trace("non-positive")
    EndIf
EndFunction
"#;
        let script = parse(src).unwrap();
        let f = &script.functions[0];
        match &f.body[0] {
            Stmt::If {
                branches,
                else_body,
                ..
            } => {
                assert_eq!(branches.len(), 2);
                assert_eq!(else_body.len(), 1);
            }
            other => panic!("expected If statement, got {:?}", other),
        }
    }

    #[test]
    fn parses_while_loop_and_assignment() {
        let src = r#"
ScriptName Example

Function CountTo(Int n)
    Int i = 0
    While i < n
        i += 1
    EndWhile
EndFunction
"#;
        let script = parse(src).unwrap();
        let f = &script.functions[0];
        match &f.body[1] {
            Stmt::While { body, .. } => {
                assert_eq!(body.len(), 1);
                assert!(matches!(
                    body[0],
                    Stmt::Assign {
                        op: AssignOp::AddAssign,
                        ..
                    }
                ));
            }
            other => panic!("expected While statement, got {:?}", other),
        }
    }

    #[test]
    fn parses_expression_precedence() {
        let src = "ScriptName Example\n\nFunction Test()\n    Int x = 1 + 2 * 3\nEndFunction\n";
        let script = parse(src).unwrap();
        match &script.functions[0].body[0] {
            Stmt::VarDecl(decl) => {
                assert_eq!(
                    decl.value,
                    Some(Expr::Binary {
                        left: Box::new(Expr::Literal(Literal::Int(1))),
                        op: BinaryOp::Add,
                        right: Box::new(Expr::Binary {
                            left: Box::new(Expr::Literal(Literal::Int(2))),
                            op: BinaryOp::Mul,
                            right: Box::new(Expr::Literal(Literal::Int(3))),
                        }),
                    })
                );
            }
            other => panic!("expected VarDecl, got {:?}", other),
        }
    }

    #[test]
    fn parses_member_index_and_call_chains() {
        let src =
            "ScriptName Example\n\nFunction Test()\n    self.Items[0].DoThing(1, 2)\nEndFunction\n";
        let script = parse(src).unwrap();
        match &script.functions[0].body[0] {
            Stmt::Expr(Expr::Call { callee, args }) => {
                assert_eq!(args.len(), 2);
                match &**callee {
                    Expr::Member { property, .. } => assert_eq!(property, "DoThing"),
                    other => panic!("expected Member callee, got {:?}", other),
                }
            }
            other => panic!("expected call expression statement, got {:?}", other),
        }
    }

    #[test]
    fn parses_cast_and_new_array() {
        let src = r#"
ScriptName Example

Function Test()
    Float f = 1 as Float
    Int[] arr = new Int[5]
EndFunction
"#;
        let script = parse(src).unwrap();
        match &script.functions[0].body[0] {
            Stmt::VarDecl(decl) => assert!(matches!(decl.value, Some(Expr::Cast { .. }))),
            other => panic!("expected VarDecl, got {:?}", other),
        }
        match &script.functions[0].body[1] {
            Stmt::VarDecl(decl) => {
                assert!(decl.type_name.is_array);
                assert!(matches!(decl.value, Some(Expr::NewArray { .. })));
            }
            other => panic!("expected VarDecl, got {:?}", other),
        }
    }

    #[test]
    fn parses_states() {
        let src = r#"
ScriptName Example

Auto State Idle
    Function OnBegin()
    EndFunction
EndState

State Active
    Event OnUpdate()
    EndEvent
EndState
"#;
        let script = parse(src).unwrap();
        assert_eq!(script.states.len(), 2);
        assert_eq!(script.states[0].name, "Idle");
        assert!(script.states[0].is_auto);
        assert_eq!(script.states[1].name, "Active");
        assert!(!script.states[1].is_auto);
        assert!(script.states[1].functions[0].is_event);
    }

    #[test]
    fn parses_full_property_with_get_set() {
        let src = r#"
ScriptName Example

Int Property Total Hidden
    Int Function Get()
        Return 5
    EndFunction
EndProperty
"#;
        let script = parse(src).unwrap();
        assert_eq!(script.properties.len(), 1);
        let prop = &script.properties[0];
        assert!(!prop.is_auto);
        assert!(prop.is_hidden);
    }

    #[test]
    fn reports_error_with_location() {
        let err = parse("ScriptName Example\n\nFunction Bad(\nEndFunction\n").unwrap_err();
        match err {
            PapyrusError::Parse(e) => assert!(e.line >= 1),
            other => panic!("expected parse error, got {:?}", other),
        }
    }

    #[test]
    fn script_ast_is_json_serializable() {
        let script =
            parse("ScriptName Example extends Quest\n\nInt Property Count = 1 Auto\n").unwrap();
        let json = serde_json::to_string(&script).unwrap();
        assert!(json.contains("\"Example\""));
        assert!(json.contains("\"Count\""));
    }
}
