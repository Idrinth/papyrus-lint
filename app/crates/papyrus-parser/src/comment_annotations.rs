//! Parsing for `@name` annotations embedded in `;` line comments.

/// A single annotation and the text belonging to it, up to the next
/// annotation in the same comment.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CommentAnnotation<'a> {
    pub name: &'a str,
    pub arguments: &'a str,
    /// One-indexed source column of the leading `@`.
    pub column: usize,
    /// Byte offsets in the complete source line, excluding separators before
    /// the next annotation.
    pub byte_start: usize,
    pub byte_end: usize,
    pub arguments_byte_start: usize,
}

/// Parses every annotation in a source line's trailing `;` comment.
pub fn parse_line_annotations(line: &str) -> Vec<CommentAnnotation<'_>> {
    let Some((comment_offset, comment)) = line_comment(line) else {
        return Vec::new();
    };
    let bytes = comment.as_bytes();
    let mut starts = Vec::new();
    let mut index = 0;
    while index < bytes.len() {
        if bytes[index] != b'@'
            || (index > 0 && bytes[index - 1] != b',' && !bytes[index - 1].is_ascii_whitespace())
        {
            index += 1;
            continue;
        }
        let name_start = index + 1;
        let mut end = name_start;
        while bytes
            .get(end)
            .is_some_and(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-'))
        {
            end += 1;
        }
        if end > name_start {
            starts.push((index, name_start, end));
            index = end;
        } else {
            index += 1;
        }
    }

    starts
        .iter()
        .enumerate()
        .map(|(position, &(start, name_start, name_end))| {
            let arguments_end = starts
                .get(position + 1)
                .map_or(comment.len(), |&(next, _, _)| next);
            let raw = &comment[name_end..arguments_end];
            let raw_end = raw.trim_end().trim_end_matches(',').trim_end();
            let arguments = raw_end.trim_start();
            CommentAnnotation {
                name: &comment[name_start..name_end],
                arguments,
                column: line[..comment_offset + start].chars().count() + 1,
                byte_start: comment_offset + start,
                byte_end: comment_offset + name_end + raw_end.len(),
                arguments_byte_start: comment_offset + name_end + raw_end.len() - arguments.len(),
            }
        })
        .collect()
}

/// Returns the text after the `;` starting a line comment and its byte offset.
pub fn line_comment(line: &str) -> Option<(usize, &str)> {
    let bytes = line.as_bytes();
    let mut in_string = false;
    let mut index = 0;
    while index < bytes.len() {
        match bytes[index] {
            b'"' => in_string = !in_string,
            b'\\' if in_string => index += 1,
            b';' if !in_string => {
                return if bytes.get(index + 1) == Some(&b'/') {
                    None
                } else {
                    Some((index + 1, &line[index + 1..]))
                };
            }
            _ => {}
        }
        index += 1;
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_multiple_annotations_and_separates_arguments() {
        let annotations = parse_line_annotations(
            "Function Old() ; @deprecated Use New() @nodiscard, @disable old-api, style",
        );
        assert_eq!(annotations.len(), 3);
        assert_eq!(annotations[0].name, "deprecated");
        assert_eq!(annotations[0].arguments, "Use New()");
        assert_eq!(annotations[1].name, "nodiscard");
        assert_eq!(annotations[1].arguments, "");
        assert_eq!(annotations[2].name, "disable");
        assert_eq!(annotations[2].arguments, "old-api, style");
    }

    #[test]
    fn ignores_non_line_comment_annotations() {
        assert!(parse_line_annotations(r#"Debug.Trace("; @private")"#).is_empty());
        assert!(parse_line_annotations(";/ @private /;").is_empty());
        assert!(parse_line_annotations("; mail user@example.com").is_empty());
    }
}
