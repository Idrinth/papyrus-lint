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
fn eval_const_does_not_fold_division_or_runtime_values() {
    assert_eq!(eval_const(&binary(int(4), BinaryOp::Div, int(2))), None);
    assert_eq!(eval_const(&Expr::Identifier("x".into())), None);
}

#[test]
fn eval_const_int_folds_int_arithmetic_and_rejects_floats() {
    assert_eq!(
        eval_const_int(&binary(int(1), BinaryOp::Add, int(1))),
        Some(2)
    );
    assert_eq!(eval_const_int(&float(3.0)), None);
    assert_eq!(
        eval_const_int(&binary(int(1), BinaryOp::Add, float(1.0))),
        None
    );
}
