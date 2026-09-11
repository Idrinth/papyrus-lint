//! Renders a standard unified diff (the same hunk format `diff -u`/`git
//! diff` produce) between a script's original source and the source an
//! automatic fix would produce, for the `fix --dry-run` flag (see
//! `lib.rs`): shows what a fix *would* change without writing anything to
//! disk.
//!
//! This is a small self-contained line-based diff (an LCS alignment,
//! grouped into hunks with three lines of context, matching `diff -u`'s
//! default) rather than a dependency on an external diff crate, since the
//! CLI doesn't otherwise need one. It doesn't emit `diff`'s "\ No newline
//! at end of file" marker for a file that doesn't end in a trailing
//! newline — a rare case for a `.psc` script, and the diff is meant for a
//! human to review rather than to be fed back into `patch`.

const CONTEXT_LINES: usize = 3;

/// Guards the LCS table below (`O(old.len() * new.len())` cells) from
/// consuming excessive memory on a pathologically large input; above this
/// many cells, [`unified_diff`] falls back to a single hunk that replaces
/// the whole file wholesale instead of computing a minimal diff.
const MAX_LCS_CELLS: usize = 20_000_000;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum EditKind {
    Equal,
    Delete,
    Insert,
}

/// A contiguous run of same-kind edits: `old[old_start..old_end]` and/or
/// `new[new_start..new_end]`, mirroring one opcode from Python's
/// `difflib.SequenceMatcher.get_opcodes()`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct OpRange {
    kind: EditKind,
    old_start: usize,
    old_end: usize,
    new_start: usize,
    new_end: usize,
}

/// Splits `source` into lines without their trailing `\n` (a trailing
/// `\r`, if any, stays part of the line, so a script's own line-ending
/// style is preserved in the diff output).
fn split_lines(source: &str) -> Vec<&str> {
    if source.is_empty() {
        return Vec::new();
    }
    let mut lines: Vec<&str> = source.split('\n').collect();
    if lines.last() == Some(&"") {
        lines.pop();
    }
    lines
}

/// Computes the minimal sequence of equal/delete/insert opcodes turning
/// `old` into `new`, via a classic LCS dynamic-programming table. Falls
/// back to a single wholesale replace opcode when the table would exceed
/// [`MAX_LCS_CELLS`].
fn compute_opcodes(old: &[&str], new: &[&str]) -> Vec<OpRange> {
    let n = old.len();
    let m = new.len();

    if (n + 1).saturating_mul(m + 1) > MAX_LCS_CELLS {
        let mut ops = Vec::new();
        if n > 0 {
            ops.push(OpRange {
                kind: EditKind::Delete,
                old_start: 0,
                old_end: n,
                new_start: 0,
                new_end: 0,
            });
        }
        if m > 0 {
            ops.push(OpRange {
                kind: EditKind::Insert,
                old_start: n,
                old_end: n,
                new_start: 0,
                new_end: m,
            });
        }
        return ops;
    }

    let mut dp = vec![0u32; (n + 1) * (m + 1)];
    let idx = |i: usize, j: usize| i * (m + 1) + j;
    for i in (0..n).rev() {
        for j in (0..m).rev() {
            dp[idx(i, j)] = if old[i] == new[j] {
                dp[idx(i + 1, j + 1)] + 1
            } else {
                dp[idx(i + 1, j)].max(dp[idx(i, j + 1)])
            };
        }
    }

    #[derive(PartialEq, Eq, Clone, Copy)]
    enum LineOp {
        Equal,
        Delete,
        Insert,
    }
    let mut per_line = Vec::with_capacity(n + m);
    let (mut i, mut j) = (0usize, 0usize);
    while i < n && j < m {
        if old[i] == new[j] {
            per_line.push(LineOp::Equal);
            i += 1;
            j += 1;
        } else if dp[idx(i + 1, j)] >= dp[idx(i, j + 1)] {
            per_line.push(LineOp::Delete);
            i += 1;
        } else {
            per_line.push(LineOp::Insert);
            j += 1;
        }
    }
    while i < n {
        per_line.push(LineOp::Delete);
        i += 1;
    }
    while j < m {
        per_line.push(LineOp::Insert);
        j += 1;
    }

    let mut ops = Vec::new();
    let (mut oi, mut ni) = (0usize, 0usize);
    let mut idx_in_line = 0;
    while idx_in_line < per_line.len() {
        let kind = per_line[idx_in_line];
        let mut count = 0usize;
        while idx_in_line < per_line.len() && per_line[idx_in_line] == kind {
            idx_in_line += 1;
            count += 1;
        }
        let (old_start, old_end, new_start, new_end) = match kind {
            LineOp::Equal => (oi, oi + count, ni, ni + count),
            LineOp::Delete => (oi, oi + count, ni, ni),
            LineOp::Insert => (oi, oi, ni, ni + count),
        };
        oi = old_end;
        ni = new_end;
        ops.push(OpRange {
            kind: match kind {
                LineOp::Equal => EditKind::Equal,
                LineOp::Delete => EditKind::Delete,
                LineOp::Insert => EditKind::Insert,
            },
            old_start,
            old_end,
            new_start,
            new_end,
        });
    }
    ops
}

/// Groups `opcodes` into hunks, each carrying up to [`CONTEXT_LINES`] of
/// unchanged context before/after its changes, splitting a long run of
/// unchanged lines between two distant changes into separate hunks —
/// mirroring `difflib.SequenceMatcher.get_grouped_opcodes()`.
fn group_opcodes(opcodes: &[OpRange], context: usize) -> Vec<Vec<OpRange>> {
    if opcodes.is_empty() {
        return Vec::new();
    }

    let mut codes = opcodes.to_vec();
    if let Some(first) = codes.first_mut() {
        if first.kind == EditKind::Equal {
            let clipped_old_start = first.old_end.saturating_sub(context).max(first.old_start);
            let clipped_new_start = first.new_end.saturating_sub(context).max(first.new_start);
            first.old_start = clipped_old_start;
            first.new_start = clipped_new_start;
        }
    }
    if let Some(last) = codes.last_mut() {
        if last.kind == EditKind::Equal {
            let clipped_old_end = (last.old_start + context).min(last.old_end);
            let clipped_new_end = (last.new_start + context).min(last.new_end);
            last.old_end = clipped_old_end;
            last.new_end = clipped_new_end;
        }
    }

    let group_threshold = context * 2;
    let mut groups: Vec<Vec<OpRange>> = Vec::new();
    let mut group: Vec<OpRange> = Vec::new();
    for op in codes {
        if op.kind == EditKind::Equal && op.old_end - op.old_start > group_threshold {
            group.push(OpRange {
                old_end: (op.old_start + context).min(op.old_end),
                new_end: (op.new_start + context).min(op.new_end),
                ..op
            });
            groups.push(std::mem::take(&mut group));
            group.push(OpRange {
                old_start: op.old_end.saturating_sub(context).max(op.old_start),
                new_start: op.new_end.saturating_sub(context).max(op.new_start),
                ..op
            });
            continue;
        }
        group.push(op);
    }
    if !(group.len() == 1 && group[0].kind == EditKind::Equal) {
        groups.push(group);
    }
    groups
}

/// Formats one hunk's `@@ -old_start,old_count +new_start,new_count @@`
/// header, using `diff`'s convention that an empty range is reported at
/// the (1-indexed) line before it rather than line `0`.
fn format_hunk_header(group: &[OpRange]) -> String {
    let first = group.first().expect("a hunk always has at least one op");
    let last = group.last().expect("a hunk always has at least one op");

    let old_count = last.old_end - first.old_start;
    let new_count = last.new_end - first.new_start;
    let old_start_display = if old_count == 0 {
        first.old_start
    } else {
        first.old_start + 1
    };
    let new_start_display = if new_count == 0 {
        first.new_start
    } else {
        first.new_start + 1
    };

    format!("@@ -{old_start_display},{old_count} +{new_start_display},{new_count} @@")
}

/// Renders a standard unified diff between `original` and `updated`,
/// labeling both sides with `path_display` (the same path shown for this
/// script elsewhere in the report). Returns an empty string when the two
/// are identical.
pub(crate) fn unified_diff(path_display: &str, original: &str, updated: &str) -> String {
    let old = split_lines(original);
    let new = split_lines(updated);
    if old == new {
        return String::new();
    }

    let opcodes = compute_opcodes(&old, &new);
    let groups = group_opcodes(&opcodes, CONTEXT_LINES);

    let mut out = String::new();
    out.push_str(&format!("--- {path_display}\n"));
    out.push_str(&format!("+++ {path_display}\n"));

    for group in groups {
        out.push_str(&format_hunk_header(&group));
        out.push('\n');
        for op in group {
            match op.kind {
                EditKind::Equal => {
                    for line in &old[op.old_start..op.old_end] {
                        out.push(' ');
                        out.push_str(line);
                        out.push('\n');
                    }
                }
                EditKind::Delete => {
                    for line in &old[op.old_start..op.old_end] {
                        out.push('-');
                        out.push_str(line);
                        out.push('\n');
                    }
                }
                EditKind::Insert => {
                    for line in &new[op.new_start..op.new_end] {
                        out.push('+');
                        out.push_str(line);
                        out.push('\n');
                    }
                }
            }
        }
    }

    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_diff_for_identical_source() {
        assert_eq!(unified_diff("a.psc", "same\ntext\n", "same\ntext\n"), "");
    }

    #[test]
    fn single_line_change_produces_one_hunk() {
        let diff = unified_diff("a.psc", "one\ntwo \nthree\n", "one\ntwo\nthree\n");

        assert_eq!(
            diff,
            "--- a.psc\n\
             +++ a.psc\n\
             @@ -1,3 +1,3 @@\n\
             \x20one\n\
             -two \n\
             +two\n\
             \x20three\n"
        );
    }

    #[test]
    fn pure_insertion_reports_zero_old_count() {
        let diff = unified_diff("a.psc", "one\ntwo\n", "one\nnew\ntwo\n");

        assert_eq!(
            diff,
            "--- a.psc\n\
             +++ a.psc\n\
             @@ -1,2 +1,3 @@\n\
             \x20one\n\
             +new\n\
             \x20two\n"
        );
    }

    #[test]
    fn pure_deletion_reports_zero_new_count() {
        let diff = unified_diff("a.psc", "one\ntwo\nthree\n", "one\nthree\n");

        assert_eq!(
            diff,
            "--- a.psc\n\
             +++ a.psc\n\
             @@ -1,3 +1,2 @@\n\
             \x20one\n\
             -two\n\
             \x20three\n"
        );
    }

    #[test]
    fn distant_changes_split_into_separate_hunks() {
        let old_lines: Vec<String> = (1..=20).map(|n| n.to_string()).collect();
        let mut new_lines = old_lines.clone();
        new_lines[0] = "changed-start".to_string();
        new_lines[19] = "changed-end".to_string();
        let old = format!("{}\n", old_lines.join("\n"));
        let new = format!("{}\n", new_lines.join("\n"));

        let diff = unified_diff("a.psc", &old, &new);
        let hunk_count = diff.matches("@@ ").count();

        assert_eq!(hunk_count, 2);
    }

    #[test]
    fn nearby_changes_merge_into_one_hunk() {
        let old = "1\n2\n3\n4\n5\n6\n7\n";
        let new = "changed\n2\n3\n4\n5\n6\nalso-changed\n";

        let diff = unified_diff("a.psc", old, new);
        let hunk_count = diff.matches("@@ ").count();

        assert_eq!(hunk_count, 1);
    }
}
