//! Renders a standard unified diff (the same hunk format `diff -u`/`git
//! diff` produce) between a script's original source and the source an
//! automatic fix would produce, for previewing a fix without writing it to
//! disk: the CLI's `fix --dry-run` flag and the desktop app's code viewer
//! "Preview fixes" button (`preview_repair_psc_file` in
//! `app/src-tauri/src/repair.rs`) both call [`unified_diff`] to show what a fix
//! *would* change.
//!
//! Built on the [`similar`] crate's line-based [`TextDiff`], rather than a
//! hand-rolled LCS implementation, since a well-tested diff library already
//! produces the exact hunk grouping/formatting `diff -u` does.

use std::borrow::Cow;

use similar::TextDiff;

const CONTEXT_LINES: usize = 3;

/// Appends a trailing newline to non-empty `text` that lacks one, so a
/// source's own "missing newline at end of file" quirk never registers as a
/// line change by itself -- matching this module's previous behavior, which
/// never emitted `diff`'s "\ No newline at end of file" marker either. An
/// empty string is left alone, since that represents zero lines rather than
/// one empty line.
fn with_trailing_newline(text: &str) -> Cow<'_, str> {
    if text.is_empty() || text.ends_with('\n') {
        Cow::Borrowed(text)
    } else {
        Cow::Owned(format!("{text}\n"))
    }
}

/// Renders a standard unified diff between `original` and `updated`,
/// labeling both sides with `path_display` (the same path shown for this
/// script elsewhere in the report). Returns an empty string when the two
/// are identical.
pub fn unified_diff(path_display: &str, original: &str, updated: &str) -> String {
    let original = with_trailing_newline(original);
    let updated = with_trailing_newline(updated);

    TextDiff::from_lines(original.as_ref(), updated.as_ref())
        .unified_diff()
        .context_radius(CONTEXT_LINES)
        .missing_newline_hint(false)
        .header(path_display, path_display)
        .to_string()
}

#[cfg(test)]
#[path = "diff_tests.rs"]
mod tests;
