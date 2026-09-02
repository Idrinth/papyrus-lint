//! Flags a declared identifier (a function/event, property, state,
//! parameter, or local/script variable) whose name doesn't match the
//! project's configured casing style (see [`crate::config::IdentifierCasing`]).
//!
//! This works from the parsed AST rather than raw tokens, since it needs
//! to reliably tell a declaration's name apart from any other identifier
//! in the script; a script that doesn't parse cleanly is left unchecked
//! rather than guessed at. `ScriptName` itself is never checked, since it
//! must match the script's filename regardless of casing style.
//!
//! A parameter has no line of its own in the AST, so it's reported on its
//! enclosing function's line.

use std::collections::HashMap;

use papyrus_parser::ast::{FunctionDecl, StateDecl, Stmt};
use papyrus_parser::token::{Keyword, TokenKind};

use crate::config::IdentifierCasing;
use crate::{fragment_code, Diagnostic};

/// This lint's [`Diagnostic::rule`] id, for `@disable` line comments.
pub const RULE: &str = "identifier-casing";

/// Checks `source` for declared identifiers that don't conform to `style`.
/// Flagged as a `[warning]`.
///
/// A declaration inside a CreationKit fragment-code wrapper (see
/// [`fragment_code`]), outside of its `;BEGIN CODE`/`;END CODE` markers, is
/// never flagged: it's CreationKit-generated boilerplate the user can't
/// edit or rename.
pub fn check(source: &str, style: IdentifierCasing) -> Vec<Diagnostic> {
    let Ok(script) = papyrus_parser::parse(source) else {
        return Vec::new();
    };
    let protected = fragment_code::protected_lines(source);

    let mut diagnostics = Vec::new();

    for variable in &script.variables {
        check_name(
            &variable.name,
            "Variable",
            variable.line,
            style,
            &protected,
            &mut diagnostics,
        );
    }
    for property in &script.properties {
        check_name(
            &property.name,
            "Property",
            property.line,
            style,
            &protected,
            &mut diagnostics,
        );
    }
    for state in &script.states {
        check_state(state, style, &protected, &mut diagnostics);
    }
    for function in &script.functions {
        check_function(function, style, &protected, &mut diagnostics);
    }

    diagnostics
}

/// Renames every non-conforming declaration and its references to `style`
/// when doing so does not add, remove, or move underscores.
///
/// Papyrus identifiers are case-insensitive, so references are matched that
/// way too. Tokens in comments and strings are naturally excluded by the
/// lexer, and CreationKit-owned fragment wrapper lines are left untouched.
/// If the source cannot be parsed or tokenized, it is returned unchanged.
pub fn repair(source: &str, style: IdentifierCasing) -> String {
    let Ok(script) = papyrus_parser::parse(source) else {
        return source.to_string();
    };
    let protected = fragment_code::protected_lines(source);
    let mut renames = HashMap::new();

    for variable in &script.variables {
        collect_name(
            &variable.name,
            variable.line,
            style,
            &protected,
            &mut renames,
        );
    }
    for property in &script.properties {
        collect_name(
            &property.name,
            property.line,
            style,
            &protected,
            &mut renames,
        );
    }
    for state in &script.states {
        collect_state_names(state, style, &protected, &mut renames);
    }
    for function in &script.functions {
        collect_function_names(function, style, &protected, &mut renames);
    }
    if renames.is_empty() {
        return source.to_string();
    }

    let Ok(tokens) = papyrus_parser::tokenize(source) else {
        return source.to_string();
    };
    let mut replacements: Vec<(usize, usize, String)> = Vec::new();
    let line_offsets = line_offsets(source);
    let mut follows_script_name = false;

    for token in tokens {
        match token.kind {
            TokenKind::Keyword(Keyword::ScriptName) => follows_script_name = true,
            TokenKind::Identifier(name) => {
                if follows_script_name {
                    follows_script_name = false;
                    continue;
                }
                if protected.get(token.line).copied().unwrap_or(false) {
                    continue;
                }
                if let Some(replacement) = renames.get(&name.to_ascii_lowercase()) {
                    let start = line_offsets[token.line - 1] + token.col - 1;
                    replacements.push((start, start + name.len(), replacement.clone()));
                }
            }
            TokenKind::Newline => follows_script_name = false,
            _ => {}
        }
    }

    let mut repaired = source.to_string();
    for (start, end, replacement) in replacements.into_iter().rev() {
        repaired.replace_range(start..end, &replacement);
    }
    repaired
}

fn collect_state_names(
    state: &StateDecl,
    style: IdentifierCasing,
    protected: &[bool],
    renames: &mut HashMap<String, String>,
) {
    collect_name(&state.name, state.line, style, protected, renames);
    for function in &state.functions {
        collect_function_names(function, style, protected, renames);
    }
}

fn collect_function_names(
    function: &FunctionDecl,
    style: IdentifierCasing,
    protected: &[bool],
    renames: &mut HashMap<String, String>,
) {
    collect_name(&function.name, function.line, style, protected, renames);
    for param in &function.params {
        collect_name(&param.name, function.line, style, protected, renames);
    }
    for stmt in &function.body {
        collect_stmt_names(stmt, style, protected, renames);
    }
}

fn collect_stmt_names(
    stmt: &Stmt,
    style: IdentifierCasing,
    protected: &[bool],
    renames: &mut HashMap<String, String>,
) {
    match stmt {
        Stmt::VarDecl(decl) => collect_name(&decl.name, decl.line, style, protected, renames),
        Stmt::If {
            branches,
            else_body,
            ..
        } => {
            for stmt in branches
                .iter()
                .flat_map(|branch| &branch.body)
                .chain(else_body)
            {
                collect_stmt_names(stmt, style, protected, renames);
            }
        }
        Stmt::While { body, .. } => {
            for stmt in body {
                collect_stmt_names(stmt, style, protected, renames);
            }
        }
        Stmt::Assign { .. } | Stmt::Expr { .. } | Stmt::Return { .. } => {}
    }
}

fn collect_name(
    name: &str,
    line: usize,
    style: IdentifierCasing,
    protected: &[bool],
    renames: &mut HashMap<String, String>,
) {
    if !protected.get(line).copied().unwrap_or(false) && !style.matches(name) {
        let replacement = convert_name(name, style);
        // Underscores are often a meaningful, externally-visible part of a
        // Papyrus identifier.  In particular, removing one can turn a name
        // such as `DFO_VampireFeed` into a different identifier rather than
        // merely correcting its letter case. Leave these substantive renames
        // to the user and only apply an automatic fix when every underscore
        // remains in exactly the same position.
        if underscore_offsets(name).eq(underscore_offsets(&replacement)) {
            renames.insert(name.to_ascii_lowercase(), replacement);
        }
    }
}

fn underscore_offsets(name: &str) -> impl Iterator<Item = usize> + '_ {
    name.match_indices('_').map(|(offset, _)| offset)
}

fn convert_name(name: &str, style: IdentifierCasing) -> String {
    let chars: Vec<char> = name.chars().collect();
    let mut words = Vec::new();
    let mut start = 0;
    for i in 0..=chars.len() {
        let boundary = i == chars.len()
            || chars[i] == '_'
            || (i > start && chars[i].is_ascii_uppercase() && chars[i - 1].is_ascii_lowercase())
            || (i > start
                && i + 1 < chars.len()
                && chars[i - 1].is_ascii_uppercase()
                && chars[i].is_ascii_uppercase()
                && chars[i + 1].is_ascii_lowercase());
        if boundary {
            if start < i {
                words.push(
                    chars[start..i]
                        .iter()
                        .collect::<String>()
                        .to_ascii_lowercase(),
                );
            }
            start = i + usize::from(i < chars.len() && chars[i] == '_');
        }
    }
    match style {
        IdentifierCasing::SnakeCase => words.join("_"),
        IdentifierCasing::ConstantCase => words.join("_").to_ascii_uppercase(),
        IdentifierCasing::CamelCase | IdentifierCasing::PascalCase => words
            .into_iter()
            .enumerate()
            .map(|(index, word)| {
                if index == 0 && style == IdentifierCasing::CamelCase {
                    word
                } else {
                    let mut chars = word.chars();
                    chars
                        .next()
                        .map(|c| c.to_ascii_uppercase())
                        .into_iter()
                        .chain(chars)
                        .collect()
                }
            })
            .collect(),
    }
}

fn line_offsets(source: &str) -> Vec<usize> {
    std::iter::once(0)
        .chain(source.match_indices('\n').map(|(index, _)| index + 1))
        .collect()
}

fn check_state(
    state: &StateDecl,
    style: IdentifierCasing,
    protected: &[bool],
    diagnostics: &mut Vec<Diagnostic>,
) {
    check_name(
        &state.name,
        "State",
        state.line,
        style,
        protected,
        diagnostics,
    );
    for function in &state.functions {
        check_function(function, style, protected, diagnostics);
    }
}

fn check_function(
    function: &FunctionDecl,
    style: IdentifierCasing,
    protected: &[bool],
    diagnostics: &mut Vec<Diagnostic>,
) {
    let kind = if function.is_event {
        "Event"
    } else {
        "Function"
    };
    check_name(
        &function.name,
        kind,
        function.line,
        style,
        protected,
        diagnostics,
    );
    for param in &function.params {
        check_name(
            &param.name,
            "Parameter",
            function.line,
            style,
            protected,
            diagnostics,
        );
    }
    for stmt in &function.body {
        check_stmt(stmt, style, protected, diagnostics);
    }
}

fn check_stmt(
    stmt: &Stmt,
    style: IdentifierCasing,
    protected: &[bool],
    diagnostics: &mut Vec<Diagnostic>,
) {
    match stmt {
        Stmt::VarDecl(decl) => check_name(
            &decl.name,
            "Variable",
            decl.line,
            style,
            protected,
            diagnostics,
        ),
        Stmt::If {
            branches,
            else_body,
            ..
        } => {
            for branch in branches {
                for inner in &branch.body {
                    check_stmt(inner, style, protected, diagnostics);
                }
            }
            for inner in else_body {
                check_stmt(inner, style, protected, diagnostics);
            }
        }
        Stmt::While { body, .. } => {
            for inner in body {
                check_stmt(inner, style, protected, diagnostics);
            }
        }
        Stmt::Assign { .. } | Stmt::Expr { .. } | Stmt::Return { .. } => {}
    }
}

fn check_name(
    name: &str,
    kind: &str,
    line: usize,
    style: IdentifierCasing,
    protected: &[bool],
    diagnostics: &mut Vec<Diagnostic>,
) {
    if protected.get(line).copied().unwrap_or(false) {
        return;
    }
    if style.matches(name) {
        return;
    }

    diagnostics.push(Diagnostic {
        line,
        column: 1,
        message: format!(
            "[warning] {kind} '{name}' does not match the configured {} casing style",
            style.label()
        ),
        rule: RULE,
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn flags_property_not_matching_pascal_case() {
        let diagnostics = check(
            "ScriptName Example\n\nInt Property myValue = 1 Auto\n",
            IdentifierCasing::PascalCase,
        );

        assert_eq!(diagnostics.len(), 1);
        assert_eq!(diagnostics[0].line, 3);
        assert!(diagnostics[0].message.starts_with("[warning]"));
        assert!(diagnostics[0].message.contains("Property"));
        assert!(diagnostics[0].message.contains("myValue"));
    }

    #[test]
    fn does_not_flag_property_matching_pascal_case() {
        let diagnostics = check(
            "ScriptName Example\n\nInt Property MyValue = 1 Auto\n",
            IdentifierCasing::PascalCase,
        );

        assert!(diagnostics.is_empty());
    }

    #[test]
    fn flags_script_level_variable_name() {
        let diagnostics = check(
            "ScriptName Example\n\nInt bad_name = 1\n",
            IdentifierCasing::PascalCase,
        );

        assert_eq!(diagnostics.len(), 1);
        assert_eq!(diagnostics[0].line, 3);
        assert_eq!(diagnostics[0].rule, RULE);
        assert!(diagnostics[0].message.contains("Variable"));
        assert!(diagnostics[0].message.contains("bad_name"));
    }

    #[test]
    fn flags_function_name() {
        let diagnostics = check(
            "ScriptName Example\n\nFunction do_thing()\nEndFunction\n",
            IdentifierCasing::PascalCase,
        );

        assert_eq!(diagnostics.len(), 1);
        assert!(diagnostics[0].message.contains("Function"));
        assert!(diagnostics[0].message.contains("do_thing"));
    }

    #[test]
    fn flags_event_name() {
        let diagnostics = check(
            "ScriptName Example\n\nEvent on_init()\nEndEvent\n",
            IdentifierCasing::PascalCase,
        );

        assert_eq!(diagnostics.len(), 1);
        assert!(diagnostics[0].message.contains("Event"));
        assert!(diagnostics[0].message.contains("on_init"));
    }

    #[test]
    fn flags_parameter_on_the_function_line() {
        let diagnostics = check(
            "ScriptName Example\n\nFunction DoThing(Int bad_name)\nEndFunction\n",
            IdentifierCasing::PascalCase,
        );

        assert_eq!(diagnostics.len(), 1);
        assert_eq!(diagnostics[0].line, 3);
        assert!(diagnostics[0].message.contains("Parameter"));
        assert!(diagnostics[0].message.contains("bad_name"));
    }

    #[test]
    fn flags_local_variable_inside_a_function_body() {
        let diagnostics = check(
            "ScriptName Example\n\nFunction DoThing()\n    Int bad_name = 1\nEndFunction\n",
            IdentifierCasing::PascalCase,
        );

        assert_eq!(diagnostics.len(), 1);
        assert_eq!(diagnostics[0].line, 4);
        assert!(diagnostics[0].message.contains("Variable"));
        assert!(diagnostics[0].message.contains("bad_name"));
    }

    #[test]
    fn flags_local_variable_nested_in_if_and_while_bodies() {
        let diagnostics = check(
            "ScriptName Example\n\nFunction DoThing()\n    If true\n        Int bad_one = 1\n    Else\n        Int bad_two = 2\n    EndIf\n    While true\n        Int bad_three = 3\n    EndWhile\nEndFunction\n",
            IdentifierCasing::PascalCase,
        );

        assert_eq!(diagnostics.len(), 3);
        assert!(diagnostics.iter().any(|d| d.message.contains("bad_one")));
        assert!(diagnostics.iter().any(|d| d.message.contains("bad_two")));
        assert!(diagnostics.iter().any(|d| d.message.contains("bad_three")));
    }

    #[test]
    fn flags_state_name() {
        let diagnostics = check(
            "ScriptName Example\n\nState bad_state\nEndState\n",
            IdentifierCasing::PascalCase,
        );

        assert_eq!(diagnostics.len(), 1);
        assert!(diagnostics[0].message.contains("State"));
        assert!(diagnostics[0].message.contains("bad_state"));
    }

    #[test]
    fn flags_function_declared_inside_a_state() {
        let diagnostics = check(
            "ScriptName Example\n\nState Idle\n    Function do_thing()\n    EndFunction\nEndState\n",
            IdentifierCasing::PascalCase,
        );

        assert!(diagnostics.iter().any(|d| d.message.contains("do_thing")));
    }

    #[test]
    fn never_flags_script_name() {
        let diagnostics = check("ScriptName not_pascal_case\n", IdentifierCasing::PascalCase);

        assert!(diagnostics.is_empty());
    }

    #[test]
    fn checks_against_the_configured_style() {
        let source = "ScriptName Example\n\nInt Property my_value = 1 Auto\n";

        assert!(check(source, IdentifierCasing::SnakeCase).is_empty());
        assert!(!check(source, IdentifierCasing::PascalCase).is_empty());
    }

    #[test]
    fn ignores_declarations_inside_a_fragment_wrapper() {
        let source = "\
;BEGIN FRAGMENT CODE - Do not edit anything between this and the end comment
Scriptname IDR__TIF__05000235 Extends TopicInfo Hidden

;BEGIN FRAGMENT Fragment_0
Function fragment_0(ObjectReference akSpeakerRef)
Actor bad_local = akSpeakerRef as Actor
;BEGIN CODE
Int GoodStyle = 1
;END CODE
EndFunction
;END FRAGMENT

;END FRAGMENT CODE - Do not edit anything between this and the begin comment
";

        let diagnostics = check(source, IdentifierCasing::PascalCase);

        assert!(diagnostics.is_empty());
    }

    #[test]
    fn does_not_crash_on_unparseable_source() {
        let diagnostics = check(
            "ScriptName Example\n\nInt Property bad_name = \"unterminated\n",
            IdentifierCasing::PascalCase,
        );
        assert!(diagnostics.is_empty());
    }

    #[test]
    fn repair_renames_declarations_and_references() {
        let source = "ScriptName Example\n\nInt Property MaxCount Auto\n\nFunction AddValue(Int ItemCount)\n    Int NewTotal = MaxCount + ItemCount\n    MaxCount = NewTotal\nEndFunction\n";

        assert_eq!(
            repair(source, IdentifierCasing::CamelCase),
            "ScriptName Example\n\nInt Property maxCount Auto\n\nFunction addValue(Int itemCount)\n    Int newTotal = maxCount + itemCount\n    maxCount = newTotal\nEndFunction\n"
        );
    }

    #[test]
    fn repair_never_adds_removes_or_moves_underscores() {
        let source = "ScriptName Example\n\nFunction DFO_VampireFeed()\n    DFO_VampireFeed()\nEndFunction\n";

        assert_eq!(repair(source, IdentifierCasing::CamelCase), source);
        assert_eq!(
            repair(
                "ScriptName Example\n\nFunction HTTPResponseCode()\nEndFunction\n",
                IdentifierCasing::SnakeCase,
            ),
            "ScriptName Example\n\nFunction HTTPResponseCode()\nEndFunction\n"
        );
    }

    #[test]
    fn repair_supports_every_casing_style() {
        assert_eq!(
            convert_name("HTTP_responseCode", IdentifierCasing::CamelCase),
            "httpResponseCode"
        );
        assert_eq!(
            convert_name("HTTP_responseCode", IdentifierCasing::PascalCase),
            "HttpResponseCode"
        );
        assert_eq!(
            convert_name("HTTPResponseCode", IdentifierCasing::SnakeCase),
            "http_response_code"
        );
        assert_eq!(
            convert_name("HTTPResponseCode", IdentifierCasing::ConstantCase),
            "HTTP_RESPONSE_CODE"
        );
    }

    #[test]
    fn repair_leaves_script_name_comments_strings_and_fragment_wrapper_untouched() {
        let source = ";BEGIN FRAGMENT CODE - Do not edit anything between this and the end comment\nScriptName bad_name\nFunction generated_name()\n;BEGIN CODE\nInt userValue = 1 ; userValue\nString textValue = \"userValue\"\nuserValue = 2\n;END CODE\nEndFunction\n;END FRAGMENT CODE - Do not edit anything between this and the begin comment\n";
        let repaired = repair(source, IdentifierCasing::PascalCase);

        assert!(repaired.contains("ScriptName bad_name\nFunction generated_name()"));
        assert!(repaired.contains("Int UserValue = 1 ; userValue"));
        assert!(repaired.contains("String TextValue = \"userValue\""));
        assert!(repaired.contains("UserValue = 2"));
    }

    #[test]
    fn repair_returns_unparseable_source_unchanged() {
        let source = "ScriptName Example\nFunction bad_name(\n";
        assert_eq!(repair(source, IdentifierCasing::PascalCase), source);
    }
}
