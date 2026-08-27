//! Flags call arguments whose inferred type doesn't match the callee's
//! declared parameter type.
//!
//! Unlike the other lints in this crate, this one works from the parsed
//! AST (via [`papyrus_parser::types`]) rather than raw tokens, since it
//! needs to know declared parameter types to say anything useful. A call
//! whose target or argument type can't be determined from the script
//! alone (an unqualified type, a member access on an unresolved object, an
//! expression this crate doesn't model) is silently skipped rather than
//! guessed at, to keep false positives rare.
//!
//! Calls to functions declared in the script being linted are always
//! checked (see [`check`]). Calls to functions declared on *other*
//! scripts (e.g. `SomeProperty.DoThing(...)`) additionally need those
//! scripts' signatures, which requires resolving script names to files —
//! something this crate deliberately has no filesystem access to do. A
//! caller that can supply such signatures (e.g. the Tauri app, backed by
//! its `FunctionTable`) can do so by implementing [`ExternalSignatures`]
//! and calling [`check_with`] instead.

use std::collections::HashMap;

use papyrus_parser::ast::{Expr, FunctionDecl, IfBranch, Literal, Script, Stmt, TypeName};
use papyrus_parser::types::{infer_type, TypeEnv};

use crate::Diagnostic;

/// Resolves the parameter types of a function declared on some other
/// script, for callers that can look such scripts up (see the module
/// docs). Both names are matched case-insensitively; returning `None`
/// means the function couldn't be resolved and the call site is skipped.
pub trait ExternalSignatures {
    fn lookup(&mut self, type_name: &str, function_name: &str) -> Option<Vec<TypeName>>;

    /// Whether `sub_type` inherits from `super_type`, directly or
    /// transitively (i.e. `sub_type`'s script, or one of its ancestors'
    /// via `Extends`, is named `super_type`). Used so e.g. an `Armor`
    /// argument is accepted for a `Form` parameter.
    ///
    /// The default always says no, which keeps existing behavior for
    /// callers that can't resolve scripts (see [`NoExternalSignatures`]).
    fn is_subtype(&mut self, _sub_type: &str, _super_type: &str) -> bool {
        false
    }
}

/// An [`ExternalSignatures`] that never resolves anything, for checking a
/// single script in isolation (see [`check`]).
pub struct NoExternalSignatures;

impl ExternalSignatures for NoExternalSignatures {
    fn lookup(&mut self, _type_name: &str, _function_name: &str) -> Option<Vec<TypeName>> {
        None
    }
}

/// Checks `source` for argument/parameter type mismatches on calls to
/// functions declared in the same script. Calls on other scripts' types
/// are not checked; see [`check_with`] for that.
pub fn check(source: &str) -> Vec<Diagnostic> {
    check_with(source, &mut NoExternalSignatures)
}

/// Like [`check`], but also checks calls to functions resolved through
/// `external` (typically functions declared on other scripts).
pub fn check_with<E: ExternalSignatures>(source: &str, external: &mut E) -> Vec<Diagnostic> {
    let Ok(script) = papyrus_parser::parse(source) else {
        return Vec::new();
    };

    let locals = LocalFunctions::from_script(&script);
    let mut env = TypeEnv::for_script(&script);
    let mut diagnostics = Vec::new();

    for function in all_functions(&script) {
        env.with_function_scope(function, |env| {
            for stmt in &function.body {
                walk_stmt(stmt, env, &locals, external, &mut diagnostics);
            }
        });
    }

    diagnostics
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

/// Parameter types of the functions declared in the script being linted,
/// keyed by lowercased name. A name declared more than once (e.g.
/// overridden in a state) with differing signatures is stored as `None`,
/// since which declaration applies at a given call site can't be
/// determined here — such calls are then skipped rather than checked
/// against a possibly-wrong signature.
struct LocalFunctions {
    by_name: HashMap<String, Option<Vec<TypeName>>>,
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
                let first: Vec<TypeName> = decls[0]
                    .params
                    .iter()
                    .map(|p| p.type_name.clone())
                    .collect();
                let consistent = decls
                    .iter()
                    .all(|decl| decl.params.iter().map(|p| &p.type_name).eq(first.iter()));
                (name, consistent.then_some(first))
            })
            .collect();

        LocalFunctions { by_name }
    }

    fn lookup(&self, name: &str) -> Option<&[TypeName]> {
        self.by_name.get(&name.to_ascii_lowercase())?.as_deref()
    }
}

fn walk_stmt<E: ExternalSignatures>(
    stmt: &Stmt,
    env: &TypeEnv,
    locals: &LocalFunctions,
    external: &mut E,
    diagnostics: &mut Vec<Diagnostic>,
) {
    match stmt {
        Stmt::VarDecl(decl) => {
            if let Some(value) = &decl.value {
                walk_expr(value, env, locals, external, diagnostics);
            }
        }
        Stmt::Assign { target, value, .. } => {
            walk_expr(target, env, locals, external, diagnostics);
            walk_expr(value, env, locals, external, diagnostics);
        }
        Stmt::Expr { value, .. } => walk_expr(value, env, locals, external, diagnostics),
        Stmt::Return { value, .. } => {
            if let Some(value) = value {
                walk_expr(value, env, locals, external, diagnostics);
            }
        }
        Stmt::If {
            branches,
            else_body,
            ..
        } => {
            for IfBranch {
                condition, body, ..
            } in branches
            {
                walk_expr(condition, env, locals, external, diagnostics);
                for stmt in body {
                    walk_stmt(stmt, env, locals, external, diagnostics);
                }
            }
            for stmt in else_body {
                walk_stmt(stmt, env, locals, external, diagnostics);
            }
        }
        Stmt::While {
            condition, body, ..
        } => {
            walk_expr(condition, env, locals, external, diagnostics);
            for stmt in body {
                walk_stmt(stmt, env, locals, external, diagnostics);
            }
        }
    }
}

fn walk_expr<E: ExternalSignatures>(
    expr: &Expr,
    env: &TypeEnv,
    locals: &LocalFunctions,
    external: &mut E,
    diagnostics: &mut Vec<Diagnostic>,
) {
    match expr {
        Expr::Call {
            callee,
            args,
            line,
            col,
        } => {
            if let Some((name, param_types)) = resolve_signature(callee, env, locals, external) {
                check_args(
                    *line,
                    *col,
                    &name,
                    &param_types,
                    args,
                    env,
                    external,
                    diagnostics,
                );
            }
            walk_expr(callee, env, locals, external, diagnostics);
            for arg in args {
                walk_expr(arg, env, locals, external, diagnostics);
            }
        }
        Expr::Binary { left, right, .. } => {
            walk_expr(left, env, locals, external, diagnostics);
            walk_expr(right, env, locals, external, diagnostics);
        }
        Expr::Unary { operand, .. } => walk_expr(operand, env, locals, external, diagnostics),
        Expr::Member { object, .. } => walk_expr(object, env, locals, external, diagnostics),
        Expr::Index { object, index } => {
            walk_expr(object, env, locals, external, diagnostics);
            walk_expr(index, env, locals, external, diagnostics);
        }
        Expr::Cast { value, .. } => walk_expr(value, env, locals, external, diagnostics),
        Expr::NewArray { size, .. } => walk_expr(size, env, locals, external, diagnostics),
        Expr::Literal(_) | Expr::Identifier(_) | Expr::Self_ | Expr::Parent => {}
    }
}

/// Resolves `callee` to a function name and its parameter types, checking
/// the script's own functions first and falling back to `external` for
/// anything that isn't a local call (or isn't declared locally).
fn resolve_signature<E: ExternalSignatures>(
    callee: &Expr,
    env: &TypeEnv,
    locals: &LocalFunctions,
    external: &mut E,
) -> Option<(String, Vec<TypeName>)> {
    let (object_type, function_name) = match callee {
        Expr::Identifier(name) => {
            if let Some(params) = locals.lookup(name) {
                return Some((name.clone(), params.to_vec()));
            }
            (infer_type(&Expr::Self_, env)?, name.clone())
        }
        Expr::Member { object, property } => {
            if matches!(**object, Expr::Self_) {
                if let Some(params) = locals.lookup(property) {
                    return Some((property.clone(), params.to_vec()));
                }
            }
            (infer_type(object, env)?, property.clone())
        }
        _ => return None,
    };

    if object_type.is_array {
        return None;
    }
    external
        .lookup(&object_type.name, &function_name)
        .map(|params| (function_name, params))
}

fn check_args<E: ExternalSignatures>(
    line: usize,
    col: usize,
    function_name: &str,
    param_types: &[TypeName],
    args: &[Expr],
    env: &TypeEnv,
    external: &mut E,
    diagnostics: &mut Vec<Diagnostic>,
) {
    for (index, arg) in args.iter().enumerate() {
        let Some(param_type) = param_types.get(index) else {
            break;
        };

        if matches!(arg, Expr::Literal(Literal::None)) {
            if !accepts_none(param_type) {
                diagnostics.push(mismatch(
                    line,
                    col,
                    function_name,
                    index,
                    param_type,
                    "None",
                ));
            }
            continue;
        }

        let Some(arg_type) = infer_type(arg, env) else {
            continue;
        };
        if !is_compatible(param_type, &arg_type, external) {
            diagnostics.push(mismatch(
                line,
                col,
                function_name,
                index,
                param_type,
                &format_type(&arg_type),
            ));
        }
    }
}

fn mismatch(
    line: usize,
    col: usize,
    function_name: &str,
    index: usize,
    param_type: &TypeName,
    got: &str,
) -> Diagnostic {
    Diagnostic {
        line,
        column: col,
        message: format!(
            "Argument {} to '{}' expects {} but got {}",
            index + 1,
            function_name,
            format_type(param_type),
            got
        ),
    }
}

fn format_type(type_name: &TypeName) -> String {
    if type_name.is_array {
        format!("{}[]", type_name.name)
    } else {
        type_name.name.clone()
    }
}

fn is_numeric(name: &str) -> bool {
    name.eq_ignore_ascii_case("int") || name.eq_ignore_ascii_case("float")
}

fn is_primitive(name: &str) -> bool {
    is_numeric(name) || name.eq_ignore_ascii_case("bool") || name.eq_ignore_ascii_case("string")
}

/// `None` is valid for any reference type (arrays and non-primitive
/// object types) but not for `Int`/`Float`/`Bool`/`String`.
fn accepts_none(param_type: &TypeName) -> bool {
    param_type.is_array || !is_primitive(&param_type.name)
}

/// Whether an argument of type `arg_type` may be passed for a parameter
/// declared as `param_type`. Exact matches (case-insensitively) are
/// always compatible; Papyrus also allows widening an `Int` argument to a
/// `Float` parameter, and passing an object whose script extends (directly
/// or transitively) the parameter's type, per `external`'s knowledge of
/// the scripts' `Extends` chains.
fn is_compatible<E: ExternalSignatures>(
    param_type: &TypeName,
    arg_type: &TypeName,
    external: &mut E,
) -> bool {
    if param_type.is_array != arg_type.is_array {
        return false;
    }
    if param_type.name.eq_ignore_ascii_case(&arg_type.name) {
        return true;
    }
    if !param_type.is_array
        && param_type.name.eq_ignore_ascii_case("float")
        && arg_type.name.eq_ignore_ascii_case("int")
    {
        return true;
    }
    if param_type.is_array || is_primitive(&param_type.name) || is_primitive(&arg_type.name) {
        return false;
    }
    external.is_subtype(&arg_type.name, &param_type.name)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn flags_mismatched_literal_argument_to_local_function() {
        let diagnostics = check(
            "ScriptName Example\n\nFunction Greet(String name)\nEndFunction\n\nFunction Test()\n    Greet(1)\nEndFunction\n",
        );

        assert_eq!(diagnostics.len(), 1);
        assert_eq!(diagnostics[0].line, 7);
        assert!(diagnostics[0].message.contains("Argument 1 to 'Greet'"));
        assert!(diagnostics[0].message.contains("expects String"));
        assert!(diagnostics[0].message.contains("got Int"));
    }

    #[test]
    fn allows_int_argument_for_float_parameter() {
        let diagnostics = check(
            "ScriptName Example\n\nFunction SetSpeed(Float speed)\nEndFunction\n\nFunction Test()\n    SetSpeed(1)\nEndFunction\n",
        );

        assert!(diagnostics.is_empty());
    }

    #[test]
    fn flags_float_argument_for_int_parameter() {
        let diagnostics = check(
            "ScriptName Example\n\nFunction SetCount(Int count)\nEndFunction\n\nFunction Test()\n    SetCount(1.5)\nEndFunction\n",
        );

        assert_eq!(diagnostics.len(), 1);
        assert!(diagnostics[0].message.contains("expects Int"));
        assert!(diagnostics[0].message.contains("got Float"));
    }

    #[test]
    fn checks_variables_properties_and_casts_by_declared_type() {
        let diagnostics = check(
            r#"
ScriptName Example

Bool Property Enabled Auto

Function Configure(Bool flag)
EndFunction

Function Test()
    Int value = 5
    Configure(value)
    Configure(Enabled)
    Configure(value as Bool)
EndFunction
"#,
        );

        assert_eq!(diagnostics.len(), 1);
        assert!(diagnostics[0].message.contains("Argument 1 to 'Configure'"));
    }

    #[test]
    fn allows_none_for_object_and_array_parameters_but_not_primitives() {
        let diagnostics = check(
            r#"
ScriptName Example

Function Track(Actor akActor, Int[] values, Int count)
EndFunction

Function Test()
    Track(None, None, None)
EndFunction
"#,
        );

        assert_eq!(diagnostics.len(), 1);
        assert!(diagnostics[0].message.contains("Argument 3 to 'Track'"));
        assert!(diagnostics[0].message.contains("got None"));
    }

    #[test]
    fn checks_self_qualified_and_unqualified_calls_the_same_way() {
        let diagnostics = check(
            "ScriptName Example\n\nFunction Greet(String name)\nEndFunction\n\nFunction Test()\n    self.Greet(1)\nEndFunction\n",
        );

        assert_eq!(diagnostics.len(), 1);
        assert!(diagnostics[0].message.contains("Greet"));
    }

    #[test]
    fn does_not_flag_a_nested_call_argument_since_its_return_type_is_unresolved() {
        // `infer_type` can't resolve what a call expression evaluates to
        // (see its docs), so an argument that is itself a call is always
        // skipped rather than checked — this only confirms that skip
        // doesn't crash or misfire.
        let diagnostics = check(
            r#"
ScriptName Example

Function Greet(String name)
EndFunction

String Function GetName()
    Return "hi"
EndFunction

Function Test()
    Greet(GetName())
EndFunction
"#,
        );

        assert!(diagnostics.is_empty());
    }

    #[test]
    fn skips_calls_with_unresolvable_target_or_argument_type() {
        let diagnostics = check(
            r#"
ScriptName Example

Function Test(Actor akActor)
    akActor.SendAnimationEvent("Wave")
    Debug.Trace("hi")
    UnknownLocal()
EndFunction
"#,
        );

        assert!(diagnostics.is_empty());
    }

    #[test]
    fn skips_ambiguous_overrides_across_states() {
        let diagnostics = check(
            r#"
ScriptName Example

Function Greet(String name)
EndFunction

State Loud
    Function Greet(Int volume)
    EndFunction
EndState

Function Test()
    Greet(1)
EndFunction
"#,
        );

        assert!(diagnostics.is_empty());
    }

    #[test]
    fn does_not_flag_calls_beyond_the_declared_parameter_count() {
        let diagnostics = check(
            "ScriptName Example\n\nFunction Greet(String name)\nEndFunction\n\nFunction Test()\n    Greet(\"hi\", 1)\nEndFunction\n",
        );

        assert!(diagnostics.is_empty());
    }

    #[test]
    fn does_not_crash_on_unparseable_source() {
        let diagnostics = check("ScriptName Example\n\nFunction Test(\nEndFunction\n");
        assert!(diagnostics.is_empty());
    }

    struct FakeExternal;

    impl ExternalSignatures for FakeExternal {
        fn lookup(&mut self, type_name: &str, function_name: &str) -> Option<Vec<TypeName>> {
            if type_name.eq_ignore_ascii_case("Actor")
                && function_name.eq_ignore_ascii_case("MoveTo")
            {
                Some(vec![TypeName {
                    name: "ObjectReference".to_string(),
                    is_array: false,
                }])
            } else {
                None
            }
        }
    }

    struct FakeExternalWithSubtypes;

    impl ExternalSignatures for FakeExternalWithSubtypes {
        fn lookup(&mut self, type_name: &str, function_name: &str) -> Option<Vec<TypeName>> {
            if type_name.eq_ignore_ascii_case("ObjectReference")
                && function_name.eq_ignore_ascii_case("GetItemCount")
            {
                Some(vec![TypeName {
                    name: "Form".to_string(),
                    is_array: false,
                }])
            } else {
                None
            }
        }

        fn is_subtype(&mut self, sub_type: &str, super_type: &str) -> bool {
            sub_type.eq_ignore_ascii_case("Armor") && super_type.eq_ignore_ascii_case("Form")
        }
    }

    #[test]
    fn accepts_an_argument_whose_script_extends_the_parameter_type() {
        let diagnostics = check_with(
            "ScriptName Example\n\nArmor Property MyArmor Auto\n\nFunction Test(ObjectReference akRef)\n    akRef.GetItemCount(MyArmor)\nEndFunction\n",
            &mut FakeExternalWithSubtypes,
        );

        assert!(diagnostics.is_empty());
    }

    #[test]
    fn still_flags_an_unrelated_object_type() {
        let diagnostics = check_with(
            "ScriptName Example\n\nWeapon Property MyWeapon Auto\n\nFunction Test(ObjectReference akRef)\n    akRef.GetItemCount(MyWeapon)\nEndFunction\n",
            &mut FakeExternalWithSubtypes,
        );

        assert_eq!(diagnostics.len(), 1);
        assert!(diagnostics[0].message.contains("expects Form"));
        assert!(diagnostics[0].message.contains("got Weapon"));
    }

    #[test]
    fn check_with_resolves_calls_through_the_external_resolver() {
        let diagnostics = check_with(
            "ScriptName Example\n\nFunction Test(Actor akActor)\n    akActor.MoveTo(1)\nEndFunction\n",
            &mut FakeExternal,
        );

        assert_eq!(diagnostics.len(), 1);
        assert!(diagnostics[0].message.contains("expects ObjectReference"));
    }
}
