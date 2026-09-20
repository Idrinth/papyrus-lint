//! Adds an `; @nodiscard` line comment to a function's header, the
//! insertion counterpart of [`crate::unused_nodiscard`], which only reads
//! the flag back out (from the header line or the line directly above it).
//! Unlike [`crate::disable_comments`]'s `@disable`, this is a single flag
//! with no rule list to merge, so an existing trailing comment is simply
//! extended with `@nodiscard` rather than parsed for prior entries.

use crate::unused_nodiscard::{line_comment_text, line_has_nodiscard};

/// Adds `; @nodiscard` to `line` (1-indexed) of `source`'s function header,
/// or extends its existing trailing comment, driving the desktop app's and
/// VS Code extension's "Add nodiscard" quick action. Left untouched if
/// `line` is out of range, or if `line` (or the line directly above it)
/// already carries the flag -- matching the lookback
/// [`crate::unused_nodiscard`] itself reads the flag with.
pub(crate) fn add_nodiscard_directive(source: &str, line: usize) -> String {
    let Some(index) = line.checked_sub(1) else {
        return source.to_string();
    };
    let lines: Vec<&str> = source.split('\n').collect();
    if lines.get(index).is_none() {
        return source.to_string();
    }
    if already_nodiscard(&lines, index) {
        return source.to_string();
    }
    let replaced = add_nodiscard_to_line(lines[index]);
    lines
        .iter()
        .enumerate()
        .map(|(i, original)| {
            if i == index {
                replaced.as_str()
            } else {
                original
            }
        })
        .collect::<Vec<_>>()
        .join("\n")
}

fn already_nodiscard(lines: &[&str], index: usize) -> bool {
    line_has_nodiscard(lines[index])
        || index
            .checked_sub(1)
            .and_then(|prev| lines.get(prev))
            .is_some_and(|line| line_has_nodiscard(line))
}

fn add_nodiscard_to_line(line: &str) -> String {
    let (content, trailing_cr) = match line.strip_suffix('\r') {
        Some(stripped) => (stripped, "\r"),
        None => (line, ""),
    };
    let separator = if line_comment_text(content).is_some() {
        " "
    } else {
        " ; "
    };
    format!("{content}{separator}@nodiscard{trailing_cr}")
}

#[cfg(test)]
#[path = "nodiscard_comments_tests.rs"]
mod tests;
