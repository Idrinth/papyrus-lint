use papyrus_parser::ast::{BinaryOp, Expr, Literal, Stmt};
use papyrus_parser::{parse, PapyrusError};

#[test]
fn preserves_nested_control_flow_and_source_locations() {
    let script = parse(
        r#"ScriptName ControlFlow
Function Search(Int limit)
    Int index = 0
    While index < limit
        If index == 2
            Return index
        ElseIf index > 10
            Return -1
        Else
            index += 1
        EndIf
    EndWhile
EndFunction
"#,
    )
    .expect("nested control flow should parse");

    let function = &script.functions[0];
    assert_eq!(function.line, 2);
    let Stmt::While {
        condition,
        body,
        line,
        col,
    } = &function.body[1]
    else {
        panic!("expected a while statement");
    };
    assert_eq!((*line, *col), (4, 5));
    assert!(matches!(
        condition,
        Expr::Binary {
            op: BinaryOp::Lt,
            ..
        }
    ));

    let Stmt::If {
        branches,
        else_body,
        line,
    } = &body[0]
    else {
        panic!("expected an if statement inside the loop");
    };
    assert_eq!(*line, 5);
    assert_eq!(branches.len(), 2);
    assert_eq!((branches[0].line, branches[0].col), (5, 9));
    assert_eq!((branches[1].line, branches[1].col), (7, 9));
    assert!(matches!(
        branches[1].body[0],
        Stmt::Return {
            value: Some(Expr::Unary { .. }),
            line: 8,
        }
    ));
    assert!(matches!(else_body[0], Stmt::Assign { line: 10, .. }));
}

#[test]
fn parses_full_property_accessors_without_promoting_them_to_script_functions() {
    let script = parse(
        r#"ScriptName Properties
Int storedValue

Int Property Value
    Int Function Get()
        Return storedValue
    EndFunction

    Function Set(Int newValue)
        storedValue = newValue
    EndFunction
EndProperty
"#,
    )
    .expect("a property with get and set accessors should parse");

    assert_eq!(script.properties.len(), 1);
    assert_eq!(script.properties[0].name, "Value");
    assert!(!script.properties[0].is_auto);
    assert_eq!(script.variables.len(), 1);
    assert!(script.functions.is_empty());
}

#[test]
fn handles_case_insensitive_syntax_comments_and_line_continuations_together() {
    let script = parse(
        "sCrIpTnAmE MixedCase\r\n\
         fUnCtIoN Calculate()\r\n\
             ;/ the continued expression may span physical lines /;\r\n\
             Int result = 1 + \\\r\n\
                 2 * 3 ; trailing comment\r\n\
             rEtUrN result\r\n\
         eNdFuNcTiOn\r\n",
    )
    .expect("Papyrus syntax should be case insensitive");

    assert_eq!(script.name, "MixedCase");
    let function = &script.functions[0];
    assert_eq!(function.name, "Calculate");
    let Stmt::VarDecl(result) = &function.body[0] else {
        panic!("expected a result variable");
    };
    assert_eq!(
        result.value,
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
    assert!(matches!(function.body[1], Stmt::Return { line: 6, .. }));
}

#[test]
fn reports_the_unexpected_terminator_when_a_nested_terminator_is_missing() {
    let error = parse(
        "ScriptName Broken\n\
         Function Run()\n\
             While true\n\
                 If false\n\
                     Return\n\
                 EndIf\n\
         EndFunction\n",
    )
    .expect_err("the missing EndWhile must be rejected");

    let PapyrusError::Parse(error) = error else {
        panic!("expected a parser error");
    };
    assert_eq!((error.line, error.col), (7, 1));
    assert_eq!(error.message, "unexpected token Keyword(EndFunction)");
}
