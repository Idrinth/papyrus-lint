use super::*;

#[test]
fn deep_merge_recurses_through_mappings_and_replaces_scalar_values() {
    let base = serde_norway::from_str(
        "semicolon: false\nrules:\n  property_sorting: false\n  trailing_whitespace: true\n",
    )
    .expect("base YAML should parse");
    let over = serde_norway::from_str(
        "semicolon: true\nrules:\n  property_sorting: true\nnew_setting: value\n",
    )
    .expect("override YAML should parse");

    let merged = deep_merge(base, over);
    let expected: serde_norway::Value = serde_norway::from_str(
        "semicolon: true\nrules:\n  property_sorting: true\n  trailing_whitespace: true\nnew_setting: value\n",
    )
    .expect("expected YAML should parse");

    assert_eq!(merged, expected);
}

#[test]
fn deep_merge_replaces_a_mapping_with_a_non_mapping_override() {
    let base = serde_norway::from_str("rules:\n  trailing_whitespace: true\n")
        .expect("base YAML should parse");
    let over = serde_norway::from_str("rules: disabled\n").expect("override YAML should parse");

    let merged = deep_merge(base, over);
    let expected: serde_norway::Value =
        serde_norway::from_str("rules: disabled\n").expect("expected YAML should parse");

    assert_eq!(merged, expected);
}
