//! Recognizes CreationKit-generated "Fragment Code" blocks so formatting
//! lints/fixes leave the parts CreationKit expects untouched.
//!
//! CreationKit writes fragment scripts (quest/dialogue/scene fragments)
//! with a `;BEGIN FRAGMENT CODE ... ;END FRAGMENT CODE` comment block
//! wrapping boilerplate it manages itself (fragment headers, the
//! generated function signature, `EndFunction`, and the markers
//! themselves). The only part of that block a user actually edits -- and
//! the only part it's safe to reformat -- is the script code between a
//! `;BEGIN CODE`/`;END CODE` pair. Reformatting anything else in the block
//! (even something as small as re-indenting a line or adding a trailing
//! semicolon to a comment) makes CreationKit fail to recognize the
//! fragment.

#[derive(PartialEq, Eq)]
enum State {
    Outside,
    Wrapper,
    Code(usize),
}

/// Walks `source` once, tracking which `;BEGIN FRAGMENT CODE`/`;BEGIN CODE`
/// section (if any) each 1-indexed line falls inside. Shared by
/// [`protected_lines`] and [`code_section_starts`] so the two never drift
/// apart on what counts as a marker line.
fn classify_lines(source: &str) -> (Vec<bool>, Vec<Option<usize>>) {
    let mut state = State::Outside;
    let line_count = source.lines().count() + 1;
    let mut protected = vec![false; line_count];
    let mut code_section_start = vec![None; line_count];

    for (index, line) in source.lines().enumerate() {
        let line_number = index + 1;
        let marker = line.trim_start().to_ascii_uppercase();

        let is_marker_line = if marker.starts_with(";BEGIN FRAGMENT CODE") {
            state = State::Wrapper;
            true
        } else if marker.starts_with(";END FRAGMENT CODE") {
            let was_inside = state != State::Outside;
            state = State::Outside;
            was_inside
        } else if state == State::Wrapper && marker.starts_with(";BEGIN CODE") {
            state = State::Code(line_number);
            true
        } else if matches!(state, State::Code(_)) && marker.starts_with(";END CODE") {
            state = State::Wrapper;
            true
        } else {
            false
        };

        protected[line_number] = is_marker_line || state == State::Wrapper;
        if !is_marker_line {
            if let State::Code(begin_line) = state {
                code_section_start[line_number] = Some(begin_line);
            }
        }
    }

    (protected, code_section_start)
}

/// Returns, for each 1-indexed line of `source`, whether it falls inside a
/// `;BEGIN FRAGMENT CODE`/`;END FRAGMENT CODE` block but outside any
/// `;BEGIN CODE`/`;END CODE` pair nested within it. Formatting lints/fixes
/// must leave lines marked `true` completely untouched. Index `0` is
/// unused (always `false`); the returned vector has
/// `source.lines().count() + 1` entries.
pub fn protected_lines(source: &str) -> Vec<bool> {
    classify_lines(source).0
}

/// Returns, for each 1-indexed line of `source`, the line number of the
/// `;BEGIN CODE` marker that opened the fragment-code section it falls
/// inside (`None` for lines outside any such section, and for the marker
/// lines themselves). CreationKit writes the code between a `;BEGIN
/// CODE`/`;END CODE` pair flush with that marker rather than nested inside
/// the (never-reindented) wrapper function around it, so callers computing
/// expected indentation should measure a line's depth relative to its
/// `;BEGIN CODE` marker's own depth instead of from the top of the file.
/// Index `0` is unused; the returned vector has `source.lines().count() +
/// 1` entries.
pub fn code_section_starts(source: &str) -> Vec<Option<usize>> {
    classify_lines(source).1
}

#[cfg(test)]
#[path = "fragment_code_tests.rs"]
mod tests;
