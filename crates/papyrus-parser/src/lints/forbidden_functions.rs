//! "Forbidden/discouraged function usage" lint.
//!
//! The rule data lives in `rules/forbidden-functions.yaml` at the repo
//! root. `build.rs` compiles that file into the `FORBIDDEN_FUNCTIONS`
//! array below at build time, so running this lint never touches a YAML
//! parser (or the filesystem) at runtime.

use crate::ast::{Expr, Script, Stmt};
use serde::Serialize;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Level {
    Error,
    Warning,
    Info,
}

impl std::fmt::Display for Level {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let s = match self {
            Level::Error => "error",
            Level::Warning => "warning",
            Level::Info => "info",
        };
        write!(f, "{s}")
    }
}

/// A single compiled entry from `rules/forbidden-functions.yaml`.
#[derive(Debug, Clone, Copy)]
pub struct ForbiddenFunctionRule {
    pub script: &'static str,
    pub function: &'static str,
    pub level: Level,
    pub message: &'static str,
}

include!(concat!(env!("OUT_DIR"), "/forbidden_functions_data.rs"));

/// A forbidden-function usage found in a script.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Finding {
    pub line: usize,
    pub level: Level,
    pub script: &'static str,
    pub function: &'static str,
    pub message: &'static str,
}

/// Finds calls to forbidden/discouraged functions (see
/// `rules/forbidden-functions.yaml`) anywhere in `script`.
pub fn lint_forbidden_functions(script: &Script) -> Vec<Finding> {
    let mut findings = Vec::new();
    for function in &script.functions {
        walk_stmts(&function.body, &mut findings);
    }
    for state in &script.states {
        for function in &state.functions {
            walk_stmts(&function.body, &mut findings);
        }
    }
    findings
}

fn walk_stmts(stmts: &[Stmt], findings: &mut Vec<Finding>) {
    for stmt in stmts {
        walk_stmt(stmt, findings);
    }
}

fn walk_stmt(stmt: &Stmt, findings: &mut Vec<Finding>) {
    match stmt {
        Stmt::VarDecl(decl) => {
            if let Some(value) = &decl.value {
                walk_expr(value, decl.line, findings);
            }
        }
        Stmt::Assign {
            target,
            value,
            line,
            ..
        } => {
            walk_expr(target, *line, findings);
            walk_expr(value, *line, findings);
        }
        Stmt::Expr { expr, line } => walk_expr(expr, *line, findings),
        Stmt::Return { value, line } => {
            if let Some(value) = value {
                walk_expr(value, *line, findings);
            }
        }
        Stmt::If {
            branches,
            else_body,
            line,
        } => {
            for branch in branches {
                walk_expr(&branch.condition, *line, findings);
                walk_stmts(&branch.body, findings);
            }
            walk_stmts(else_body, findings);
        }
        Stmt::While {
            condition,
            body,
            line,
        } => {
            walk_expr(condition, *line, findings);
            walk_stmts(body, findings);
        }
    }
}

/// Walks `expr` looking for forbidden calls, attributing any finding to
/// `line` — the line of the statement `expr` was found in. The AST does
/// not currently track a line per-expression, so nested calls are
/// attributed to their enclosing statement's line; in Papyrus source this
/// is almost always the exact line anyway, since statements are
/// conventionally single-line.
fn walk_expr(expr: &Expr, line: usize, findings: &mut Vec<Finding>) {
    match expr {
        Expr::Literal(_) | Expr::Identifier(_) | Expr::Self_ | Expr::Parent => {}
        Expr::Binary { left, right, .. } => {
            walk_expr(left, line, findings);
            walk_expr(right, line, findings);
        }
        Expr::Unary { operand, .. } => walk_expr(operand, line, findings),
        Expr::Call { callee, args } => {
            if let Some(rule) = match_call(callee) {
                findings.push(Finding {
                    line,
                    level: rule.level,
                    script: rule.script,
                    function: rule.function,
                    message: rule.message,
                });
            }
            walk_expr(callee, line, findings);
            for arg in args {
                walk_expr(arg, line, findings);
            }
        }
        Expr::Member { object, .. } => walk_expr(object, line, findings),
        Expr::Index { object, index } => {
            walk_expr(object, line, findings);
            walk_expr(index, line, findings);
        }
        Expr::Cast { value, .. } => walk_expr(value, line, findings),
        Expr::NewArray { size, .. } => walk_expr(size, line, findings),
    }
}

/// Matches a call's callee expression against the compiled rule table.
///
/// The parser has no type/symbol resolution, so a call site's receiver
/// (e.g. the `akRef` in `akRef.GetLinkedRef()`, or an implicit `self` for
/// an unqualified call) can't generally be resolved back to the script
/// that declares the function. Matching is therefore done by function
/// name alone, case-insensitively — Papyrus identifiers are
/// case-insensitive, and every entry in `forbidden-functions.yaml`
/// currently has a unique function name, so this has no false matches in
/// practice. `rule.script` is kept for reference/messaging, not as a
/// matching key.
fn match_call(callee: &Expr) -> Option<&'static ForbiddenFunctionRule> {
    let name = match callee {
        Expr::Identifier(name) => name.as_str(),
        Expr::Member { property, .. } => property.as_str(),
        _ => return None,
    };
    FORBIDDEN_FUNCTIONS
        .iter()
        .find(|rule| rule.function.eq_ignore_ascii_case(name))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parse;

    #[test]
    fn compiled_rules_are_loaded_from_yaml() {
        assert_eq!(FORBIDDEN_FUNCTIONS.len(), 12);
        assert!(FORBIDDEN_FUNCTIONS
            .iter()
            .any(|r| r.script == "Game" && r.function == "GetPlayer" && r.level == Level::Error));
    }

    #[test]
    fn flags_qualified_call_statement() {
        let src = r#"
ScriptName Example extends ObjectReference

Function DoThing()
    Game.GetPlayer()
EndFunction
"#;
        let script = parse(src).unwrap();
        let findings = lint_forbidden_functions(&script);
        assert_eq!(findings.len(), 1);
        assert_eq!(findings[0].line, 5);
        assert_eq!(findings[0].level, Level::Error);
        assert_eq!(findings[0].function, "GetPlayer");
    }

    #[test]
    fn flags_call_used_in_expression() {
        let src = r#"
ScriptName Example

Function DoThing()
    Actor player = Game.GetPlayer() as Actor
EndFunction
"#;
        let script = parse(src).unwrap();
        let findings = lint_forbidden_functions(&script);
        assert_eq!(findings.len(), 1);
        assert_eq!(findings[0].function, "GetPlayer");
    }

    #[test]
    fn flags_unqualified_instance_call() {
        let src = r#"
ScriptName Example extends ObjectReference

Function DoThing()
    GetLinkedRef()
EndFunction
"#;
        let script = parse(src).unwrap();
        let findings = lint_forbidden_functions(&script);
        assert_eq!(findings.len(), 1);
        assert_eq!(findings[0].function, "GetLinkedRef");
        assert_eq!(findings[0].level, Level::Warning);
    }

    #[test]
    fn flags_call_on_arbitrary_receiver() {
        let src = r#"
ScriptName Example

Function DoThing(ObjectReference akRef)
    akRef.RegisterForUpdate(1.0)
EndFunction
"#;
        let script = parse(src).unwrap();
        let findings = lint_forbidden_functions(&script);
        assert_eq!(findings.len(), 1);
        assert_eq!(findings[0].function, "RegisterForUpdate");
    }

    #[test]
    fn does_not_flag_unrelated_calls() {
        let src = r#"
ScriptName Example

Function DoThing()
    Debug.MessageBox("hi")
    self.DoOtherThing()
EndFunction

Function DoOtherThing()
EndFunction
"#;
        let script = parse(src).unwrap();
        let findings = lint_forbidden_functions(&script);
        assert!(findings.is_empty());
    }

    #[test]
    fn flags_calls_inside_if_and_while_bodies() {
        let src = r#"
ScriptName Example

Function DoThing(Int x)
    If x > 0
        Debug.Trace("positive")
    EndIf
    While x > 0
        Debug.Notification("still going")
        x -= 1
    EndWhile
EndFunction
"#;
        let script = parse(src).unwrap();
        let findings = lint_forbidden_functions(&script);
        assert_eq!(findings.len(), 2);
        assert!(findings.iter().any(|f| f.function == "Trace"));
        assert!(findings.iter().any(|f| f.function == "Notification"));
    }
}
