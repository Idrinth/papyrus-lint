//! Variable type tracking for a parsed Papyrus script.
//!
//! Lints that need to know a value's declared type (e.g. flagging an
//! implicit Int/Float conversion, or a condition that isn't already a
//! `Bool`) can build a [`TypeEnv`] for a script, enter each function's
//! scope with [`TypeEnv::with_function_scope`], and call [`infer_type`] on
//! the expressions they're checking.

use std::collections::HashMap;

use crate::ast::{
    BinaryOp, Expr, FunctionDecl, IfBranch, Literal, Script, Stmt, TypeName, UnaryOp,
};

/// Maps names visible at some point in a script to their declared
/// [`TypeName`].
///
/// Holds a stack of scopes: the script's properties and variables at the
/// bottom, then one scope pushed per function. Lookups search from the
/// innermost scope outward, so a local (or parameter) shadows a
/// same-named property.
#[derive(Debug, Clone)]
pub struct TypeEnv {
    self_type: TypeName,
    parent_type: Option<TypeName>,
    scopes: Vec<HashMap<String, TypeName>>,
}

impl TypeEnv {
    /// Builds the script-level scope from its properties and variables.
    pub fn for_script(script: &Script) -> Self {
        let mut scope = HashMap::new();
        for property in &script.properties {
            scope.insert(
                property.name.to_ascii_lowercase(),
                property.type_name.clone(),
            );
        }
        for variable in &script.variables {
            scope.insert(
                variable.name.to_ascii_lowercase(),
                variable.type_name.clone(),
            );
        }
        TypeEnv {
            self_type: scalar(&script.name),
            parent_type: script.extends.as_deref().map(scalar),
            scopes: vec![scope],
        }
    }

    /// Looks up `name`, innermost scope first. Matched case-insensitively,
    /// since Papyrus identifiers (like its type names) are.
    pub fn lookup(&self, name: &str) -> Option<&TypeName> {
        let name = name.to_ascii_lowercase();
        self.scopes.iter().rev().find_map(|scope| scope.get(&name))
    }

    /// Pushes a scope declaring `function`'s parameters and every local
    /// variable declared in its body (including ones nested in `If`/`While`
    /// blocks), runs `f` with that scope visible, then pops it again.
    pub fn with_function_scope<R>(
        &mut self,
        function: &FunctionDecl,
        f: impl FnOnce(&mut Self) -> R,
    ) -> R {
        let mut scope = HashMap::new();
        for param in &function.params {
            scope.insert(param.name.to_ascii_lowercase(), param.type_name.clone());
        }
        collect_locals(&function.body, &mut scope);
        self.scopes.push(scope);
        let result = f(self);
        self.scopes.pop();
        result
    }
}

fn collect_locals(body: &[Stmt], scope: &mut HashMap<String, TypeName>) {
    for stmt in body {
        match stmt {
            Stmt::VarDecl(decl) => {
                scope.insert(decl.name.to_ascii_lowercase(), decl.type_name.clone());
            }
            Stmt::If {
                branches,
                else_body,
                ..
            } => {
                for IfBranch { body, .. } in branches {
                    collect_locals(body, scope);
                }
                collect_locals(else_body, scope);
            }
            Stmt::While { body, .. } => collect_locals(body, scope),
            Stmt::Assign { .. } | Stmt::Expr { .. } | Stmt::Return { .. } => {}
        }
    }
}

fn scalar(name: &str) -> TypeName {
    TypeName {
        name: name.to_string(),
        is_array: false,
    }
}

fn is_type(type_name: &TypeName, name: &str) -> bool {
    !type_name.is_array && type_name.name.eq_ignore_ascii_case(name)
}

/// Infers the [`TypeName`] an expression evaluates to, using `env` to
/// resolve identifiers, `Self`, and `Parent`.
///
/// Returns `None` when the type can't be determined from local
/// information alone: a member access or function call, for instance,
/// depends on the type of another script that isn't tracked here.
pub fn infer_type(expr: &Expr, env: &TypeEnv) -> Option<TypeName> {
    match expr {
        Expr::Literal(Literal::Int(_)) => Some(scalar("Int")),
        Expr::Literal(Literal::Float(_)) => Some(scalar("Float")),
        Expr::Literal(Literal::String(_)) => Some(scalar("String")),
        Expr::Literal(Literal::Bool(_)) => Some(scalar("Bool")),
        Expr::Literal(Literal::None) => None,
        Expr::Identifier(name) => env.lookup(name).cloned(),
        Expr::Self_ => Some(env.self_type.clone()),
        Expr::Parent => env.parent_type.clone(),
        Expr::Unary {
            op: UnaryOp::Not, ..
        } => Some(scalar("Bool")),
        Expr::Unary {
            op: UnaryOp::Neg,
            operand,
        } => infer_type(operand, env),
        Expr::Binary {
            op:
                BinaryOp::Eq
                | BinaryOp::NotEq
                | BinaryOp::Gt
                | BinaryOp::Lt
                | BinaryOp::GtEq
                | BinaryOp::LtEq
                | BinaryOp::And
                | BinaryOp::Or,
            ..
        } => Some(scalar("Bool")),
        Expr::Binary { op, left, right } => {
            let left_ty = infer_type(left, env)?;
            let right_ty = infer_type(right, env)?;
            arithmetic_result(*op, &left_ty, &right_ty)
        }
        Expr::Cast { type_name, .. } => Some(scalar(type_name)),
        Expr::NewArray { type_name, .. } => Some(TypeName {
            name: type_name.name.clone(),
            is_array: true,
        }),
        Expr::Index { object, .. } => {
            let base = infer_type(object, env)?;
            base.is_array.then(|| scalar(&base.name))
        }
        Expr::Member { .. } | Expr::Call { .. } => None,
    }
}

/// Papyrus's numeric promotion for `+ - * / %`: an `Int` combined with a
/// `Float` promotes to `Float`; `+` between two `String`s concatenates.
/// Any other operand type (including arrays) can't be resolved here.
fn arithmetic_result(op: BinaryOp, left: &TypeName, right: &TypeName) -> Option<TypeName> {
    if op == BinaryOp::Add && (is_type(left, "string") || is_type(right, "string")) {
        return Some(scalar("String"));
    }
    let numeric = |t: &TypeName| is_type(t, "int") || is_type(t, "float");
    if !numeric(left) || !numeric(right) {
        return None;
    }
    if is_type(left, "float") || is_type(right, "float") {
        Some(scalar("Float"))
    } else {
        Some(scalar("Int"))
    }
}

#[cfg(test)]
mod tests {
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
            infer_type(&Expr::Literal(Literal::Int(1)), &env),
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
    fn comparisons_and_boolean_ops_always_yield_bool() {
        let script = parse("ScriptName Example\n").unwrap();
        let env = TypeEnv::for_script(&script);
        let cmp = Expr::Binary {
            left: Box::new(Expr::Literal(Literal::Int(1))),
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
            left: Box::new(Expr::Literal(Literal::Int(1))),
            op: BinaryOp::Add,
            right: Box::new(Expr::Literal(Literal::Int(2))),
        };
        assert_eq!(infer_type(&int_plus_int, &env), Some(scalar("Int")));

        let int_plus_float = Expr::Binary {
            left: Box::new(Expr::Literal(Literal::Int(1))),
            op: BinaryOp::Add,
            right: Box::new(Expr::Literal(Literal::Float(2.0))),
        };
        assert_eq!(infer_type(&int_plus_float, &env), Some(scalar("Float")));

        let string_concat = Expr::Binary {
            left: Box::new(Expr::Literal(Literal::String("a".to_string()))),
            op: BinaryOp::Add,
            right: Box::new(Expr::Literal(Literal::Int(2))),
        };
        assert_eq!(infer_type(&string_concat, &env), Some(scalar("String")));
    }

    #[test]
    fn cast_new_array_and_index_resolve_from_their_declared_types() {
        let script = parse("ScriptName Example\n").unwrap();
        let env = TypeEnv::for_script(&script);

        let cast = Expr::Cast {
            value: Box::new(Expr::Literal(Literal::Int(1))),
            type_name: "Float".to_string(),
        };
        assert_eq!(infer_type(&cast, &env), Some(scalar("Float")));

        let new_array = Expr::NewArray {
            type_name: scalar("Int"),
            size: Box::new(Expr::Literal(Literal::Int(5))),
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
            index: Box::new(Expr::Literal(Literal::Int(0))),
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
}
