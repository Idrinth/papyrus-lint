use papyrus_parser::ast::{BinaryOp, Expr, Literal, Stmt};
use papyrus_parser::{parse, PapyrusError};

#[test]
fn records_auto_state_metadata_on_each_declaration() {
    let script = parse(
        r#"ScriptName Stateful
Auto State Waiting
    Function Start()
    EndFunction

    Event OnUpdate()
    EndEvent
EndState

State Running
    Bool Function IsReady() Native
EndState
"#,
    )
    .expect("states should parse");

    assert_eq!(script.states.len(), 2);
    assert_eq!(script.states[0].name, "Waiting");
    assert!(script.states[0].is_auto);
    assert_eq!(script.states[0].line, 2);
    assert_eq!(script.states[0].functions.len(), 2);
    assert!(script.states[0]
        .functions
        .iter()
        .all(|function| function.state.as_deref() == Some("Waiting")));
    assert!(script.states[0].functions[1].is_event);

    assert_eq!(script.states[1].name, "Running");
    assert!(!script.states[1].is_auto);
    assert_eq!(
        script.states[1].functions[0].state.as_deref(),
        Some("Running")
    );
    assert!(script.states[1].functions[0].is_native);
}

#[test]
fn parses_mixed_positional_and_named_call_arguments() {
    let script = parse(
        r#"ScriptName Calls
Function Run()
    Submit(1 + 2, enabled = !false, label = "ready")
EndFunction
"#,
    )
    .expect("mixed call arguments should parse");

    let Stmt::Expr {
        value: Expr::Call { args, .. },
        ..
    } = &script.functions[0].body[0]
    else {
        panic!("expected a call expression");
    };

    assert!(matches!(
        &args[0],
        Expr::Binary {
            op: BinaryOp::Add,
            ..
        }
    ));
    assert!(matches!(
        &args[1],
        Expr::NamedArg { name, value }
            if name == "enabled" && matches!(value.as_ref(), Expr::Unary { .. })
    ));
    assert_eq!(
        args[2],
        Expr::NamedArg {
            name: "label".into(),
            value: Box::new(Expr::Literal(Literal::String("ready".into()))),
        }
    );
}

#[test]
fn accepts_eof_as_the_final_statement_terminator() {
    let script = parse("ScriptName NoFinalNewline").expect("EOF should terminate the header");

    assert_eq!(script.name, "NoFinalNewline");
    assert!(script.functions.is_empty());
}

#[test]
fn reports_the_start_of_unterminated_lexical_constructs() {
    for (source, message, line, col) in [
        (
            "ScriptName Broken\n;/ comment",
            "unterminated block comment",
            2,
            1,
        ),
        (
            "ScriptName Broken\n{documentation",
            "unterminated comment block",
            2,
            1,
        ),
        (
            "ScriptName Broken\nString value = \"text",
            "unterminated string literal",
            2,
            16,
        ),
    ] {
        let PapyrusError::Lex(error) = parse(source).expect_err("source should not lex") else {
            panic!("expected a lexer error");
        };

        assert_eq!(error.message, message);
        assert_eq!((error.line, error.col), (line, col));
    }
}

#[test]
fn distinguishes_an_empty_else_clause_from_no_else_clause() {
    let script = parse(
        r#"ScriptName Branches
Function Check(Bool first, Bool second)
    If first
    Else
    EndIf

    If second
    EndIf
EndFunction
"#,
    )
    .expect("empty conditional bodies should parse");

    let Stmt::If {
        branches,
        else_body,
        else_line,
        else_col,
        ..
    } = &script.functions[0].body[0]
    else {
        panic!("expected the first statement to be an if");
    };
    assert!(branches[0].body.is_empty());
    assert!(else_body.is_empty());
    assert_eq!((*else_line, *else_col), (Some(4), Some(5)));

    let Stmt::If {
        else_body,
        else_line,
        else_col,
        ..
    } = &script.functions[0].body[1]
    else {
        panic!("expected the second statement to be an if");
    };
    assert!(else_body.is_empty());
    assert_eq!((*else_line, *else_col), (None, None));
}

#[test]
fn preserves_complex_default_parameter_expressions() {
    let script = parse(
        r#"ScriptName Defaults
Function Configure(Int count = 2 * (3 + 4), Bool enabled = !false, String label = "ready") Native
"#,
    )
    .expect("expression defaults should parse");

    let params = &script.functions[0].params;
    assert!(matches!(
        params[0].default,
        Some(Expr::Binary {
            op: BinaryOp::Mul,
            ref right,
            ..
        }) if matches!(right.as_ref(), Expr::Binary { op: BinaryOp::Add, .. })
    ));
    assert!(matches!(params[1].default, Some(Expr::Unary { .. })));
    assert_eq!(
        params[2].default,
        Some(Expr::Literal(Literal::String("ready".into())))
    );
}

#[test]
fn rejects_unterminated_full_properties_at_end_of_file() {
    let error = parse(
        "ScriptName Broken\n\
         Int Property Value\n\
             Int Function Get()\n\
                 Return 1\n\
             EndFunction\n",
    )
    .expect_err("a full property requires EndProperty");

    let PapyrusError::Parse(error) = error else {
        panic!("expected a parser error");
    };
    assert_eq!((error.line, error.col), (6, 1));
    assert_eq!(error.message, "expected keyword EndProperty, found Eof");
}
