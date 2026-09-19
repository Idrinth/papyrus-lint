//! Shared AST walk for lints that inspect values flowing into typed slots.

use std::collections::HashMap;

use papyrus_parser::ast::{Expr, FunctionDecl, PropertyDecl, Script, Stmt, TypeName, VariableDecl};
use papyrus_parser::types::{infer_type, TypeEnv};

use crate::visitor::{AstLint, LintVisitor, Store, VisitCtx};

pub(crate) enum Slot<'a> {
    Declaration {
        name: &'a str,
    },
    Assignment {
        target: &'a Expr,
    },
    Return {
        function: &'a str,
    },
    Argument {
        parameter: &'a str,
        function: &'a str,
    },
}

pub(crate) trait TypeFlowLint: Default {
    fn check(
        &mut self,
        target_type: &TypeName,
        value: &Expr,
        env: &TypeEnv,
        slot: Slot<'_>,
        line: usize,
        store: &mut Store,
    );
}

struct IndexedFunction {
    name: String,
    params: Vec<(String, TypeName)>,
}

struct Collect<L> {
    lint: L,
    store: Store,
    env: Option<TypeEnv>,
    functions: HashMap<String, IndexedFunction>,
    return_type: Option<TypeName>,
    function_name: String,
}

impl<L: Default> Default for Collect<L> {
    fn default() -> Self {
        Self {
            lint: L::default(),
            store: Store::default(),
            env: None,
            functions: HashMap::new(),
            return_type: None,
            function_name: String::new(),
        }
    }
}

impl<L: TypeFlowLint> AstLint for Collect<L> {
    fn store(&mut self) -> &mut Store {
        &mut self.store
    }

    fn visit_script(&mut self, script: &Script, _ctx: &mut VisitCtx<'_>) {
        self.env = Some(TypeEnv::for_script(script));
        self.functions = index_functions(script);
    }

    fn visit_function(&mut self, function: &FunctionDecl, _ctx: &mut VisitCtx<'_>) {
        if let Some(env) = &mut self.env {
            env.enter_function(function);
        }
        self.return_type = function.return_type.clone();
        self.function_name = function.name.clone();
    }

    fn leave_function(&mut self, _function: &FunctionDecl, _ctx: &mut VisitCtx<'_>) {
        if let Some(env) = &mut self.env {
            env.leave_function();
        }
        self.return_type = None;
        self.function_name.clear();
    }

    fn visit_property(&mut self, property: &PropertyDecl, _ctx: &mut VisitCtx<'_>) {
        if let (Some(value), Some(env)) = (&property.value, self.env.as_ref()) {
            self.lint.check(
                &property.type_name,
                value,
                env,
                Slot::Declaration {
                    name: &property.name,
                },
                property.line,
                &mut self.store,
            );
        }
    }

    fn visit_variable(&mut self, variable: &VariableDecl, _ctx: &mut VisitCtx<'_>) {
        if let (Some(value), Some(env)) = (&variable.value, self.env.as_ref()) {
            self.lint.check(
                &variable.type_name,
                value,
                env,
                Slot::Declaration {
                    name: &variable.name,
                },
                variable.line,
                &mut self.store,
            );
        }
    }

    fn visit_stmt(&mut self, stmt: &Stmt, _ctx: &mut VisitCtx<'_>) {
        let Some(env) = self.env.as_ref() else {
            return;
        };
        match stmt {
            Stmt::Assign {
                target,
                value,
                line,
                ..
            } => {
                if let Some(target_type) = infer_type(target, env) {
                    self.lint.check(
                        &target_type,
                        value,
                        env,
                        Slot::Assignment { target },
                        *line,
                        &mut self.store,
                    );
                }
            }
            Stmt::Return {
                value: Some(value),
                line,
            } => {
                if let Some(return_type) = &self.return_type {
                    self.lint.check(
                        return_type,
                        value,
                        env,
                        Slot::Return {
                            function: &self.function_name,
                        },
                        *line,
                        &mut self.store,
                    );
                }
            }
            _ => {}
        }
    }

    fn visit_expr(&mut self, expr: &Expr, ctx: &mut VisitCtx<'_>) {
        let Expr::Call { callee, args, .. } = expr else {
            return;
        };
        let Some(env) = self.env.as_ref() else {
            return;
        };
        let Some(name) = local_call_name(callee) else {
            return;
        };
        let Some(function) = self.functions.get(&name.to_lowercase()) else {
            return;
        };
        for (index, arg) in args.iter().enumerate() {
            let (value, parameter, target_type) = match arg {
                Expr::NamedArg { name, value } => {
                    let Some((parameter, target_type)) = function
                        .params
                        .iter()
                        .find(|(parameter, _)| parameter.eq_ignore_ascii_case(name))
                    else {
                        continue;
                    };
                    (value.as_ref(), parameter.as_str(), target_type)
                }
                _ => {
                    let Some((parameter, target_type)) = function.params.get(index) else {
                        break;
                    };
                    (arg, parameter.as_str(), target_type)
                }
            };
            self.lint.check(
                target_type,
                value,
                env,
                Slot::Argument {
                    parameter,
                    function: &function.name,
                },
                ctx.line,
                &mut self.store,
            );
        }
    }
}

pub(crate) fn visitor<L: TypeFlowLint + 'static>() -> LintVisitor {
    LintVisitor::Ast(Box::new(Collect::<L>::default()))
}

fn local_call_name(callee: &Expr) -> Option<&str> {
    match callee {
        Expr::Identifier(name) => Some(name),
        Expr::Member { object, property } if matches!(**object, Expr::Self_) => Some(property),
        _ => None,
    }
}

fn index_functions(script: &Script) -> HashMap<String, IndexedFunction> {
    let mut functions = HashMap::new();
    for function in script
        .functions
        .iter()
        .chain(script.states.iter().flat_map(|state| &state.functions))
    {
        functions
            .entry(function.name.to_lowercase())
            .or_insert_with(|| IndexedFunction {
                name: function.name.clone(),
                params: function
                    .params
                    .iter()
                    .map(|param| (param.name.clone(), param.type_name.clone()))
                    .collect(),
            });
    }
    functions
}
