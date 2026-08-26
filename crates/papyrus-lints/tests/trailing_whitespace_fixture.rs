//! Integration test using a real-world Papyrus fragment script, complete
//! with CRLF line endings and tab indentation, to make sure the trailing
//! whitespace lint only flags genuine trailing whitespace.

const FIXTURE: &str = include_str!("fixtures/IDR__TIF__050002AB.psc");

#[test]
fn flags_only_the_property_lines_with_trailing_spaces() {
    let diagnostics = papyrus_lints::trailing_whitespace::check(FIXTURE);

    let flagged_lines: Vec<usize> = diagnostics.iter().map(|d| d.line).collect();
    assert_eq!(flagged_lines, vec![24, 26, 28]);

    for diagnostic in &diagnostics {
        assert_eq!(diagnostic.message, "Line contains trailing whitespace");
    }
}

#[test]
fn does_not_flag_crlf_line_endings_or_tab_indentation() {
    let diagnostics = papyrus_lints::trailing_whitespace::check(FIXTURE);

    // Tab-indented lines in the fixture (e.g. the `while` loop body) don't
    // have trailing whitespace, only leading tabs, so they must not appear.
    assert!(!diagnostics.iter().any(|d| d.line == 10));
}
