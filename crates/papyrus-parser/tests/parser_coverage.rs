use papyrus_parser::ast::{AssignOp, BinaryOp, Expr, Literal, Stmt, UnaryOp};
use papyrus_parser::{parse, PapyrusError};

#[test]
fn parses_all_property_and_script_modifiers() {
    let script = parse(
        "ScriptName Flags Conditional Hidden\n\
         Int Property ReadOnly = 3 AutoReadOnly Conditional Hidden\n\
         Bool Property Writable Auto Conditional\n\
         Int counter = 0 Conditional\n",
    )
    .expect("script should parse");

    assert!(script.is_hidden);
    assert!(script.is_conditional);
    let read_only = &script.properties[0];
    assert!(read_only.is_auto_read_only);
    assert!(read_only.is_hidden);
    assert!(read_only.is_conditional);
    assert_eq!(read_only.value, Some(Expr::Literal(Literal::Int(3))));
    assert!(script.properties[1].is_auto);
    assert!(script.properties[1].is_conditional);
    assert!(script.variables[0].is_conditional);
}

#[test]
fn parses_literals_unary_boolean_and_comparison_operators() {
    let script = parse(
        r#"ScriptName Expressions
Function Test()
    Bool a = !false || true && 1 != 2
    Bool b = 1 < 2 && 2 <= 2 && 3 > 2 && 3 >= 3 && 1 == 1
    Int c = -(8 - 3) / 5 % 2
    String s = "value"
    ObjectReference missing = None
    ObjectReference me = Self
    ObjectReference base = Parent
EndFunction
"#,
    )
    .expect("script should parse");

    let body = &script.functions[0].body;
    let value = |index| match &body[index] {
        Stmt::VarDecl(variable) => variable.value.as_ref().unwrap(),
        statement => panic!("expected variable declaration, got {statement:?}"),
    };
    assert!(matches!(
        value(0),
        Expr::Binary {
            op: BinaryOp::Or,
            ..
        }
    ));
    assert!(matches!(
        value(1),
        Expr::Binary {
            op: BinaryOp::And,
            ..
        }
    ));
    assert!(matches!(
        value(2),
        Expr::Binary {
            op: BinaryOp::Mod,
            left,
            ..
        } if matches!(left.as_ref(), Expr::Binary { op: BinaryOp::Div, .. })
    ));
    assert_eq!(value(3), &Expr::Literal(Literal::String("value".into())));
    assert_eq!(value(4), &Expr::Literal(Literal::None));
    assert_eq!(value(5), &Expr::Self_);
    assert_eq!(value(6), &Expr::Parent);

    let Expr::Binary { left, .. } = value(0) else {
        unreachable!()
    };
    assert!(matches!(
        left.as_ref(),
        Expr::Unary {
            op: UnaryOp::Not,
            ..
        }
    ));
}

#[test]
fn parses_every_assignment_operator_and_bare_return() {
    let script = parse(
        "ScriptName Assignments\n\
         Function Update()\n\
             Int value = 10\n\
             value = 9\n\
             value += 1\n\
             value -= 2\n\
             value *= 3\n\
             value /= 4\n\
             value %= 5\n\
             Return\n\
         EndFunction\n",
    )
    .expect("script should parse");

    let body = &script.functions[0].body;
    let operators: Vec<_> = body[1..7]
        .iter()
        .map(|statement| match statement {
            Stmt::Assign { op, .. } => *op,
            other => panic!("expected assignment, got {other:?}"),
        })
        .collect();
    assert_eq!(
        operators,
        [
            AssignOp::Assign,
            AssignOp::AddAssign,
            AssignOp::SubAssign,
            AssignOp::MulAssign,
            AssignOp::DivAssign,
            AssignOp::ModAssign,
        ]
    );
    assert!(matches!(body[7], Stmt::Return { value: None, .. }));
}

#[test]
fn parses_typed_state_functions_array_parameters_and_empty_calls() {
    let script = parse(
        "ScriptName Stateful\n\
         State Running\n\
             Int Function Count(String[] names) Global Native\n\
             Event OnBegin()\n\
                 Reset()\n\
             EndEvent\n\
         EndState\n",
    )
    .expect("state should parse");

    let functions = &script.states[0].functions;
    assert_eq!(functions[0].return_type.as_ref().unwrap().name, "Int");
    assert!(functions[0].is_global);
    assert!(functions[0].is_native);
    assert!(functions[0].params[0].type_name.is_array);
    assert!(functions[1].is_event);
    assert!(matches!(
        &functions[1].body[0],
        Stmt::Expr {
            value: Expr::Call { args, .. },
            ..
        } if args.is_empty()
    ));
}

#[test]
fn returns_precise_lex_and_parse_errors() {
    let lex_error = parse("ScriptName Bad\n@").unwrap_err();
    assert_eq!(lex_error.to_string(), "2:1: unexpected character '@'");
    assert!(matches!(lex_error, PapyrusError::Lex(_)));

    for (source, expected_message) in [
        ("NotScript Bad\n", "expected keyword ScriptName"),
        ("ScriptName Bad trailing\n", "expected end of line"),
        ("ScriptName Bad\nState Open\n", "expected EndState"),
        (
            "ScriptName Bad\nFunction Broken()\nInt value =\nEndFunction\n",
            "unexpected token Newline",
        ),
    ] {
        let error = parse(source).unwrap_err();
        let PapyrusError::Parse(error) = error else {
            panic!("expected parse error")
        };
        assert!(
            error.message.contains(expected_message),
            "expected {:?} to contain {expected_message:?}",
            error.message
        );
        assert!(error.line >= 1);
        assert!(error.col >= 1);
    }
}
