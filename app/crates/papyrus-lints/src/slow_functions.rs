//! Flags calls to functions listed in `rules/slow-functions.yaml` that
//! have a faster equivalent available, and suggests that replacement.
//!
//! Rules are compiled into the `SLOW_FUNCTIONS` array below by `build.rs`
//! at build time, so this never parses YAML at runtime. Like the other
//! lints in this crate, it works on tokens rather than the parsed AST, so
//! it still runs on scripts that don't parse cleanly.

use crate::Diagnostic;
use papyrus_parser::token::TokenKind;

pub struct SlowFunctionRule {
    pub object: &'static str,
    pub function: &'static str,
    pub replacement: &'static str,
    /// Whether `object` is a native singleton (e.g. `Game`, `Utility`)
    /// always called through its literal script name, rather than a base
    /// type (e.g. `GlobalVariable`) called through a variable of some
    /// subclass. See `check` for how this is used.
    pub global: bool,
}

include!(concat!(env!("OUT_DIR"), "/slow_functions_data.rs"));

/// This lint's [`Diagnostic::rule`] id, for `@disable` line comments.
pub const RULE: &str = "slow-functions";

/// Checks `source` for calls to functions with a faster equivalent.
///
/// A call site is any identifier immediately followed by `(`. The lexer
/// has no type/symbol resolution, so — like the receiver in
/// `akGlobal.GetValueInt()` — a call's qualifier can't generally be
/// resolved back to the script that declares the function; matching is
/// therefore done by function name alone, case-insensitively (Papyrus
/// identifiers are case-insensitive). Flagged as an `[info]`, since it's a
/// performance suggestion rather than a correctness issue.
///
/// The exception is a rule whose `object` is a native singleton
/// (`global: true` in the YAML, e.g. `Utility`) rather than a base type
/// used through a variable: those scripts are never subclassed, so a
/// qualified call to one of their functions is only a real match when the
/// qualifier is literally that script's name.
pub fn check(source: &str) -> Vec<Diagnostic> {
    check_with_rules(source, SLOW_FUNCTIONS)
}

/// Replaces slow calls with the faster expression supplied by their rule.
///
/// A replacement containing `(` describes the complete call. The word
/// `value` in such a replacement is a placeholder for the original call's
/// argument. A bare replacement names the faster function and retains the
/// original argument list. Calls whose parentheses are unbalanced are left
/// untouched.
pub fn repair(source: &str) -> String {
    repair_with_rules(source, SLOW_FUNCTIONS)
}

fn repair_with_rules(source: &str, rules: &'static [SlowFunctionRule]) -> String {
    let Ok(tokens) = papyrus_parser::tokenize(source) else {
        return source.to_string();
    };
    let line_starts = line_starts(source);
    let mut replacements = Vec::new();

    for (call_index, token) in tokens.iter().enumerate() {
        let TokenKind::Identifier(name) = &token.kind else {
            continue;
        };
        if !matches!(
            tokens.get(call_index + 1).map(|token| &token.kind),
            Some(TokenKind::LParen)
        ) {
            continue;
        }
        let Some(rule) = find_rule(rules, name) else {
            continue;
        };
        if rule.global && !qualifier_matches(&tokens, call_index, rule.object) {
            continue;
        }
        let Some(close_index) = matching_close_paren(&tokens, call_index + 1) else {
            continue;
        };

        let start = token_offset(&line_starts, token);
        let argument_start = token_offset(&line_starts, &tokens[call_index + 1]) + 1;
        let close = token_offset(&line_starts, &tokens[close_index]);
        let end = close + 1;
        // An outer slow call owns its entire source range. Repair its argument
        // recursively and don't also queue an overlapping inner edit against
        // offsets from the original string.
        if replacements
            .iter()
            .any(|(outer_start, outer_end, _)| *outer_start <= start && end <= *outer_end)
        {
            continue;
        }
        let argument = repair_with_rules(source[argument_start..close].trim(), rules);
        let replacement = if rule.replacement.contains('(') {
            rule.replacement.replace("value", &argument)
        } else {
            format!("{}({argument})", rule.replacement)
        };
        replacements.push((start, end, replacement));
    }

    let mut repaired = source.to_string();
    for (start, end, replacement) in replacements.into_iter().rev() {
        repaired.replace_range(start..end, &replacement);
    }
    repaired
}

fn matching_close_paren(
    tokens: &[papyrus_parser::token::Token],
    open_index: usize,
) -> Option<usize> {
    let mut depth = 0usize;
    for (index, token) in tokens.iter().enumerate().skip(open_index) {
        match token.kind {
            TokenKind::LParen => depth += 1,
            TokenKind::RParen => {
                depth -= 1;
                if depth == 0 {
                    return Some(index);
                }
            }
            _ => {}
        }
    }
    None
}

fn line_starts(source: &str) -> Vec<usize> {
    std::iter::once(0)
        .chain(
            source
                .bytes()
                .enumerate()
                .filter_map(|(index, byte)| (byte == b'\n').then_some(index + 1)),
        )
        .collect()
}

fn token_offset(line_starts: &[usize], token: &papyrus_parser::token::Token) -> usize {
    line_starts[token.line - 1] + token.col - 1
}

fn check_with_rules(source: &str, rules: &'static [SlowFunctionRule]) -> Vec<Diagnostic> {
    let tokens = match papyrus_parser::tokenize(source) {
        Ok(tokens) => tokens,
        Err(_) => return Vec::new(),
    };

    let mut diagnostics = Vec::new();
    for (i, window) in tokens.windows(2).enumerate() {
        let TokenKind::Identifier(name) = &window[0].kind else {
            continue;
        };
        if !matches!(window[1].kind, TokenKind::LParen) {
            continue;
        }
        let Some(rule) = find_rule(rules, name) else {
            continue;
        };
        if rule.global && !qualifier_matches(&tokens, i, rule.object) {
            continue;
        }
        diagnostics.push(Diagnostic {
            line: window[0].line,
            column: window[0].col,
            message: format!(
                "[info] {}.{} is slower than necessary; use `{}` instead",
                rule.object, rule.function, rule.replacement
            ),
            rule: RULE,
        });
    }
    diagnostics
}

/// Whether the call at `tokens[call_index]` is qualified with `object`
/// (case-insensitively), i.e. preceded by `object.`.
fn qualifier_matches(
    tokens: &[papyrus_parser::token::Token],
    call_index: usize,
    object: &str,
) -> bool {
    if call_index < 2 {
        return false;
    }
    if !matches!(tokens[call_index - 1].kind, TokenKind::Dot) {
        return false;
    }
    let TokenKind::Identifier(qualifier) = &tokens[call_index - 2].kind else {
        return false;
    };
    qualifier.eq_ignore_ascii_case(object)
}

fn find_rule(rules: &'static [SlowFunctionRule], name: &str) -> Option<&'static SlowFunctionRule> {
    rules
        .iter()
        .find(|rule| rule.function.eq_ignore_ascii_case(name))
}

#[cfg(test)]
mod tests {
    use super::*;

    static GLOBAL_RULES: &[SlowFunctionRule] = &[SlowFunctionRule {
        object: "ExampleGlobal",
        function: "SlowCall",
        replacement: "FastCall",
        global: true,
    }];

    #[test]
    fn compiled_rules_are_loaded_from_yaml() {
        assert_eq!(SLOW_FUNCTIONS.len(), 2);
        assert!(SLOW_FUNCTIONS
            .iter()
            .any(|r| r.object == "GlobalVariable" && r.function == "GetValueInt"));
    }

    #[test]
    fn flags_qualified_call() {
        let diagnostics = check(
            "ScriptName Example\n\nFunction DoThing(GlobalVariable akGlobal)\n    akGlobal.GetValueInt()\nEndFunction\n",
        );
        assert_eq!(diagnostics.len(), 1);
        assert_eq!(diagnostics[0].line, 4);
        assert!(diagnostics[0].message.starts_with("[info]"));
        assert!(diagnostics[0]
            .message
            .contains("GlobalVariable.GetValueInt"));
        assert!(diagnostics[0].message.contains("GetValue() As Int"));
    }

    #[test]
    fn flags_call_case_insensitively() {
        let diagnostics = check(
            "ScriptName Example\n\nFunction DoThing(GlobalVariable akGlobal)\n    akGlobal.setvalueint(1)\nEndFunction\n",
        );
        assert_eq!(diagnostics.len(), 1);
        assert!(diagnostics[0].message.contains("SetValueInt"));
    }

    #[test]
    fn flags_code_before_an_inline_comment_but_ignores_calls_in_comment_text() {
        let diagnostics = check(
            "ScriptName Example\n\nFunction DoThing(GlobalVariable akGlobal)\n    akGlobal.GetValueInt() ; akGlobal.GetValueInt()\n    ; akGlobal.GetValueInt()\nEndFunction\n",
        );

        assert_eq!(diagnostics.len(), 1);
        assert_eq!((diagnostics[0].line, diagnostics[0].column), (4, 14));
    }

    #[test]
    fn global_rule_requires_its_literal_qualifier_case_insensitively() {
        let diagnostics = check_with_rules(
            "ExampleGlobal.SlowCall(1.0)\nexampleglobal.slowcall(1.0)\nakOther.SlowCall(1.0)\nSlowCall(1.0)\nGetGlobal().SlowCall(1.0)\n",
            GLOBAL_RULES,
        );

        assert_eq!(diagnostics.len(), 2);
        assert_eq!(diagnostics[0].line, 1);
        assert_eq!(diagnostics[1].line, 2);
        assert!(diagnostics
            .iter()
            .all(|diagnostic| diagnostic.message.contains("ExampleGlobal.SlowCall")));
    }

    #[test]
    fn does_not_flag_unrelated_calls() {
        let diagnostics = check(
            "ScriptName Example\n\nFunction DoThing()\n    Debug.MessageBox(\"hi\")\n    self.DoOtherThing()\nEndFunction\n\nFunction DoOtherThing()\nEndFunction\n",
        );
        assert!(diagnostics.is_empty());
    }

    #[test]
    fn ignores_identifiers_that_are_not_calls() {
        let diagnostics = check("ScriptName Example\n\nInt GetValueInt = 1\n");
        assert!(diagnostics.is_empty());
    }

    #[test]
    fn does_not_crash_on_unparseable_source() {
        let diagnostics = check(
            "ScriptName Example\n\nFunction DoThing()\n    akGlobal.GetValueInt(\"unterminated\n",
        );
        assert!(diagnostics.is_empty());
    }

    #[test]
    fn repairs_getter_with_the_complete_replacement_expression() {
        assert_eq!(
            repair("value = akGlobal.GetValueInt()\n"),
            "value = akGlobal.GetValue() As Int\n"
        );
    }

    #[test]
    fn repairs_setter_and_preserves_its_argument_expression() {
        assert_eq!(
            repair("akGlobal.SetValueInt(GetAmount(1, 2) + 3)\n"),
            "akGlobal.SetValue(GetAmount(1, 2) + 3 As Float)\n"
        );
    }

    #[test]
    fn repair_obeys_global_qualifier_rules() {
        let source = "ExampleGlobal.SlowCall(1.0)\nakOther.SlowCall(1.0)\n";

        assert_eq!(
            repair_with_rules(source, GLOBAL_RULES),
            "ExampleGlobal.FastCall(1.0)\nakOther.SlowCall(1.0)\n"
        );
    }

    #[test]
    fn repairs_nested_slow_calls_and_handles_unicode_before_a_call() {
        let source = "text = \"é\"\nakGlobal.SetValueInt(akOther.GetValueInt())\n";

        assert_eq!(
            repair(source),
            "text = \"é\"\nakGlobal.SetValue(akOther.GetValue() As Int As Float)\n"
        );
    }

    #[test]
    fn repairs_multiple_calls_on_the_same_line_from_right_to_left() {
        assert_eq!(
            repair("first = left.GetValueInt() + right.GetValueInt()\n"),
            "first = left.GetValue() As Int + right.GetValue() As Int\n"
        );
    }

    #[test]
    fn repairs_a_call_after_unicode_on_the_same_line() {
        assert_eq!(
            repair("message = \"café\" + akGlobal.GetValueInt()\n"),
            "message = \"café\" + akGlobal.GetValue() As Int\n"
        );
    }

    #[test]
    fn repair_does_not_change_calls_in_strings_or_comments() {
        let source = "text = \"GetValueInt()\" ; GetValueInt()\n; SetValueInt(1)\n";

        assert_eq!(repair(source), source);
    }

    #[test]
    fn repair_leaves_an_unbalanced_call_untouched() {
        let source = "akGlobal.SetValueInt(GetAmount(1, 2)\n";

        assert_eq!(repair(source), source);
    }

    #[test]
    fn bare_replacement_preserves_empty_and_multiple_argument_lists() {
        assert_eq!(
            repair_with_rules(
                "ExampleGlobal.SlowCall()\nExampleGlobal.SlowCall(1, Other(2))\n",
                GLOBAL_RULES,
            ),
            "ExampleGlobal.FastCall()\nExampleGlobal.FastCall(1, Other(2))\n"
        );
    }
}
