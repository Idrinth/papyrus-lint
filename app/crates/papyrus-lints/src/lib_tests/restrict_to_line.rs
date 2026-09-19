//! Tests for [`crate::restrict_to_line`] and [`crate::repaired_line`].

use super::super::*;
use super::support::config_with;

#[test]
fn restrict_to_line_keeps_only_the_target_line_changed() {
    let original = "Call(1,2)\nCall(3,4)\nCall(5,6)\n";
    let repaired = repair_filtered(original, &Config::default(), Some(comma_spacing::RULE));

    let restricted = restrict_to_line(original, &repaired, 2).unwrap();

    assert_eq!(restricted, "Call(1,2)\nCall(3, 4)\nCall(5,6)\n");
}

#[test]
fn restrict_to_line_returns_none_when_line_count_changes() {
    let original = "Call(1,2)\n";
    let repaired = "Call(1,2)\nCall(3,4)\n";

    assert_eq!(restrict_to_line(original, repaired, 1), None);
}

#[test]
fn restrict_to_line_out_of_range_leaves_original_unchanged() {
    let original = "Call(1,2)\nCall(3,4)\n";
    let repaired = "Call(1, 2)\nCall(3, 4)\n";

    assert_eq!(
        restrict_to_line(original, repaired, 0).as_deref(),
        Some(original)
    );
    assert_eq!(
        restrict_to_line(original, repaired, 4).as_deref(),
        Some(original)
    );
}

#[test]
fn restrict_to_line_can_replace_the_trailing_empty_line() {
    let original = "Call(1,2)\n";
    let repaired = "Call(1,2)\nreplacement";

    assert_eq!(
        restrict_to_line(original, repaired, 2).as_deref(),
        Some("Call(1,2)\nreplacement")
    );
}

#[test]
fn repaired_line_returns_just_the_target_line_after_the_fix() {
    let source = "Call(1,2)\nCall(3,4)\nCall(5,6)\n";
    let config = Config::default();

    assert_eq!(
        repaired_line(source, &config, comma_spacing::RULE, 2).as_deref(),
        Some("Call(3, 4)")
    );
}

#[test]
fn repaired_line_is_none_when_the_rule_does_not_change_the_source_at_all() {
    let source = "Call(1, 2)\n";

    assert_eq!(
        repaired_line(source, &Config::default(), comma_spacing::RULE, 1),
        None
    );
}

#[test]
fn repaired_line_is_none_when_the_fix_shifts_the_line_count() {
    let source = "ScriptName Example\n\nInt Property Zulu = 1 Auto\nActor Property Alpha Auto\n";
    let config = config_with(|c| c.rules.property_sorting = true);

    assert_eq!(
        repaired_line(source, &config, property_sorting::RULE, 3),
        None
    );
}

#[test]
fn repaired_line_is_none_for_an_out_of_range_line() {
    let source = "Call(1,2)\n";

    assert_eq!(
        repaired_line(source, &Config::default(), comma_spacing::RULE, 99),
        None
    );
}

#[test]
fn repaired_line_is_none_when_the_fix_touches_other_lines_but_not_the_target_one() {
    let source = "Call(1,2)\nCall(3, 4)\n";

    assert_eq!(
        repaired_line(source, &Config::default(), comma_spacing::RULE, 2),
        None
    );
}
