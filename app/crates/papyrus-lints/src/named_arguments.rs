//! Prefers Papyrus's named-argument call syntax (`func(argB = 1)`) over
//! positional arguments, according to the configured [`NamedArguments`]
//! policy.
//!
//! Parameter names (and which parameters carry a default value) are only
//! known for functions declared in the script being linted, so only a call
//! resolved to a local function (a bare call, or `self.Func(...)`) is
//! checked; a call to a function declared on another script is left
//! unflagged rather than guessed at. A named argument is always accepted
//! regardless of policy — this lint only ever nudges a positional argument
//! toward the named form, never the reverse.

use std::collections::HashMap;

use papyrus_parser::ast::{Expr, FunctionDecl, IfBranch, Script, Stmt};
use serde::{Deserialize, Serialize};

use crate::Diagnostic;

/// This lint's [`Diagnostic::rule`] id, for `@disable` line comments.
pub const RULE: &str = "named-arguments";

/// How strongly this lint prefers named arguments over positional ones.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum NamedArguments {
    /// Every argument on a call resolved to a local function must be
    /// passed by name.
    Always,
    /// Only an argument filling a parameter that has a default value must
    /// be passed by name; arguments filling a required (no-default)
    /// parameter may stay positional.
    InsteadOfDefaults,
    /// No preference: positional arguments are never flagged.
    #[default]
    Never,
}

/// A declared function parameter's name and whether it has a default
/// value, as needed to resolve both positional and named arguments
/// (`func(argB = 1)`) against it.
#[derive(Clone)]
struct ParamInfo {
    name: String,
    has_default: bool,
}

/// Parameters of the functions declared in the script being linted, keyed
/// by lowercased name. A name declared more than once (e.g. overridden in
/// a state) with a differing signature (including which parameters have
/// defaults, since that decides what this lint requires) is stored as
/// `None`, since which declaration applies at a given call site can't be
/// determined here — such calls are then skipped rather than checked
/// against a possibly-wrong signature.
struct LocalFunctions {
    by_name: HashMap<String, Option<Vec<ParamInfo>>>,
}

impl LocalFunctions {
    fn from_script(script: &Script) -> Self {
        let mut grouped: HashMap<String, Vec<&FunctionDecl>> = HashMap::new();
        for function in all_functions(script) {
            grouped
                .entry(function.name.to_ascii_lowercase())
                .or_default()
                .push(function);
        }

        let by_name = grouped
            .into_iter()
            .map(|(name, decls)| {
                let first: Vec<ParamInfo> = decls[0]
                    .params
                    .iter()
                    .map(|p| ParamInfo {
                        name: p.name.clone(),
                        has_default: p.default.is_some(),
                    })
                    .collect();
                let consistent = decls.iter().all(|decl| {
                    decl.params.len() == first.len()
                        && decl.params.iter().zip(&first).all(|(p, expected)| {
                            p.name.eq_ignore_ascii_case(&expected.name)
                                && p.default.is_some() == expected.has_default
                        })
                });
                (name, consistent.then_some(first))
            })
            .collect();

        LocalFunctions { by_name }
    }

    fn lookup(&self, name: &str) -> Option<&[ParamInfo]> {
        self.by_name.get(&name.to_ascii_lowercase())?.as_deref()
    }
}

/// Iterates every function declared directly on a script, plus every
/// function declared in each of its states.
fn all_functions(script: &Script) -> impl Iterator<Item = &FunctionDecl> {
    script.functions.iter().chain(
        script
            .states
            .iter()
            .flat_map(|state| state.functions.iter()),
    )
}

/// Checks `source` for positional call arguments that `setting` prefers to
/// see passed by name instead.
pub fn check(source: &str, setting: NamedArguments) -> Vec<Diagnostic> {
    if setting == NamedArguments::Never {
        return Vec::new();
    }
    let Ok(script) = papyrus_parser::parse(source) else {
        return Vec::new();
    };

    let locals = LocalFunctions::from_script(&script);
    let mut diagnostics = Vec::new();
    for function in all_functions(&script) {
        for stmt in &function.body {
            walk_stmt(stmt, &locals, setting, &mut diagnostics);
        }
    }
    diagnostics
}

fn walk_stmt(
    stmt: &Stmt,
    locals: &LocalFunctions,
    setting: NamedArguments,
    diagnostics: &mut Vec<Diagnostic>,
) {
    match stmt {
        Stmt::VarDecl(decl) => {
            if let Some(value) = &decl.value {
                walk_expr(value, locals, setting, diagnostics);
            }
        }
        Stmt::Assign { target, value, .. } => {
            walk_expr(target, locals, setting, diagnostics);
            walk_expr(value, locals, setting, diagnostics);
        }
        Stmt::Expr { value, .. } => walk_expr(value, locals, setting, diagnostics),
        Stmt::Return {
            value: Some(value), ..
        } => walk_expr(value, locals, setting, diagnostics),
        Stmt::Return { value: None, .. } => {}
        Stmt::If {
            branches,
            else_body,
            ..
        } => {
            for IfBranch {
                condition, body, ..
            } in branches
            {
                walk_expr(condition, locals, setting, diagnostics);
                for inner in body {
                    walk_stmt(inner, locals, setting, diagnostics);
                }
            }
            for inner in else_body {
                walk_stmt(inner, locals, setting, diagnostics);
            }
        }
        Stmt::While {
            condition, body, ..
        } => {
            walk_expr(condition, locals, setting, diagnostics);
            for inner in body {
                walk_stmt(inner, locals, setting, diagnostics);
            }
        }
    }
}

fn walk_expr(
    expr: &Expr,
    locals: &LocalFunctions,
    setting: NamedArguments,
    diagnostics: &mut Vec<Diagnostic>,
) {
    match expr {
        Expr::Call {
            callee,
            args,
            line,
            col,
        } => {
            if let Some((name, params)) = resolve_local(callee, locals) {
                check_call(*line, *col, &name, params, args, setting, diagnostics);
            }
            walk_expr(callee, locals, setting, diagnostics);
            for arg in args {
                walk_expr(arg, locals, setting, diagnostics);
            }
        }
        Expr::Binary { left, right, .. } => {
            walk_expr(left, locals, setting, diagnostics);
            walk_expr(right, locals, setting, diagnostics);
        }
        Expr::Unary { operand, .. } => walk_expr(operand, locals, setting, diagnostics),
        Expr::Member { object, .. } => walk_expr(object, locals, setting, diagnostics),
        Expr::Index { object, index } => {
            walk_expr(object, locals, setting, diagnostics);
            walk_expr(index, locals, setting, diagnostics);
        }
        Expr::Cast { value, .. } => walk_expr(value, locals, setting, diagnostics),
        Expr::NewArray { size, .. } => walk_expr(size, locals, setting, diagnostics),
        Expr::NamedArg { value, .. } => walk_expr(value, locals, setting, diagnostics),
        Expr::Literal(_) | Expr::Identifier(_) | Expr::Self_ | Expr::Parent => {}
    }
}

/// Resolves `callee` to a local function's name and parameters: either a
/// bare call (`Func(...)`) or one explicitly qualified with `self`
/// (`self.Func(...)`). A call on anything else (another script's property,
/// `Parent`, an array element, ...) can't be resolved to a known parameter
/// list here and is left unchecked.
fn resolve_local<'a>(
    callee: &Expr,
    locals: &'a LocalFunctions,
) -> Option<(String, &'a [ParamInfo])> {
    match callee {
        Expr::Identifier(name) => locals.lookup(name).map(|params| (name.clone(), params)),
        Expr::Member { object, property } if matches!(**object, Expr::Self_) => locals
            .lookup(property)
            .map(|params| (property.clone(), params)),
        _ => None,
    }
}

fn check_call(
    line: usize,
    col: usize,
    function_name: &str,
    params: &[ParamInfo],
    args: &[Expr],
    setting: NamedArguments,
    diagnostics: &mut Vec<Diagnostic>,
) {
    for (index, arg) in args.iter().enumerate() {
        if matches!(arg, Expr::NamedArg { .. }) {
            continue;
        }
        let Some(param) = params.get(index) else {
            break;
        };
        let should_flag = match setting {
            NamedArguments::Always => true,
            NamedArguments::InsteadOfDefaults => param.has_default,
            NamedArguments::Never => false,
        };
        if !should_flag {
            continue;
        }

        diagnostics.push(Diagnostic {
            line,
            column: col,
            message: format!(
                "[warning] Argument {} to '{}' should be passed as a named argument ({} = ...)",
                index + 1,
                function_name,
                param.name
            ),
            rule: RULE,
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn never_setting_flags_nothing() {
        let diagnostics = check(
            "ScriptName Example\n\nFunction Greet(String name)\nEndFunction\n\nFunction Test()\n    Greet(\"hi\")\nEndFunction\n",
            NamedArguments::Never,
        );

        assert!(diagnostics.is_empty());
    }

    #[test]
    fn always_flags_a_positional_argument_to_a_local_function() {
        let diagnostics = check(
            "ScriptName Example\n\nFunction Greet(String name)\nEndFunction\n\nFunction Test()\n    Greet(\"hi\")\nEndFunction\n",
            NamedArguments::Always,
        );

        assert_eq!(diagnostics.len(), 1);
        assert_eq!(diagnostics[0].line, 7);
        assert_eq!(diagnostics[0].rule, RULE);
        assert!(diagnostics[0].message.starts_with("[warning]"));
        assert!(diagnostics[0].message.contains("Argument 1 to 'Greet'"));
        assert!(diagnostics[0].message.contains("name = ..."));
    }

    #[test]
    fn always_does_not_flag_an_already_named_argument() {
        let diagnostics = check(
            "ScriptName Example\n\nFunction Greet(String name)\nEndFunction\n\nFunction Test()\n    Greet(name = \"hi\")\nEndFunction\n",
            NamedArguments::Always,
        );

        assert!(diagnostics.is_empty());
    }

    #[test]
    fn always_flags_every_positional_argument_including_required_ones() {
        let diagnostics = check(
            "ScriptName Example\n\nFunction MyFunction(Int argA, Int argB = 0)\nEndFunction\n\nFunction Test()\n    MyFunction(1, 2)\nEndFunction\n",
            NamedArguments::Always,
        );

        assert_eq!(diagnostics.len(), 2);
        assert!(diagnostics.iter().any(|d| d.message.contains("argA")));
        assert!(diagnostics.iter().any(|d| d.message.contains("argB")));
    }

    #[test]
    fn instead_of_defaults_only_flags_arguments_with_a_default_value() {
        let diagnostics = check(
            "ScriptName Example\n\nFunction MyFunction(Int argA, Int argB = 0)\nEndFunction\n\nFunction Test()\n    MyFunction(1, 2)\nEndFunction\n",
            NamedArguments::InsteadOfDefaults,
        );

        assert_eq!(diagnostics.len(), 1);
        assert!(diagnostics[0]
            .message
            .contains("Argument 2 to 'MyFunction'"));
        assert!(diagnostics[0].message.contains("argB = ..."));
    }

    #[test]
    fn instead_of_defaults_accepts_a_named_argument_for_a_default_parameter() {
        let diagnostics = check(
            "ScriptName Example\n\nFunction MyFunction(Int argA, Int argB = 0)\nEndFunction\n\nFunction Test()\n    MyFunction(1, argB = 2)\nEndFunction\n",
            NamedArguments::InsteadOfDefaults,
        );

        assert!(diagnostics.is_empty());
    }

    #[test]
    fn resolves_self_qualified_calls_the_same_way() {
        let diagnostics = check(
            "ScriptName Example\n\nFunction Greet(String name)\nEndFunction\n\nFunction Test()\n    self.Greet(\"hi\")\nEndFunction\n",
            NamedArguments::Always,
        );

        assert_eq!(diagnostics.len(), 1);
        assert!(diagnostics[0].message.contains("Greet"));
    }

    #[test]
    fn does_not_flag_calls_to_functions_declared_on_other_scripts() {
        let diagnostics = check(
            "ScriptName Example\n\nFunction Test(Actor akActor)\n    akActor.MoveTo(1, 2, 3)\nEndFunction\n",
            NamedArguments::Always,
        );

        assert!(diagnostics.is_empty());
    }

    #[test]
    fn does_not_flag_calls_beyond_the_declared_parameter_count() {
        let diagnostics = check(
            "ScriptName Example\n\nFunction Greet(String name)\nEndFunction\n\nFunction Test()\n    Greet(\"hi\", 1)\nEndFunction\n",
            NamedArguments::Always,
        );

        assert_eq!(diagnostics.len(), 1);
    }

    #[test]
    fn skips_ambiguous_overrides_across_states() {
        let diagnostics = check(
            "ScriptName Example\n\nFunction Greet(String name)\nEndFunction\n\nState Loud\n    Function Greet(Int volume)\n    EndFunction\nEndState\n\nFunction Test()\n    Greet(\"hi\")\nEndFunction\n",
            NamedArguments::Always,
        );

        assert!(diagnostics.is_empty());
    }

    #[test]
    fn checks_functions_declared_inside_a_state_too() {
        let diagnostics = check(
            "ScriptName Example\n\nState Active\n    Function Greet(String name)\n    EndFunction\n\n    Function Test()\n        Greet(\"hi\")\n    EndFunction\nEndState\n",
            NamedArguments::Always,
        );

        assert_eq!(diagnostics.len(), 1);
    }

    #[test]
    fn flags_a_positional_argument_nested_inside_another_call() {
        let diagnostics = check(
            "ScriptName Example\n\nFunction Greet(String name)\nEndFunction\n\nFunction Test()\n    Debug.Trace(Greet(\"hi\"))\nEndFunction\n",
            NamedArguments::Always,
        );

        assert_eq!(diagnostics.len(), 1);
        assert!(diagnostics[0].message.contains("Greet"));
    }

    #[test]
    fn does_not_crash_on_unparseable_source() {
        let diagnostics = check(
            "ScriptName Example\n\nFunction Test(\nEndFunction\n",
            NamedArguments::Always,
        );
        assert!(diagnostics.is_empty());
    }
}
