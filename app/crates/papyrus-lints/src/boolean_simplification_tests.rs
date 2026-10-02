use super::*;

fn check(source: &str) -> Vec<Diagnostic> {
    let ast = papyrus_parser::parse(source).ok();
    let tokens = papyrus_parser::tokenize(source).ok();
    super::check(
        source,
        ast.as_ref(),
        tokens.as_deref(),
        &crate::config::Config::default(),
        &mut crate::external_signatures::NoExternalSignatures,
    )
}

fn check_for_game(source: &str, game: papyrus_lint_globals::Game) -> Vec<Diagnostic> {
    let ast = papyrus_parser::parse_for_game(source, game).ok();
    let tokens = papyrus_parser::tokenize(source).ok();
    let config = crate::config::Config {
        game,
        ..Default::default()
    };
    super::check(
        source,
        ast.as_ref(),
        tokens.as_deref(),
        &config,
        &mut crate::external_signatures::NoExternalSignatures,
    )
}

fn repair(source: &str) -> String {
    let ast = papyrus_parser::parse(source).ok();
    let tokens = papyrus_parser::tokenize(source).ok();
    super::repair(
        source,
        ast.as_ref(),
        tokens.as_deref(),
        &crate::config::Config::default(),
    )
}

const HEADER: &str = "ScriptName Example\n\n";

fn function(body: &str) -> String {
    format!("{HEADER}Function Test(Bool ready, Bool other, Int count)\n{body}EndFunction\n")
}

#[test]
fn flags_each_redundant_bool_comparison() {
    let source = function(
        "    If ready == true\n    EndIf\n    If ready == false\n    EndIf\n    If ready != true\n    EndIf\n    If ready != false\n    EndIf\n    If true == ready\n    EndIf\n    If false != ready\n    EndIf\n",
    );
    let diagnostics = check(&source);

    assert_eq!(diagnostics.len(), 6);
    assert!(diagnostics.iter().all(|diagnostic| diagnostic.message.starts_with("[info]")));
    assert!(diagnostics[0].message.contains("== true"));
    assert!(diagnostics[0].message.contains("value itself"));
    assert!(diagnostics[1].message.contains("== false"));
    assert!(diagnostics[1].message.contains("negation"));
    assert_eq!(diagnostics[0].line, 4);
    assert_eq!(diagnostics[0].column, 14);
}

#[test]
fn flags_bool_properties_parameters_and_locals() {
    let source = "\
ScriptName Example

Bool Property Ready Auto

Function Test(Bool flag)
    Bool local = true
    If Ready == TRUE
    EndIf
    If flag != False
    EndIf
    While local == false
    EndWhile
EndFunction
";
    let diagnostics = check(source);

    assert_eq!(diagnostics.len(), 3);
    assert_eq!(
        diagnostics.iter().map(|diagnostic| diagnostic.line).collect::<Vec<_>>(),
        vec![7, 9, 11]
    );
}

#[test]
fn leaves_non_bool_and_unresolved_operands_alone() {
    let source = function(
        "    If count == true\n    EndIf\n    If GetReady() == true\n    EndIf\n    If Self.Ready == false\n    EndIf\n    If ready > true\n    EndIf\n    If ready && true\n    EndIf\n    If count\n    EndIf\n",
    );

    assert!(check(&source).is_empty());
}

#[test]
fn leaves_bool_arrays_alone_but_flags_an_element() {
    let source = "\
ScriptName Example

Function Test(Bool[] flags, Bool flag)
    If flags == true
    EndIf
    If flags[0] == false
    EndIf
    If flag == true
    EndIf
EndFunction
";
    let diagnostics = check(source);

    assert_eq!(diagnostics.len(), 2);
    assert_eq!(diagnostics[0].line, 6);
    assert_eq!(diagnostics[1].line, 8);
}

#[test]
fn flags_a_bool_cast_logical_expression_and_nested_comparison() {
    let source = function(
        "    If (ready && other) == false\n    EndIf\n    If (count as Bool) == true\n    EndIf\n    If (ready == true) == false\n    EndIf\n",
    );
    let diagnostics = check(&source);

    assert_eq!(diagnostics.len(), 4, "{diagnostics:?}");
}

#[test]
fn flags_a_bool_property_and_ignores_a_full_property_body() {
    let source = "\
ScriptName Example

Bool Property Ready Auto

Bool Property Stored
    Function Get()
        Return Ready == true
    EndFunction
EndProperty

Function Test()
    If Ready == true
    EndIf
    If Stored == false
    EndIf
EndFunction
";
    let diagnostics = check(source);

    assert_eq!(diagnostics.len(), 2, "{diagnostics:?}");
    assert_eq!(
        diagnostics
            .iter()
            .map(|diagnostic| diagnostic.line)
            .collect::<Vec<_>>(),
        vec![12, 14]
    );
}

#[test]
fn pairs_a_property_initializer_with_a_later_function() {
    let source = "\
ScriptName Example

Bool Property Ready Auto
Bool Property Flag = Ready == false Auto

Function Early(Bool ready)
    If ready == true
    EndIf
EndFunction

Function Late(Bool other, Int count)
    If count == true
    EndIf
    If other != true
    EndIf
EndFunction
";
    let diagnostics = check(source);

    assert_eq!(diagnostics.len(), 3, "{diagnostics:?}");
    assert_eq!(
        diagnostics
            .iter()
            .map(|diagnostic| diagnostic.line)
            .collect::<Vec<_>>(),
        vec![4, 7, 14]
    );
}

#[test]
fn rewrites_comparisons_to_the_value_or_its_negation() {
    let source = function(
        "    If ready == true\n    EndIf\n    If ready == false\n    EndIf\n    If ready != true\n    EndIf\n    If ready != false\n    EndIf\n    If true == ready\n    EndIf\n    If FALSE != ready\n    EndIf\n    If ready==false\n    EndIf\n    If ! ready == false\n    EndIf\n    If (ready && other) == false\n    EndIf\n    If (count as Bool) == false\n    EndIf\n    Return ready == true\n",
    );

    assert_eq!(
        repair(&source),
        function(
            "    If ready\n    EndIf\n    If ! ready\n    EndIf\n    If ! ready\n    EndIf\n    If ready\n    EndIf\n    If ready\n    EndIf\n    If ready\n    EndIf\n    If ! ready\n    EndIf\n    If ready\n    EndIf\n    If ! (ready && other)\n    EndIf\n    If ! (count as Bool)\n    EndIf\n    Return ready\n",
        )
    );
}

#[test]
fn rewrites_nested_and_literal_comparisons() {
    let source = function(
        "    If (ready == true) == false\n    EndIf\n    If true == false\n    EndIf\n    If TRUE == TRUE\n    EndIf\n",
    );

    assert_eq!(
        repair(&source),
        function("    If ! ready\n    EndIf\n    If false\n    EndIf\n    If TRUE\n    EndIf\n")
    );
}

#[test]
fn repair_preserves_a_non_bool_comparison() {
    let source = function("    If count == true\n    EndIf\n    If GetReady() == false\n    EndIf\n");

    assert_eq!(repair(&source), source);
}

#[test]
fn disable_directives_suppress_diagnostics_and_repairs() {
    let line_disabled = crate::lint(
        "ScriptName Example\n\nFunction Test(Bool ready)\n    If ready == true ; @disable boolean-simplification\n    EndIf\n    If ready == false\n    EndIf\nEndFunction\n",
        &crate::config::Config::default(),
    );
    let file_disabled = crate::lint(
        "; @disable-file boolean-simplification\nScriptName Example\n\nFunction Test(Bool ready)\n    If ready == true\n    EndIf\nEndFunction\n",
        &crate::config::Config::default(),
    );

    assert!(line_disabled.iter().all(|diagnostic| diagnostic.line != 4 || diagnostic.rule != RULE));
    assert!(line_disabled.iter().any(|diagnostic| diagnostic.rule == RULE && diagnostic.line == 6));
    assert!(file_disabled.iter().all(|diagnostic| diagnostic.rule != RULE));

    let source = "ScriptName Example\n\nFunction Test(Bool ready)\n    If ready == true ; @disable boolean-simplification\n    EndIf\n    If ready == false\n    EndIf\nEndFunction\n";
    assert_eq!(
        repair(source),
        "ScriptName Example\n\nFunction Test(Bool ready)\n    If ready == true ; @disable boolean-simplification\n    EndIf\n    If ! ready\n    EndIf\nEndFunction\n"
    );
}

#[test]
fn config_off_switch_suppresses_diagnostics() {
    let source = function("    If ready == true\n    EndIf\n");
    let mut config = crate::config::Config::default();
    config.rules.boolean_simplification = false;

    let diagnostics = crate::lint(&source, &config);

    assert!(diagnostics.iter().all(|diagnostic| diagnostic.rule != RULE));
}

#[test]
fn invalid_source_returns_no_diagnostics() {
    let diagnostics = check("ScriptName Example\n\nFunction Test(Bool ready)\n    If ready == true\nEndFunction\n");

    assert!(diagnostics.is_empty());
}

#[test]
fn direct_check_requires_both_the_ast_and_tokens() {
    let source = function("    If ready == true\n    EndIf\n");
    let ast = papyrus_parser::parse(&source).expect("fixture parses");
    let tokens = papyrus_parser::tokenize(&source).expect("fixture tokenizes");
    let config = crate::config::Config::default();

    assert!(super::check(
        &source,
        Some(&ast),
        None,
        &config,
        &mut crate::external_signatures::NoExternalSignatures,
    )
    .is_empty());
    assert!(super::check(
        &source,
        None,
        Some(&tokens),
        &config,
        &mut crate::external_signatures::NoExternalSignatures,
    )
    .is_empty());
}

#[test]
fn direct_check_fails_closed_when_ast_and_tokens_do_not_match() {
    let source = function("    If ready == true\n    EndIf\n");
    let ast = papyrus_parser::parse(&source).expect("fixture parses");
    let no_comparison = papyrus_parser::tokenize(&function("    If ready\n    EndIf\n"))
        .expect("fixture tokenizes");
    let different_comparison =
        papyrus_parser::tokenize(&function("    If ready != true\n    EndIf\n"))
            .expect("fixture tokenizes");
    let config = crate::config::Config::default();

    for tokens in [&no_comparison, &different_comparison] {
        assert!(super::check(
            &source,
            Some(&ast),
            Some(tokens),
            &config,
            &mut crate::external_signatures::NoExternalSignatures,
        )
        .is_empty());
    }
}

#[test]
fn checks_declaration_initializers_and_state_functions() {
    let source = "\
ScriptName Example

Bool Property Ready Auto
Bool Property Initial = Ready == true Auto
Bool Cached = Ready != false

Struct Options
    Bool Enabled = true == false
EndStruct

Group Settings
    Bool Property Grouped = Ready == false Auto
EndGroup

State Waiting
    Function Test(Bool fallback = true == false)
        Return fallback != true
    EndFunction
EndState
";

    let diagnostics = check_for_game(source, papyrus_lint_globals::Game::Fallout4);

    assert_eq!(
        diagnostics
            .iter()
            .map(|diagnostic| diagnostic.line)
            .collect::<Vec<_>>(),
        vec![4, 5, 8, 12, 16, 17]
    );
}

#[test]
fn checks_comparisons_in_each_statement_container() {
    let source = "\
ScriptName Example

Guard MainGuard

Function Test(Bool ready, Bool other)
    Bool local = ready == true
    local = other != false
    Consume(ready == false)
    If ready == true
        Return other == false
    ElseIf other != true
        local = ready != false
    Else
        local = other == true
    EndIf
    While ready == false
        local = other != false
    EndWhile
    LockGuard MainGuard
        local = ready == true
    EndLockGuard
    TryLockGuard MainGuard
        local = ready != true
    ElseTryLockGuard
        local = other == false
    EndTryLockGuard
EndFunction
";

    let diagnostics = check_for_game(source, papyrus_lint_globals::Game::Starfield);

    assert_eq!(diagnostics.len(), 13, "{diagnostics:?}");
    assert_eq!(diagnostics.first().map(|diagnostic| diagnostic.line), Some(6));
    assert_eq!(diagnostics.last().map(|diagnostic| diagnostic.line), Some(25));
}

#[test]
fn repair_handles_parenthesized_and_unary_operands() {
    let source = function(
        "    If ((ready)) == true\n    EndIf\n    If (ready && other) != true\n    EndIf\n    If ! (ready && other) == false\n    EndIf\n",
    );

    assert_eq!(
        repair(&source),
        function(
            "    If ready\n    EndIf\n    If ! (ready && other)\n    EndIf\n    If (ready && other)\n    EndIf\n",
        )
    );
}

#[test]
fn operand_rendering_preserves_expression_precedence() {
    assert_eq!(render_operand("ready && other", true), "! (ready && other)");
    assert_eq!(render_operand("(ready && other)", true), "! (ready && other)");
    assert_eq!(render_operand("! ready", true), "ready");
    assert_eq!(render_operand("((ready))", false), "ready");
    assert_eq!(render_operand("value as Bool", true), "! (value as Bool)");
    assert_eq!(render_operand("ready + other", true), "! (ready + other)");
    assert_eq!(render_operand("ready is Bool", true), "! (ready is Bool)");
    assert_eq!(render_operand("\"ready && other\"", true), "! \"ready && other\"");
    assert_eq!(render_operand("!= ready", true), "! (!= ready)");
}

#[test]
fn malformed_parentheses_are_not_stripped() {
    assert_eq!(strip_wrapping_parens("(ready) + other"), None);
    assert_eq!(strip_wrapping_parens("((ready)"), None);
    assert_eq!(strip_one_not("!= ready"), None);
}

#[test]
fn lexical_span_helpers_cover_every_lexeme_shape() {
    let source = "\"escaped \\\"text\" 0x2A 12.50 name && != + ;";

    assert_eq!(&source[..lexeme_end(source, 0)], "\"escaped \\\"text\"");
    let hex = source.find("0x2A").unwrap();
    assert_eq!(&source[hex..lexeme_end(source, hex)], "0x2A");
    let float = source.find("12.50").unwrap();
    assert_eq!(&source[float..lexeme_end(source, float)], "12.50");
    let word = source.find("name").unwrap();
    assert_eq!(&source[word..lexeme_end(source, word)], "name");
    let and = source.find("&&").unwrap();
    assert_eq!(&source[and..lexeme_end(source, and)], "&&");
    let not_equal = source.find("!=").unwrap();
    assert_eq!(&source[not_equal..lexeme_end(source, not_equal)], "!=");
    let plus = source.find('+').unwrap();
    assert_eq!(&source[plus..lexeme_end(source, plus)], "+");
    let semicolon = source.find(';').unwrap();
    assert_eq!(&source[semicolon..lexeme_end(source, semicolon)], ";");
    assert_eq!(lexeme_end(source, source.len()), source.len());

    assert_eq!(string_end(b"\"unterminated", 0), b"\"unterminated".len());
    assert_eq!(string_end(b"\"line\nnext", 0), 5);
}

#[test]
fn malformed_edit_ranges_are_ignored() {
    let reversed = Hit {
        line: 1,
        column: 1,
        start: 3,
        end: 2,
        message: String::new(),
        replacement: "x".to_string(),
    };
    let out_of_bounds = Hit {
        line: 1,
        column: 1,
        start: 0,
        end: 99,
        message: String::new(),
        replacement: "x".to_string(),
    };

    assert_eq!(apply_edits("ready", &[&reversed, &out_of_bounds]), "ready");
}

#[test]
fn empty_and_unbalanced_token_ranges_are_not_literals() {
    let tokens = papyrus_parser::tokenize("(true) false").unwrap();

    assert_eq!(literal_operand(&tokens, &(0..0)), None);
    assert!(!outer_parens_wrap(&tokens, 1, 1));
    assert!(!outer_parens_wrap(&tokens, 0, 2));
    assert_eq!(token_slice("(true) false", &[0], &tokens, &(1..1)), "");
    assert_eq!(token_slice("(true) false", &[0], &tokens, &(99..100)), "");
    assert_eq!(token_slice("(true) false", &[0], &tokens, &(0..99)), "");
}
