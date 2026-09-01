use papyrus_parser::ast::{AssignOp, Expr, Literal, Script, Stmt};
use papyrus_parser::{parse, PapyrusError};

#[test]
fn preserves_call_locations_through_postfix_chains() {
    let script = parse(
        "ScriptName Calls\n\
         Function Run()\n\
             handlers[0].Invoke(value = 42)\n\
         EndFunction\n",
    )
    .expect("a chained call should parse");

    let Stmt::Expr {
        value:
            Expr::Call {
                callee,
                args,
                line,
                col,
            },
        ..
    } = &script.functions[0].body[0]
    else {
        panic!("expected a call expression");
    };

    assert_eq!((*line, *col), (3, 19));
    assert!(matches!(
        callee.as_ref(),
        Expr::Member { object, property }
            if property == "Invoke" && matches!(object.as_ref(), Expr::Index { .. })
    ));
    assert!(matches!(
        &args[0],
        Expr::NamedArg { name, value }
            if name == "value" && **value == Expr::Literal(Literal::Int(42))
    ));
}

#[test]
fn parses_assignments_to_member_and_index_targets() {
    let script = parse(
        "ScriptName AssignTargets\n\
         Function Update(Int index)\n\
             settings.Enabled = true\n\
             values[index] *= 2\n\
         EndFunction\n",
    )
    .expect("member and index assignments should parse");

    assert!(matches!(
        &script.functions[0].body[0],
        Stmt::Assign {
            target: Expr::Member { property, .. },
            op: AssignOp::Assign,
            value: Expr::Literal(Literal::Bool(true)),
            line: 3,
        } if property == "Enabled"
    ));
    assert!(matches!(
        &script.functions[0].body[1],
        Stmt::Assign {
            target: Expr::Index { index, .. },
            op: AssignOp::MulAssign,
            value: Expr::Literal(Literal::Int(2)),
            line: 4,
        } if **index == Expr::Identifier("index".into())
    ));
}

#[test]
fn supports_chained_casts_and_array_types() {
    let script = parse(
        "ScriptName Casts\n\
         Function Convert(Form value)\n\
             ObjectReference result = value as Alias as ObjectReference\n\
             Form[] copies = new Form[3]\n\
         EndFunction\n",
    )
    .expect("casts and array creation should parse");

    let Stmt::VarDecl(result) = &script.functions[0].body[0] else {
        panic!("expected a result declaration");
    };
    assert!(matches!(
        result.value.as_ref(),
        Some(Expr::Cast { value, type_name })
            if type_name == "ObjectReference"
                && matches!(value.as_ref(), Expr::Cast { type_name, .. } if type_name == "Alias")
    ));

    let Stmt::VarDecl(copies) = &script.functions[0].body[1] else {
        panic!("expected an array declaration");
    };
    assert!(copies.type_name.is_array);
    assert!(matches!(
        copies.value.as_ref(),
        Some(Expr::NewArray { type_name, size })
            if type_name.name == "Form"
                && !type_name.is_array
                && **size == Expr::Literal(Literal::Int(3))
    ));
}

#[test]
fn ast_json_round_trip_preserves_the_complete_tree() {
    let script = parse(
        "ScriptName Serializable Extends Quest Hidden Conditional\n\
         String Property Label = \"ready\" AutoReadOnly\n\
         Function Run(Bool enabled = true)\n\
             If enabled\n\
                 Return\n\
             EndIf\n\
         EndFunction\n",
    )
    .expect("fixture should parse");

    let json = serde_json::to_string(&script).expect("the AST should serialize");
    let restored: Script = serde_json::from_str(&json).expect("the AST should deserialize");

    assert_eq!(restored, script);
}

#[test]
fn reports_missing_block_terminators_at_end_of_file() {
    for (source, expected_message) in [
        (
            "ScriptName Broken\nFunction Run()\nWhile true\nReturn\n",
            "expected keyword EndWhile, found Eof",
        ),
        (
            "ScriptName Broken\nInt Property Value\nFunction Get()\n",
            "expected keyword EndProperty, found Eof",
        ),
    ] {
        let PapyrusError::Parse(error) = parse(source).expect_err("the block is unterminated")
        else {
            panic!("expected a parse error");
        };

        assert_eq!(error.message, expected_message);
        assert_eq!(error.line, source.lines().count() + 1);
        assert_eq!(error.col, 1);
    }
}
