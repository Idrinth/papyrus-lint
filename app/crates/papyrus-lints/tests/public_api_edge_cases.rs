//! Edge-case coverage for the crate's black-box lint and repair API.

use papyrus_lints::{lint, repair, repair_filtered, restrict_to_line, Config};

#[test]
fn empty_source_is_a_stable_noop() {
    let config = Config::default();

    assert!(lint("", &config).is_empty());
    assert_eq!(repair("", &config), "");
    assert_eq!(repair_filtered("", &config, Some("comma-spacing")), "");
}

#[test]
fn clean_source_is_not_rewritten() {
    let source = "ScriptName Example\r\n\r\nFunction Run(Int Left, Int Right)\r\n\tCall(Left, Right)\r\nEndFunction\r\n";
    let config = Config::default();

    assert_eq!(repair(source, &config), source);
}

#[test]
fn filtered_repair_preserves_unicode_and_unrelated_text() {
    let source = "String Message = \"héllo,world\"\n; λ,μ\nCall(1,2)\n";

    assert_eq!(
        repair_filtered(source, &Config::default(), Some("comma-spacing")),
        "String Message = \"héllo,world\"\n; λ,μ\nCall(1, 2)\n"
    );
}

#[test]
fn line_restriction_preserves_mixed_line_endings_outside_the_target() {
    let original = "Call(1,2)\r\nCall(3,4)\nCall(5,6)\r\n";
    let repaired = repair_filtered(original, &Config::default(), Some("comma-spacing"));

    assert_eq!(
        restrict_to_line(original, &repaired, 2),
        Some("Call(1,2)\r\nCall(3, 4)\nCall(5,6)\r\n".to_string())
    );
}

#[test]
fn line_restriction_can_apply_a_fix_to_the_first_or_last_line() {
    let original = "Call(1,2)\nCall(3,4)";
    let repaired = repair_filtered(original, &Config::default(), Some("comma-spacing"));

    assert_eq!(
        restrict_to_line(original, &repaired, 1),
        Some("Call(1, 2)\nCall(3,4)".to_string())
    );
    assert_eq!(
        restrict_to_line(original, &repaired, 2),
        Some("Call(1,2)\nCall(3, 4)".to_string())
    );
}
