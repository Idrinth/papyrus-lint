//! Tests for the `ExternalSignatures`-aware repair entry points:
//! [`crate::repair_with_external_arguments`],
//! [`crate::repair_filtered_with_external_arguments`],
//! [`crate::repair_filtered_by_tag_with_external_arguments`], and
//! [`crate::repair_selected_with_external_arguments`].

use super::super::*;
use super::support::*;

#[test]
fn repair_with_external_arguments_removes_an_unused_import_resolved_through_external() {
    let source = "ScriptName Example\n\nImport Helpers\n\nFunction Test()\nEndFunction\n";

    let repaired = repair_with_external_arguments(
        source,
        &Config::default(),
        &mut FakeExternalWithUnusedImport,
    );

    assert_eq!(
        repaired,
        "ScriptName Example\n\n\nFunction Test()\nEndFunction\n"
    );
    assert_eq!(
        repair(source, &Config::default()),
        source,
        "the plain, resolver-less repair must remain a no-op for unused-import"
    );
}

#[test]
fn repair_filtered_with_external_arguments_only_removes_the_named_rule() {
    let source =
        "ScriptName Example\n\nImport Helpers\n\nFunction Test()\n    Call(1,2)\nEndFunction\n";

    let unused_import_only = repair_filtered_with_external_arguments(
        source,
        &Config::default(),
        &mut FakeExternalWithUnusedImport,
        Some(unused_import::RULE),
    );
    assert_eq!(
        unused_import_only,
        "ScriptName Example\n\n\nFunction Test()\n    Call(1,2)\nEndFunction\n"
    );

    let comma_only = repair_filtered_with_external_arguments(
        source,
        &Config::default(),
        &mut FakeExternalWithUnusedImport,
        Some(comma_spacing::RULE),
    );
    assert_eq!(
        comma_only,
        "ScriptName Example\n\nImport Helpers\n\nFunction Test()\n    Call(1, 2)\nEndFunction\n"
    );
}

#[test]
fn repair_filtered_by_tag_with_external_arguments_matches_unused_imports_own_tag() {
    let source = "ScriptName Example\n\nImport Helpers\n\nFunction Test()\nEndFunction\n";

    let maintainability = repair_filtered_by_tag_with_external_arguments(
        source,
        &Config::default(),
        &mut FakeExternalWithUnusedImport,
        Some("maintainability"),
    );
    assert_eq!(
        maintainability,
        "ScriptName Example\n\n\nFunction Test()\nEndFunction\n"
    );

    let style_only = repair_filtered_by_tag_with_external_arguments(
        source,
        &Config::default(),
        &mut FakeExternalWithUnusedImport,
        Some("style"),
    );
    assert_eq!(style_only, source);
}

#[test]
fn repair_with_external_arguments_skips_unused_import_when_its_rule_is_disabled() {
    let source = "ScriptName Example\n\nImport Helpers\n\nFunction Test()\nEndFunction\n";
    let disabled_config = config_with(|c| c.rules.unused_import = false);

    assert_eq!(
        repair_with_external_arguments(source, &disabled_config, &mut FakeExternalWithUnusedImport),
        source
    );
}

#[test]
fn repair_selected_prefers_a_tag_filter_over_a_rule_filter() {
    let source = "Call(1,2)  \n";

    let repaired = repair_selected_with_external_arguments(
        source,
        &Config::default(),
        &mut FakeExternalWithUnusedImport,
        Some(comma_spacing::RULE),
        Some("style"),
        None,
    )
    .unwrap();

    assert_eq!(repaired, "Call(1, 2)\n");
}

#[test]
fn repair_selected_can_restrict_a_named_fix_to_one_line() {
    let source = "Call(1,2)\nCall(3,4)\n";

    let repaired = repair_selected_with_external_arguments(
        source,
        &Config::default(),
        &mut FakeExternalWithUnusedImport,
        Some(comma_spacing::RULE),
        None,
        Some(2),
    )
    .unwrap();

    assert_eq!(repaired, "Call(1,2)\nCall(3, 4)\n");
}

#[test]
fn repair_selected_returns_none_for_a_line_count_shifting_fix() {
    let source = "ScriptName Example\n\nImport Helpers\n\nFunction Test()\nEndFunction\n";

    let repaired = repair_selected_with_external_arguments(
        source,
        &Config::default(),
        &mut FakeExternalWithUnusedImport,
        Some(unused_import::RULE),
        None,
        Some(3),
    );

    assert_eq!(repaired, None);
}
