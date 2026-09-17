//! Integration test using a real-world Papyrus fragment script, complete
//! with CRLF line endings and tab indentation, to make sure the trailing
//! whitespace lint only flags genuine trailing whitespace.

use papyrus_lints::{lint, repair_filtered, Config};

const FIXTURE: &str = include_str!("fixtures/IDR__TIF__050002AB.psc");

fn trailing_whitespace(source: &str) -> Vec<papyrus_lints::Diagnostic> {
    lint(source, &Config::default())
        .into_iter()
        .filter(|diagnostic| diagnostic.rule == "trailing-whitespace")
        .collect()
}

#[test]
fn flags_only_the_property_lines_with_trailing_spaces() {
    let diagnostics = trailing_whitespace(FIXTURE);

    let flagged_lines: Vec<usize> = diagnostics.iter().map(|d| d.line).collect();
    assert_eq!(flagged_lines, vec![24, 26, 28]);

    for diagnostic in &diagnostics {
        assert_eq!(
            diagnostic.message,
            "[warning] Line contains trailing whitespace"
        );
    }
}

#[test]
fn does_not_flag_crlf_line_endings_or_tab_indentation() {
    let diagnostics = trailing_whitespace(FIXTURE);

    // Tab-indented lines in the fixture (e.g. the `while` loop body) don't
    // have trailing whitespace, only leading tabs, so they must not appear.
    assert!(!diagnostics.iter().any(|d| d.line == 10));
}

#[test]
fn repair_clears_all_trailing_whitespace_diagnostics() {
    let repaired = repair_filtered(FIXTURE, &Config::default(), Some("trailing-whitespace"));
    assert!(trailing_whitespace(&repaired).is_empty());
}

#[test]
fn repair_only_changes_the_flagged_lines() {
    let repaired = repair_filtered(FIXTURE, &Config::default(), Some("trailing-whitespace"));

    let original_lines: Vec<&str> = FIXTURE.lines().collect();
    let repaired_lines: Vec<&str> = repaired.lines().collect();
    assert_eq!(original_lines.len(), repaired_lines.len());

    let flagged_lines = [24, 26, 28];
    for (index, (original, fixed)) in original_lines.iter().zip(&repaired_lines).enumerate() {
        let line_number = index + 1;
        if flagged_lines.contains(&line_number) {
            assert_ne!(
                original, fixed,
                "line {line_number} should have been repaired"
            );
        } else {
            assert_eq!(original, fixed, "line {line_number} should be unchanged");
        }
    }
}
