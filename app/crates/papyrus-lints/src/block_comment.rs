//! Detects `;/ ... /;` block comments so line-based lints never touch
//! their delimiters or content.
//!
//! Papyrus block comments spend `;` on half of both delimiters (`;/` to
//! open, `/;` to close). A lint that treats every trailing `;` as an
//! independent, insertable/removable marker -- as [`crate::semicolon`]
//! does -- can otherwise corrupt a closing `/;` into a bare `/`, breaking
//! the comment (and the script after it). Any line touched by a block
//! comment must be left exactly as-is by such lints.

/// Returns, for each 1-indexed line of `source`, whether any part of the
/// line falls inside a `;/ .. /;` block comment, including the delimiter
/// characters themselves. A `"..."` string literal's contents are never
/// mistaken for a delimiter, matching `papyrus_parser`'s own lexer; a plain
/// `;` line comment (not followed by `/`) runs to the end of its line, so
/// nothing after it can open one either. Index `0` is unused; the returned
/// vector has `source.lines().count() + 1` entries.
pub fn protected_lines(source: &str) -> Vec<bool> {
    let line_count = source.lines().count() + 1;
    let mut protected = vec![false; line_count];
    let mut in_comment = false;

    for (index, line) in source.lines().enumerate() {
        protected[index + 1] = scan_line(line, &mut in_comment);
    }

    protected
}

/// Scans a single line, advancing `in_comment` across its start/end state,
/// and reports whether the line touched a block comment at any point.
fn scan_line(line: &str, in_comment: &mut bool) -> bool {
    let bytes = line.as_bytes();
    let mut pos = 0;
    let mut touched = *in_comment;

    while pos < bytes.len() {
        if *in_comment {
            match line[pos..].find("/;") {
                Some(offset) => {
                    *in_comment = false;
                    pos += offset + 2;
                    touched = true;
                }
                None => break,
            }
        } else {
            match bytes[pos] {
                b'"' => {
                    pos += 1;
                    while pos < bytes.len() && bytes[pos] != b'"' {
                        pos += if bytes[pos] == b'\\' { 2 } else { 1 };
                    }
                    pos += 1;
                }
                b';' if bytes.get(pos + 1) == Some(&b'/') => {
                    *in_comment = true;
                    pos += 2;
                    touched = true;
                }
                b';' => break,
                _ => pos += 1,
            }
        }
    }

    touched
}

#[cfg(test)]
#[path = "block_comment_tests.rs"]
mod tests;
