use papyrus_parser::ast::{BinaryOp, Expr, Literal, Stmt, TypeName, UnaryOp};
use papyrus_parser::parse;
use papyrus_parser::types::{infer_type, TypeEnv};

fn scalar(name: &str) -> TypeName {
    TypeName {
        name: name.to_string(),
        is_array: false,
    }
}

fn binary(left: Expr, op: BinaryOp, right: Expr) -> Expr {
    Expr::Binary {
        left: Box::new(left),
        op,
        right: Box::new(right),
    }
}

#[test]
fn resolves_self_parent_and_missing_parent_types() {
    let child = parse("ScriptName Child Extends ParentScript\n").expect("script should parse");
    let child_env = TypeEnv::for_script(&child);

    assert_eq!(infer_type(&Expr::Self_, &child_env), Some(scalar("Child")));
    assert_eq!(
        infer_type(&Expr::Parent, &child_env),
        Some(scalar("ParentScript"))
    );

    let root = parse("ScriptName Root\n").expect("script should parse");
    assert_eq!(infer_type(&Expr::Parent, &TypeEnv::for_script(&root)), None);
}

#[test]
fn function_scope_collects_locals_from_every_nested_branch_and_loop() {
    let script = parse(
        r#"ScriptName Scoped
String Property Shared Auto
Function Inspect(Int argument)
    If true
        Float fromIf = 1.0
    ElseIf false
        Bool fromElseIf = true
    Else
        String fromElse = "fallback"
    EndIf
    While false
        Form[] fromWhile
    EndWhile
    Int Shared = 2
EndFunction
"#,
    )
    .expect("nested locals should parse");
    let mut env = TypeEnv::for_script(&script);

    env.with_function_scope(&script.functions[0], |scoped| {
        for (name, expected) in [
            ("argument", scalar("Int")),
            ("fromIf", scalar("Float")),
            ("fromElseIf", scalar("Bool")),
            ("fromElse", scalar("String")),
            (
                "fromWhile",
                TypeName {
                    name: "Form".into(),
                    is_array: true,
                },
            ),
            ("Shared", scalar("Int")),
        ] {
            assert_eq!(scoped.lookup(name), Some(&expected), "lookup for {name}");
        }
    });

    assert_eq!(env.lookup("Shared"), Some(&scalar("String")));
    assert_eq!(env.lookup("argument"), None);
    assert_eq!(env.lookup("fromWhile"), None);
}

#[test]
fn infers_all_literal_and_unary_expression_types() {
    let script = parse("ScriptName Expressions\n").expect("script should parse");
    let env = TypeEnv::for_script(&script);

    for (expression, expected) in [
        (Expr::Literal(Literal::int(4)), Some(scalar("Int"))),
        (Expr::Literal(Literal::Float(4.5)), Some(scalar("Float"))),
        (
            Expr::Literal(Literal::String("four".into())),
            Some(scalar("String")),
        ),
        (Expr::Literal(Literal::Bool(true)), Some(scalar("Bool"))),
        (Expr::Literal(Literal::None), None),
        (
            Expr::Unary {
                op: UnaryOp::Not,
                operand: Box::new(Expr::Literal(Literal::int(1))),
            },
            Some(scalar("Bool")),
        ),
        (
            Expr::Unary {
                op: UnaryOp::Neg,
                operand: Box::new(Expr::Literal(Literal::Float(1.5))),
            },
            Some(scalar("Float")),
        ),
    ] {
        assert_eq!(infer_type(&expression, &env), expected, "{expression:?}");
    }
}

#[test]
fn arithmetic_inference_covers_promotion_concatenation_and_invalid_operands() {
    let script = parse("ScriptName Arithmetic\n").expect("script should parse");
    let env = TypeEnv::for_script(&script);

    for op in [
        BinaryOp::Add,
        BinaryOp::Sub,
        BinaryOp::Mul,
        BinaryOp::Div,
        BinaryOp::Mod,
    ] {
        let ints = binary(
            Expr::Literal(Literal::int(6)),
            op,
            Expr::Literal(Literal::int(2)),
        );
        assert_eq!(infer_type(&ints, &env), Some(scalar("Int")), "{op:?}");

        let promoted = binary(
            Expr::Literal(Literal::int(6)),
            op,
            Expr::Literal(Literal::Float(2.0)),
        );
        assert_eq!(infer_type(&promoted, &env), Some(scalar("Float")), "{op:?}");
    }

    let concat = binary(
        Expr::Literal(Literal::int(6)),
        BinaryOp::Add,
        Expr::Literal(Literal::String(" items".into())),
    );
    assert_eq!(infer_type(&concat, &env), Some(scalar("String")));

    let invalid = binary(
        Expr::Literal(Literal::Bool(true)),
        BinaryOp::Mul,
        Expr::Literal(Literal::int(2)),
    );
    assert_eq!(infer_type(&invalid, &env), None);
}

#[test]
fn boolean_operators_and_comparisons_always_infer_bool() {
    let script = parse("ScriptName Conditions\n").expect("script should parse");
    let env = TypeEnv::for_script(&script);

    for op in [
        BinaryOp::Eq,
        BinaryOp::NotEq,
        BinaryOp::Gt,
        BinaryOp::Lt,
        BinaryOp::GtEq,
        BinaryOp::LtEq,
        BinaryOp::And,
        BinaryOp::Or,
    ] {
        let expression = binary(
            Expr::Identifier("unknownLeft".into()),
            op,
            Expr::Identifier("unknownRight".into()),
        );
        assert_eq!(
            infer_type(&expression, &env),
            Some(scalar("Bool")),
            "{op:?}"
        );
    }
}

#[test]
fn array_indexing_requires_an_inferable_array_expression() {
    let script = parse("ScriptName Arrays\nInt[] Property Values Auto\nInt Property Scalar Auto\n")
        .expect("script should parse");
    let env = TypeEnv::for_script(&script);
    let index = |name: &str| Expr::Index {
        object: Box::new(Expr::Identifier(name.into())),
        index: Box::new(Expr::Literal(Literal::int(0))),
    };

    assert_eq!(infer_type(&index("Values"), &env), Some(scalar("Int")));
    assert_eq!(infer_type(&index("Scalar"), &env), None);
    assert_eq!(infer_type(&index("Missing"), &env), None);
}

#[test]
fn named_arguments_delegate_to_the_value_but_calls_and_members_remain_unknown() {
    let script = parse("ScriptName Calls\nFunction Run()\n    Submit(amount = 3)\nEndFunction\n")
        .expect("call should parse");
    let env = TypeEnv::for_script(&script);
    let Stmt::Expr {
        value: call @ Expr::Call { args, .. },
        ..
    } = &script.functions[0].body[0]
    else {
        panic!("expected a call expression");
    };

    assert_eq!(infer_type(&args[0], &env), Some(scalar("Int")));
    assert_eq!(infer_type(call, &env), None);
    assert_eq!(
        infer_type(
            &Expr::Member {
                object: Box::new(Expr::Self_),
                property: "Value".into(),
            },
            &env,
        ),
        None
    );
}
