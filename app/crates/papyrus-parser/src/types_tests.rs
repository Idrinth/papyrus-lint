use super::*;
use crate::parse;

#[test]
fn resolves_properties_and_variables_at_script_scope() {
    let script = parse(
        "ScriptName Example extends Quest\n\nInt Property Count = 1 Auto\nfloat _cached = 0.0\n",
    )
    .unwrap();
    let env = TypeEnv::for_script(&script);
    assert_eq!(env.lookup("Count"), Some(&scalar("Int")));
    assert_eq!(env.lookup("_cached"), Some(&scalar("float")));
    assert_eq!(env.lookup("Missing"), None);
}

#[test]
fn identifier_lookup_is_case_insensitive() {
    let script = parse(
            "ScriptName Example\n\nInt Property Count = 1 Auto\n\nFunction Test(Float aValue)\nEndFunction\n",
        )
        .unwrap();
    let mut env = TypeEnv::for_script(&script);
    assert_eq!(env.lookup("COUNT"), Some(&scalar("Int")));
    assert_eq!(env.lookup("count"), Some(&scalar("Int")));

    let function = &script.functions[0];
    env.with_function_scope(function, |scoped| {
        assert_eq!(scoped.lookup("AVALUE"), Some(&scalar("Float")));
    });
}

#[test]
fn function_scope_covers_params_and_nested_locals_then_pops() {
    let script = parse(
        r#"
ScriptName Example

Function Test(Int a)
    If a > 0
        Float b = 1.0
    EndIf
EndFunction
"#,
    )
    .unwrap();
    let mut env = TypeEnv::for_script(&script);
    let function = &script.functions[0];
    env.with_function_scope(function, |scoped| {
        assert_eq!(scoped.lookup("a"), Some(&scalar("Int")));
        assert_eq!(scoped.lookup("b"), Some(&scalar("Float")));
    });
    assert_eq!(env.lookup("a"), None);
    assert_eq!(env.lookup("b"), None);
}

#[test]
fn local_shadows_same_named_property() {
    let script = parse(
            "ScriptName Example\n\nInt Property Count = 1 Auto\n\nFunction Test()\n    Float Count = 1.0\nEndFunction\n",
        )
        .unwrap();
    let mut env = TypeEnv::for_script(&script);
    let function = &script.functions[0];
    env.with_function_scope(function, |scoped| {
        assert_eq!(scoped.lookup("Count"), Some(&scalar("Float")));
    });
}

#[test]
fn infers_literal_and_identifier_types() {
    let script = parse("ScriptName Example\n\nInt Property Count = 1 Auto\n").unwrap();
    let env = TypeEnv::for_script(&script);
    assert_eq!(
        infer_type(&Expr::Literal(Literal::int(1)), &env),
        Some(scalar("Int"))
    );
    assert_eq!(
        infer_type(&Expr::Literal(Literal::Bool(true)), &env),
        Some(scalar("Bool"))
    );
    assert_eq!(infer_type(&Expr::Literal(Literal::None), &env), None);
    assert_eq!(
        infer_type(&Expr::Identifier("Count".to_string()), &env),
        Some(scalar("Int"))
    );
    assert_eq!(infer_type(&Expr::Self_, &env), Some(scalar("Example")));
}

#[test]
fn infers_named_argument_type_from_its_value() {
    let script = parse("ScriptName Example\n").unwrap();
    let env = TypeEnv::for_script(&script);
    let argument = Expr::NamedArg {
        name: "amount".to_string(),
        value: Box::new(Expr::Literal(Literal::Float(1.5))),
    };

    assert_eq!(infer_type(&argument, &env), Some(scalar("Float")));
}

#[test]
fn comparisons_and_boolean_ops_always_yield_bool() {
    let script = parse("ScriptName Example\n").unwrap();
    let env = TypeEnv::for_script(&script);
    let cmp = Expr::Binary {
        left: Box::new(Expr::Literal(Literal::int(1))),
        op: BinaryOp::Gt,
        right: Box::new(Expr::Literal(Literal::Float(2.0))),
    };
    assert_eq!(infer_type(&cmp, &env), Some(scalar("Bool")));
}

#[test]
fn arithmetic_promotes_int_and_float_and_concatenates_strings() {
    let script = parse("ScriptName Example\n").unwrap();
    let env = TypeEnv::for_script(&script);

    let int_plus_int = Expr::Binary {
        left: Box::new(Expr::Literal(Literal::int(1))),
        op: BinaryOp::Add,
        right: Box::new(Expr::Literal(Literal::int(2))),
    };
    assert_eq!(infer_type(&int_plus_int, &env), Some(scalar("Int")));

    let int_plus_float = Expr::Binary {
        left: Box::new(Expr::Literal(Literal::int(1))),
        op: BinaryOp::Add,
        right: Box::new(Expr::Literal(Literal::Float(2.0))),
    };
    assert_eq!(infer_type(&int_plus_float, &env), Some(scalar("Float")));

    let string_concat = Expr::Binary {
        left: Box::new(Expr::Literal(Literal::String("a".to_string()))),
        op: BinaryOp::Add,
        right: Box::new(Expr::Literal(Literal::int(2))),
    };
    assert_eq!(infer_type(&string_concat, &env), Some(scalar("String")));
}

#[test]
fn cast_new_array_and_index_resolve_from_their_declared_types() {
    let script = parse("ScriptName Example\n").unwrap();
    let env = TypeEnv::for_script(&script);

    let cast = Expr::Cast {
        value: Box::new(Expr::Literal(Literal::int(1))),
        type_name: "Float".to_string(),
    };
    assert_eq!(infer_type(&cast, &env), Some(scalar("Float")));

    let type_check = Expr::Is {
        value: Box::new(Expr::Identifier("akActionRef".to_string())),
        type_name: "Actor".to_string(),
    };
    assert_eq!(infer_type(&type_check, &env), Some(scalar("Bool")));

    let new_array = Expr::NewArray {
        type_name: scalar("Int"),
        size: Box::new(Expr::Literal(Literal::int(5))),
    };
    assert_eq!(
        infer_type(&new_array, &env),
        Some(TypeName {
            name: "Int".to_string(),
            is_array: true,
        })
    );

    let index = Expr::Index {
        object: Box::new(new_array),
        index: Box::new(Expr::Literal(Literal::int(0))),
    };
    assert_eq!(infer_type(&index, &env), Some(scalar("Int")));
}

#[test]
fn member_access_and_calls_are_unresolvable_without_other_scripts() {
    let script = parse("ScriptName Example\n").unwrap();
    let env = TypeEnv::for_script(&script);

    let member = Expr::Member {
        object: Box::new(Expr::Self_),
        property: "SomeField".to_string(),
    };
    assert_eq!(infer_type(&member, &env), None);

    let call = Expr::Call {
        callee: Box::new(Expr::Identifier("DoThing".to_string())),
        args: Vec::new(),
        line: 1,
        col: 1,
    };
    assert_eq!(infer_type(&call, &env), None);
}

#[test]
fn infers_new_struct_type() {
    let script = parse("ScriptName Example\n").unwrap();
    let env = TypeEnv::for_script(&script);
    let expression = Expr::NewStruct {
        type_name: "Point".into(),
    };

    assert_eq!(infer_type(&expression, &env), Some(scalar("Point")));
}

#[test]
fn arithmetic_requires_both_operand_types_to_be_known() {
    let script = parse("ScriptName Example\n").unwrap();
    let env = TypeEnv::for_script(&script);
    let add = |left, right| Expr::Binary {
        left: Box::new(left),
        op: BinaryOp::Add,
        right: Box::new(right),
    };

    assert_eq!(
        infer_type(
            &add(
                Expr::Identifier("missing".into()),
                Expr::Literal(Literal::int(1)),
            ),
            &env,
        ),
        None
    );
    assert_eq!(
        infer_type(
            &add(
                Expr::Literal(Literal::int(1)),
                Expr::Identifier("missing".into()),
            ),
            &env,
        ),
        None
    );
}
