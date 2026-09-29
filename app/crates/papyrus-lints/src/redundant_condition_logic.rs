fn check_condition(condition: &Expr, line: usize, column: usize, store: &mut Store) {
    walk_logic(condition, None, line, column, store);
}

fn walk_logic(
    expr: &Expr,
    parent_op: Option<BinaryOp>,
    line: usize,
    column: usize,
    store: &mut Store,
) {
    match expr {
        Expr::Binary { left, op, right } if matches!(op, BinaryOp::And | BinaryOp::Or) => {
            if parent_op != Some(*op) {
                analyze_chain(expr, *op, line, column, store);
            }
            walk_logic(left, Some(*op), line, column, store);
            walk_logic(right, Some(*op), line, column, store);
        }
        Expr::Binary { left, right, .. } => {
            walk_logic(left, None, line, column, store);
            walk_logic(right, None, line, column, store);
        }
        Expr::Unary { operand, .. }
        | Expr::Member { object: operand, .. }
        | Expr::Cast { value: operand, .. }
        | Expr::Is { value: operand, .. } => {
            walk_logic(operand, None, line, column, store);
        }
        Expr::Index { object, index } => {
            walk_logic(object, None, line, column, store);
            walk_logic(index, None, line, column, store);
        }
        Expr::Call { callee, args, .. } => {
            walk_logic(callee, None, line, column, store);
            for arg in args {
                walk_logic(arg, None, line, column, store);
            }
        }
        Expr::NamedArg { value, .. } => walk_logic(value, None, line, column, store),
        Expr::Literal(_)
        | Expr::Identifier(_)
        | Expr::Self_
        | Expr::Parent
        | Expr::NewArray { .. }
        | Expr::NewStruct { .. } => {}
    }
}

fn analyze_chain(expr: &Expr, op: BinaryOp, line: usize, column: usize, store: &mut Store) {
    let clauses = collect_clauses(expr, op);
    if clauses.len() < 2 {
        return;
    }

    let mut flagged = vec![false; clauses.len()];
    for i in 0..clauses.len() {
        if flagged[i] {
            continue;
        }
        for j in (i + 1)..clauses.len() {
            if flagged[j] {
                continue;
            }
            // Prefer reporting the later clause when either direction applies
            // (covers duplicates, where both are redundant given the other).
            if is_redundant_given(clauses[j], clauses[i], op) {
                flagged[j] = true;
                store.emit(
                    line,
                    column,
                    "[warning] Condition clause is redundant given another clause on the same identifier",
                    RULE,
                );
            } else if is_redundant_given(clauses[i], clauses[j], op) {
                flagged[i] = true;
                store.emit(
                    line,
                    column,
                    "[warning] Condition clause is redundant given another clause on the same identifier",
                    RULE,
                );
                break;
            }
        }
    }
}

fn collect_clauses(expr: &Expr, op: BinaryOp) -> Vec<&Expr> {
    match expr {
        Expr::Binary {
            left,
            op: chain_op,
            right,
        } if *chain_op == op => {
            let mut clauses = collect_clauses(left, op);
            clauses.extend(collect_clauses(right, op));
            clauses
        }
        _ => vec![expr],
    }
}

fn is_redundant_given(candidate: &Expr, other: &Expr, op: BinaryOp) -> bool {
    if identical_simple_refs(candidate, other) {
        return true;
    }

    let Some((lhs_c, op_c, value_c)) = extract_comparison(candidate) else {
        return false;
    };
    let Some((lhs_o, op_o, value_o)) = extract_comparison(other) else {
        return false;
    };
    if !same_identifier(lhs_c, lhs_o) {
        return false;
    }
    let Some(interval_c) = Interval::from_comparison(op_c, value_c) else {
        return false;
    };
    let Some(interval_o) = Interval::from_comparison(op_o, value_o) else {
        return false;
    };

    match op {
        BinaryOp::And => interval_o.is_subset_of(&interval_c),
        BinaryOp::Or => interval_c.is_subset_of(&interval_o),
        _ => false,
    }
}

fn identical_simple_refs(left: &Expr, right: &Expr) -> bool {
    same_identifier(left, right) && is_simple_ref(left) && is_simple_ref(right)
}

fn is_simple_ref(expr: &Expr) -> bool {
    match expr {
        Expr::Identifier(_) => true,
        Expr::Member { object, .. } => matches!(
            object.as_ref(),
            Expr::Identifier(_) | Expr::Self_ | Expr::Parent
        ),
        _ => false,
    }
}

fn is_ident_or_property(expr: &Expr) -> bool {
    is_simple_ref(expr)
}

fn same_identifier(left: &Expr, right: &Expr) -> bool {
    match (left, right) {
        (Expr::Identifier(a), Expr::Identifier(b)) => a.eq_ignore_ascii_case(b),
        (
            Expr::Member {
                object: lo,
                property: lp,
            },
            Expr::Member {
                object: ro,
                property: rp,
            },
        ) => lp.eq_ignore_ascii_case(rp) && same_identifier_object(lo, ro),
        _ => false,
    }
}

fn same_identifier_object(left: &Expr, right: &Expr) -> bool {
    match (left, right) {
        (Expr::Self_, Expr::Self_) | (Expr::Parent, Expr::Parent) => true,
        (Expr::Identifier(a), Expr::Identifier(b)) => a.eq_ignore_ascii_case(b),
        _ => false,
    }
}

fn extract_comparison(expr: &Expr) -> Option<(&Expr, BinaryOp, f64)> {
    let Expr::Binary { left, op, right } = expr else {
        return None;
    };
    if !matches!(
        op,
        BinaryOp::Eq | BinaryOp::Gt | BinaryOp::Lt | BinaryOp::GtEq | BinaryOp::LtEq
    ) {
        return None;
    }

    match (numeric_literal(left), numeric_literal(right)) {
        (None, Some(value)) if is_ident_or_property(left) => Some((left, *op, value)),
        (Some(value), None) if is_ident_or_property(right) => Some((right, flip(*op), value)),
        _ => None,
    }
}

fn numeric_literal(expr: &Expr) -> Option<f64> {
    match expr {
        Expr::Literal(Literal::Int { value, .. }) => Some(*value as f64),
        Expr::Literal(Literal::Float(value)) => Some(*value),
        Expr::Unary {
            op: UnaryOp::Neg,
            operand,
        } => numeric_literal(operand).map(|value| -value),
        _ => None,
    }
}

fn flip(op: BinaryOp) -> BinaryOp {
    match op {
        BinaryOp::Gt => BinaryOp::Lt,
        BinaryOp::Lt => BinaryOp::Gt,
        BinaryOp::GtEq => BinaryOp::LtEq,
        BinaryOp::LtEq => BinaryOp::GtEq,
        other => other,
    }
}

#[derive(Clone, Copy)]
struct Bound {
    value: f64,
    inclusive: bool,
}

#[derive(Clone, Copy)]
struct Interval {
    low: Option<Bound>,
    high: Option<Bound>,
}

impl Interval {
    fn from_comparison(op: BinaryOp, value: f64) -> Option<Self> {
        match op {
            BinaryOp::Gt => Some(Self {
                low: Some(Bound {
                    value,
                    inclusive: false,
                }),
                high: None,
            }),
            BinaryOp::GtEq => Some(Self {
                low: Some(Bound {
                    value,
                    inclusive: true,
                }),
                high: None,
            }),
            BinaryOp::Lt => Some(Self {
                low: None,
                high: Some(Bound {
                    value,
                    inclusive: false,
                }),
            }),
            BinaryOp::LtEq => Some(Self {
                low: None,
                high: Some(Bound {
                    value,
                    inclusive: true,
                }),
            }),
            BinaryOp::Eq => Some(Self {
                low: Some(Bound {
                    value,
                    inclusive: true,
                }),
                high: Some(Bound {
                    value,
                    inclusive: true,
                }),
            }),
            _ => None,
        }
    }

    fn is_subset_of(&self, other: &Self) -> bool {
        low_rank(self.low) >= low_rank(other.low) && high_rank(self.high) <= high_rank(other.high)
    }
}

fn low_rank(bound: Option<Bound>) -> (f64, i8) {
    match bound {
        None => (f64::NEG_INFINITY, 0),
        Some(Bound { value, inclusive }) => (value, if inclusive { 0 } else { 1 }),
    }
}

fn high_rank(bound: Option<Bound>) -> (f64, i8) {
    match bound {
        None => (f64::INFINITY, 0),
        Some(Bound { value, inclusive }) => (value, if inclusive { 1 } else { 0 }),
    }
}
