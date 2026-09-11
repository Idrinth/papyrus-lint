//! Basic AST parser for Bethesda's Papyrus scripting language.
//!
//! This module is the foundation the lint rules build on: a lexer that
//! turns Papyrus source text into tokens, an AST describing a script's
//! structure, and a recursive-descent parser that builds the AST from the
//! token stream. [`parse`] and [`tokenize`] are both memoized against the
//! most recently seen source text (see [`cache`]), since a single lint
//! pass over one script calls into them dozens of times with the exact
//! same source.

pub mod ast;
mod cache;
pub mod lexer;
pub mod parser;
pub mod token;
pub mod types;

use lexer::LexError;
use parser::ParseError;

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

/// Parses Papyrus source text into a `Script` AST. Memoized against the
/// most recently seen `source` -- see [`cache`].
pub fn parse(source: &str) -> Result<ast::Script, PapyrusError> {
    cache::parse(source)
}

/// Lexes Papyrus source text into tokens, the same as
/// [`lexer::Lexer::new(source).tokenize()`](lexer::Lexer::tokenize).
/// Memoized against the most recently seen `source` -- see [`cache`] --
/// which is what lets every raw-token-based lint rule in `papyrus-lints`
/// call this directly instead of running its own lexer pass.
pub fn tokenize(source: &str) -> Result<Vec<token::Token>, LexError> {
    cache::tokenize(source)
}

/// Inserts a precomputed `ast` into [`parse`]'s in-memory memoization cache
/// as if `source` had just been parsed to it, so the next [`parse`] call
/// with the same `source` in this process returns it without re-parsing.
/// Lets a caller that already has a validated AST for `source` from
/// elsewhere (e.g. `papyrus-lint-core`'s disk-backed AST cache) short-circuit
/// this crate's own parse of it -- notably before calling into
/// `papyrus_lints::lint()`/`repair()`, which parse their `source` argument
/// internally without ever seeing this AST themselves.
pub fn prime_cache(source: &str, ast: ast::Script) {
    cache::prime(source, ast);
}

/// Same as [`prime_cache`], but for [`tokenize`]'s in-memory memoization
/// cache: inserts a precomputed `tokens` as if `source` had just been
/// lexed to it, so the next [`tokenize`] call with the same `source` in
/// this process returns it without re-lexing. Lets a caller that already
/// has a validated token stream for `source` from elsewhere (e.g.
/// `papyrus-lint-core`'s disk-backed AST cache) short-circuit this crate's
/// own lex of it -- notably before calling into
/// `papyrus_lints::lint()`/`repair()`, whose raw-token-based rules
/// tokenize their `source` argument internally without ever seeing these
/// tokens themselves.
pub fn prime_tokenize_cache(source: &str, tokens: Vec<token::Token>) {
    cache::prime_tokens(source, tokens);
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
        assert_eq!(max_count.value, Some(Expr::Literal(Literal::int(10))));

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
        assert_eq!(f.params[1].default, Some(Expr::Literal(Literal::int(1))));
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
                        left: Box::new(Expr::Literal(Literal::int(1))),
                        op: BinaryOp::Add,
                        right: Box::new(Expr::Binary {
                            left: Box::new(Expr::Literal(Literal::int(2))),
                            op: BinaryOp::Mul,
                            right: Box::new(Expr::Literal(Literal::int(3))),
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
            Stmt::Expr {
                value: Expr::Call { callee, args, .. },
                ..
            } => {
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
    fn parses_named_arguments_in_call() {
        let src = "ScriptName Example\n\nFunction MyFunction(Int argA = 0, Int argB = 0)\nEndFunction\n\nEvent OnUpdate()\n    MyFunction(argB = 1)\nEndEvent\n";
        let script = parse(src).unwrap();
        match &script.functions[1].body[0] {
            Stmt::Expr {
                value: Expr::Call { args, .. },
                ..
            } => {
                assert_eq!(args.len(), 1);
                match &args[0] {
                    Expr::NamedArg { name, value } => {
                        assert_eq!(name, "argB");
                        assert_eq!(**value, Expr::Literal(Literal::int(1)));
                    }
                    other => panic!("expected NamedArg, got {:?}", other),
                }
            }
            other => panic!("expected call expression statement, got {:?}", other),
        }
    }

    #[test]
    fn parses_mixed_positional_and_named_arguments_in_call() {
        let src = "ScriptName Example\n\nFunction Greet(String greeting, String name)\nEndFunction\n\nFunction Test()\n    Greet(\"Hi\", name = \"World\")\nEndFunction\n";
        let script = parse(src).unwrap();
        match &script.functions[1].body[0] {
            Stmt::Expr {
                value: Expr::Call { args, .. },
                ..
            } => {
                assert_eq!(args.len(), 2);
                assert_eq!(args[0], Expr::Literal(Literal::String("Hi".to_string())));
                match &args[1] {
                    Expr::NamedArg { name, value } => {
                        assert_eq!(name, "name");
                        assert_eq!(**value, Expr::Literal(Literal::String("World".to_string())));
                    }
                    other => panic!("expected NamedArg, got {:?}", other),
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
        assert_eq!(script.states[0].functions[0].state.as_deref(), Some("Idle"));
        assert_eq!(script.states[1].name, "Active");
        assert!(!script.states[1].is_auto);
        assert!(script.states[1].functions[0].is_event);
        assert_eq!(
            script.states[1].functions[0].state.as_deref(),
            Some("Active")
        );
    }

    #[test]
    fn empty_state_functions_carry_no_state_name() {
        let src = "ScriptName Example\n\nFunction DoThing()\nEndFunction\n";
        let script = parse(src).unwrap();
        assert_eq!(script.functions[0].state, None);
    }

    #[test]
    fn parses_full_property_with_both_get_and_set() {
        let src = r#"
ScriptName Example

Int myInt_Var = 0
Int Property myInt
    Int Function Get()
        Return myInt_Var
    EndFunction
    Function Set(Int value)
        myInt_Var = value
    EndFunction
EndProperty
"#;
        let script = parse(src).unwrap();
        assert_eq!(script.properties.len(), 1);
        let prop = &script.properties[0];
        assert!(!prop.is_auto);
        assert!(!prop.is_auto_read_only);
    }

    /// The "long" read-only property form: a full `Property`/`EndProperty`
    /// block that defines only a `Get` function and no `Set`, as opposed to
    /// the short `AutoReadOnly` form (see
    /// `parses_auto_read_only_property_with_required_default_value` below).
    #[test]
    fn parses_full_property_with_get_only_the_long_read_only_form() {
        let src = r#"
ScriptName Example

Int myVar = 5
Int Property Total Hidden
    Int Function Get()
        Return myVar
    EndFunction
EndProperty
"#;
        let script = parse(src).unwrap();
        assert_eq!(script.properties.len(), 1);
        let prop = &script.properties[0];
        assert!(!prop.is_auto);
        assert!(!prop.is_auto_read_only);
        assert!(prop.is_hidden);
    }

    #[test]
    fn parses_full_property_with_set_only_the_write_only_form() {
        let src = r#"
ScriptName Example

Int myVar = 5
Int Property WriteOnly
    Function Set(Int value)
        If value >= 0
            myVar = value
        Else
            myVar = 0
        EndIf
    EndFunction
EndProperty
"#;
        let script = parse(src).unwrap();
        assert_eq!(script.properties.len(), 1);
        let prop = &script.properties[0];
        assert!(!prop.is_auto);
        assert!(!prop.is_auto_read_only);
    }

    #[test]
    fn parses_auto_property_with_default_value() {
        let script = parse("ScriptName Example\n\nInt Property myInt = 5 Auto\n").unwrap();
        let prop = &script.properties[0];
        assert!(prop.is_auto);
        assert!(!prop.is_auto_read_only);
        assert_eq!(prop.value, Some(Expr::Literal(Literal::int(5))));
    }

    #[test]
    fn parses_auto_read_only_property_with_required_default_value() {
        let script =
            parse("ScriptName Example\n\nInt Property myReadOnlyInt = 20 AutoReadOnly\n").unwrap();
        let prop = &script.properties[0];
        assert!(!prop.is_auto);
        assert!(prop.is_auto_read_only);
        assert_eq!(prop.value, Some(Expr::Literal(Literal::int(20))));
    }

    #[test]
    fn parses_auto_property_with_conditional_flag() {
        let script = parse("ScriptName Example\n\nInt Property myVar Auto Conditional\n").unwrap();
        let prop = &script.properties[0];
        assert!(prop.is_auto);
        assert!(prop.is_conditional);
    }

    #[test]
    fn parses_array_typed_auto_property() {
        let script = parse("ScriptName Example\n\nInt[] Property Values Auto\n").unwrap();
        let prop = &script.properties[0];
        assert!(prop.type_name.is_array);
        assert!(prop.is_auto);
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

    #[test]
    fn tokenize_preserves_integer_formats_and_source_locations() {
        use super::token::{IntFormat, TokenKind};

        let tokens = tokenize("Int decimal = 42\nInt hexadecimal = 0x2A\n").unwrap();
        let integers: Vec<_> = tokens
            .iter()
            .filter_map(|token| match token.kind {
                TokenKind::IntLiteral(value, format) => {
                    Some((value, format, token.line, token.col))
                }
                _ => None,
            })
            .collect();

        assert_eq!(
            integers,
            vec![
                (42, IntFormat::Decimal, 1, 15),
                (42, IntFormat::Hexadecimal, 2, 19),
            ]
        );
        assert!(matches!(tokens.last().unwrap().kind, TokenKind::Eof));
    }

    #[test]
    fn tokenize_returns_independent_vectors_when_result_is_cached() {
        use super::token::TokenKind;

        let source = "ScriptName Cached\n";
        let mut first = tokenize(source).unwrap();
        assert!(matches!(first.pop().unwrap().kind, TokenKind::Eof));

        let second = tokenize(source).unwrap();
        assert!(matches!(second.last().unwrap().kind, TokenKind::Eof));
        assert_eq!(second.len(), first.len() + 1);
    }

    #[test]
    fn parse_returns_independent_asts_when_result_is_cached() {
        let source = "ScriptName CachedAst extends Quest\n";
        let mut first = parse(source).unwrap();
        first.name = "ChangedByCaller".to_string();
        first.extends = None;

        let second = parse(source).unwrap();
        assert_eq!(second.name, "CachedAst");
        assert_eq!(second.extends.as_deref(), Some("Quest"));
    }

    #[test]
    fn prime_cache_supplies_a_precomputed_ast_for_matching_source() {
        let source = "ScriptName SourceName\n";
        let mut precomputed = parse("ScriptName PrecomputedName Conditional\n").unwrap();
        precomputed.line = 42;

        prime_cache(source, precomputed.clone());

        assert_eq!(parse(source).unwrap(), precomputed);
    }

    #[test]
    fn primed_ast_is_ignored_for_a_different_source() {
        let primed = parse("ScriptName PrimedAst\n").unwrap();
        prime_cache("ScriptName PrimedSource\n", primed);

        let parsed = parse("ScriptName ActualSource Hidden\n").unwrap();
        assert_eq!(parsed.name, "ActualSource");
        assert!(parsed.is_hidden);
    }

    #[test]
    fn prime_tokenize_cache_supplies_precomputed_tokens_for_matching_source() {
        use super::token::{Token, TokenKind};

        let source = "ScriptName SourceTokens\n";
        let precomputed = vec![
            Token::new(TokenKind::Identifier("injected".to_string()), 7, 11),
            Token::new(TokenKind::Eof, 7, 19),
        ];

        prime_tokenize_cache(source, precomputed.clone());

        assert_eq!(tokenize(source).unwrap(), precomputed);
    }

    #[test]
    fn primed_tokens_are_ignored_for_a_different_source() {
        use super::token::{Keyword, Token, TokenKind};

        prime_tokenize_cache(
            "ScriptName PrimedTokens\n",
            vec![Token::new(TokenKind::Eof, 99, 99)],
        );

        let tokens = tokenize("ScriptName ActualTokens\n").unwrap();
        assert_eq!(tokens[0].kind, TokenKind::Keyword(Keyword::ScriptName));
        assert_eq!(tokens[0].line, 1);
        assert_eq!(tokens[0].col, 1);
        assert!(tokens.len() > 1);
    }

    #[test]
    fn reports_lex_errors_with_location_and_message() {
        let error = parse("ScriptName Example\n@\n").unwrap_err();

        assert_eq!(error.to_string(), "2:1: unexpected character '@'");
        assert!(matches!(
            error,
            PapyrusError::Lex(LexError {
                line: 2,
                col: 1,
                ..
            })
        ));
    }

    #[test]
    fn reports_parse_errors_with_location_and_message() {
        let error = parse("ScriptName Example\nFunction Broken(\n").unwrap_err();
        let message = error.to_string();

        assert!(message.starts_with("2:"), "unexpected error: {message}");
        assert!(message.contains("expected identifier"), "{message}");
        assert!(matches!(error, PapyrusError::Parse(_)));
    }
}
