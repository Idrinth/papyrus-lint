use super::*;

fn check_with(source: &str, max_line_length: usize) -> Vec<Diagnostic> {
    super::check(
        source,
        None,
        None,
        &crate::config::Config {
            max_line_length,
            ..crate::config::Config::default()
        },
        &mut crate::external_signatures::NoExternalSignatures,
    )
}

#[test]
fn flags_each_line_over_the_configured_limit() {
    let diagnostics = check_with("12345\n123456\nabcdefg", 5);

    assert_eq!(diagnostics.len(), 2);
    assert_eq!((diagnostics[0].line, diagnostics[0].column), (2, 6));
    assert_eq!((diagnostics[1].line, diagnostics[1].column), (3, 6));
    assert_eq!(
        diagnostics[0].message,
        "[warning] Line is 6 characters long (maximum is 5)"
    );
}

#[test]
fn accepts_lines_at_the_limit_and_empty_input() {
    assert!(check_with("12345\n\nabc", 5).is_empty());
    assert!(check_with("", 5).is_empty());
}

#[test]
fn counts_characters_instead_of_utf8_bytes() {
    assert!(check_with("Hé 🌍", 4).is_empty());
    let diagnostics = check_with("Hé 🌍!", 4);
    assert_eq!((diagnostics[0].line, diagnostics[0].column), (1, 5));
}

#[test]
fn honors_rule_and_file_disable_directives() {
    let config = crate::config::Config {
        max_line_length: 10,
        ..crate::config::Config::default()
    };
    let line_disabled = "ScriptName Example ; @disable line-length\n";
    let file_disabled = "; @disable-file line-length\nScriptName Example\n";

    assert!(crate::lint(line_disabled, &config).is_empty());
    assert!(crate::lint(file_disabled, &config).is_empty());
}

#[test]
fn rule_switch_disables_the_check() {
    let mut config = crate::config::Config {
        max_line_length: 5,
        ..crate::config::Config::default()
    };
    config.rules.line_length = false;

    assert!(crate::lint("ScriptName Example", &config).is_empty());
}
