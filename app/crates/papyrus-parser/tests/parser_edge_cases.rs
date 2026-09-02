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
