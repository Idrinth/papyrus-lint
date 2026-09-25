use super::*;
use papyrus_parser::ast::{BinaryOp, Expr, Literal, UnaryOp};

fn int(value: i64) -> Expr {
    Expr::Literal(Literal::int(value))
}

fn float(value: f64) -> Expr {
    Expr::Literal(Literal::Float(value))
}

fn binary(left: Expr, op: BinaryOp, right: Expr) -> Expr {
    Expr::Binary {
        left: Box::new(left),
        op,
        right: Box::new(right),
    }
}

fn neg(operand: Expr) -> Expr {
    Expr::Unary {
        op: UnaryOp::Neg,
        operand: Box::new(operand),
    }
}

fn member(object: Expr, property: &str) -> Expr {
    Expr::Member {
        object: Box::new(object),
        property: property.into(),
    }
}

fn call(callee: Expr) -> Expr {
    Expr::Call {
        callee: Box::new(callee),
        args: Vec::new(),
        line: 1,
        col: 1,
    }
}

#[test]
fn as_number_rejects_a_non_numeric_literal() {
    assert_eq!(as_number(&Literal::Bool(true)), None);
}

#[test]
fn eval_const_folds_int_arithmetic_and_unary_neg() {
    assert_eq!(
        eval_const(&binary(int(1), BinaryOp::Add, int(2))),
        Some(Literal::int(3))
    );
    assert_eq!(eval_const(&neg(int(4))), Some(Literal::int(-4)));
}

#[test]
fn eval_const_promotes_to_float_when_either_operand_is_float() {
    assert_eq!(
        eval_const(&binary(int(1), BinaryOp::Add, float(2.0))),
        Some(Literal::Float(3.0))
    );
}

#[test]
fn eval_const_folds_division_and_modulo() {
    assert_eq!(
        eval_const(&binary(int(4), BinaryOp::Div, int(2))),
        Some(Literal::int(2))
    );
    assert_eq!(
        eval_const(&binary(int(7), BinaryOp::Mod, int(4))),
        Some(Literal::int(3))
    );
}

#[test]
fn eval_const_does_not_fold_division_or_modulo_by_zero() {
    assert_eq!(eval_const(&binary(int(1), BinaryOp::Div, int(0))), None);
    assert_eq!(eval_const(&binary(int(1), BinaryOp::Mod, int(0))), None);
}

#[test]
fn eval_const_folds_comparisons_logicals_and_string_concat() {
    assert_eq!(
        eval_const(&binary(int(1), BinaryOp::Eq, int(1))),
        Some(Literal::Bool(true))
    );
    assert_eq!(
        eval_const(&binary(
            Expr::Literal(Literal::Bool(true)),
            BinaryOp::And,
            Expr::Literal(Literal::Bool(false)),
        )),
        Some(Literal::Bool(false))
    );
    assert_eq!(
        eval_const(&binary(
            Expr::Literal(Literal::String("foo".into())),
            BinaryOp::Add,
            Expr::Literal(Literal::String("bar".into())),
        )),
        Some(Literal::String("foobar".into()))
    );
}

#[test]
fn eval_const_does_not_fold_runtime_values() {
    assert_eq!(eval_const(&Expr::Identifier("x".into())), None);
}

#[test]
fn eval_const_folds_identical_side_effect_free_subtraction_to_zero() {
    let length = member(Expr::Identifier("values".into()), "Length");
    assert_eq!(
        eval_const(&binary(length.clone(), BinaryOp::Sub, length)),
        Some(Literal::int(0))
    );
    assert_eq!(
        eval_const(&binary(
            Expr::Identifier("total".into()),
            BinaryOp::Sub,
            Expr::Identifier("total".into()),
        )),
        Some(Literal::int(0))
    );
}

#[test]
fn eval_const_does_not_fold_identical_calls_subtracted_from_themselves() {
    let get_value = call(Expr::Identifier("GetValue".into()));
    assert_eq!(
        eval_const(&binary(get_value.clone(), BinaryOp::Sub, get_value)),
        None
    );
}

#[test]
fn eval_const_does_not_fold_subtraction_of_different_members() {
    assert_eq!(
        eval_const(&binary(
            member(Expr::Identifier("values".into()), "Length"),
            BinaryOp::Sub,
            member(Expr::Identifier("other".into()), "Length"),
        )),
        None
    );
}

#[test]
fn eval_const_equality_handles_bool_none_and_incompatible_values() {
    assert_eq!(
        eval_const(&binary(
            Expr::Literal(Literal::Bool(true)),
            BinaryOp::Eq,
            Expr::Literal(Literal::Bool(false)),
        )),
        Some(Literal::Bool(false))
    );
    assert_eq!(
        eval_const(&binary(
            Expr::Literal(Literal::None),
            BinaryOp::Eq,
            Expr::Literal(Literal::None),
        )),
        Some(Literal::Bool(true))
    );
    assert_eq!(
        eval_const(&binary(Expr::Literal(Literal::None), BinaryOp::Eq, int(1),)),
        Some(Literal::Bool(false))
    );
    assert_eq!(
        eval_const(&binary(
            Expr::Literal(Literal::String("1".into())),
            BinaryOp::Eq,
            int(1),
        )),
        None
    );
}

#[test]
fn eval_const_rejects_negating_a_bool_or_adding_string_to_int() {
    assert_eq!(eval_const(&neg(Expr::Literal(Literal::Bool(true)))), None);
    assert_eq!(
        eval_const(&binary(
            Expr::Literal(Literal::String("left".into())),
            BinaryOp::Add,
            int(1),
        )),
        None
    );
}

#[test]
fn eval_const_int_folds_int_arithmetic_including_division() {
    assert_eq!(
        eval_const_int(&binary(int(1), BinaryOp::Add, int(1))),
        Some(2)
    );
    assert_eq!(
        eval_const_int(&binary(int(4), BinaryOp::Div, int(2))),
        Some(2)
    );
}

#[test]
fn eval_const_int_rejects_floats_and_bools() {
    assert_eq!(eval_const_int(&float(3.0)), None);
    assert_eq!(
        eval_const_int(&binary(int(1), BinaryOp::Add, float(1.0))),
        None
    );
    assert_eq!(eval_const_int(&binary(int(1), BinaryOp::Eq, int(1))), None);
}
