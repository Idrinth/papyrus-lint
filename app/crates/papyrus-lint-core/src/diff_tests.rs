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

#[test]
fn changes_with_exactly_twice_the_context_between_them_share_a_hunk() {
    let old = "change-one\n1\n2\n3\n4\n5\n6\nchange-two\n";
    let new = "updated-one\n1\n2\n3\n4\n5\n6\nupdated-two\n";

    let diff = unified_diff("a.psc", old, new);

    assert_eq!(diff.matches("@@ ").count(), 1);
    assert!(diff.contains("@@ -1,8 +1,8 @@"));
}

#[test]
fn one_more_than_twice_the_context_splits_changes_into_two_hunks() {
    let old = "change-one\n1\n2\n3\n4\n5\n6\n7\nchange-two\n";
    let new = "updated-one\n1\n2\n3\n4\n5\n6\n7\nupdated-two\n";

    let diff = unified_diff("a.psc", old, new);

    assert_eq!(diff.matches("@@ ").count(), 2);
    assert!(diff.contains("@@ -1,4 +1,4 @@"));
    assert!(diff.contains("@@ -6,4 +6,4 @@"));
    assert!(!diff.contains(" 4\n"));
}

#[test]
fn appending_to_a_file_reports_the_insertion_after_the_last_old_line() {
    let diff = unified_diff("a.psc", "one\ntwo\n", "one\ntwo\nthree\n");

    assert_eq!(
        diff,
        "--- a.psc\n\
         +++ a.psc\n\
         @@ -1,2 +1,3 @@\n\
         \x20one\n\
         \x20two\n\
         +three\n"
    );
}

#[test]
fn prepending_to_a_file_keeps_following_lines_as_context() {
    let diff = unified_diff("a.psc", "one\ntwo\n", "zero\none\ntwo\n");

    assert_eq!(
        diff,
        "--- a.psc\n\
         +++ a.psc\n\
         @@ -1,2 +1,3 @@\n\
         +zero\n\
         \x20one\n\
         \x20two\n"
    );
}

#[test]
fn insertion_into_empty_file_uses_zero_old_range() {
    let diff = unified_diff("new.psc", "", "ScriptName New\nFunction Run()\n");

    assert_eq!(
        diff,
        "--- new.psc\n\
         +++ new.psc\n\
         @@ -0,0 +1,2 @@\n\
         +ScriptName New\n\
         +Function Run()\n"
    );
}

#[test]
fn deletion_of_entire_file_uses_zero_new_range() {
    let diff = unified_diff("removed.psc", "ScriptName Old\n", "");

    assert_eq!(
        diff,
        "--- removed.psc\n\
         +++ removed.psc\n\
         @@ -1 +0,0 @@\n\
         -ScriptName Old\n"
    );
}

#[test]
fn preserves_carriage_returns_in_crlf_source() {
    let diff = unified_diff("windows.psc", "one\r\ntwo \r\n", "one\r\ntwo\r\n");

    assert!(diff.contains(" one\r\n"));
    assert!(diff.contains("-two \r\n"));
    assert!(diff.contains("+two\r\n"));
}

#[test]
fn trailing_newline_alone_is_not_treated_as_a_line_change() {
    assert_eq!(unified_diff("a.psc", "one\ntwo", "one\ntwo\n"), "");
}

#[test]
fn large_inputs_still_produce_a_minimal_diff() {
    // Regression coverage for the hand-rolled LCS implementation this
    // module used to have, which fell back to a whole-file replacement
    // above a fixed cell-count budget; `similar` has no such limit.
    let original = (0..4_472)
        .map(|line| format!("line-{line}"))
        .collect::<Vec<_>>()
        .join("\n");
    let mut updated_lines: Vec<String> = (0..4_472).map(|line| format!("line-{line}")).collect();
    updated_lines.insert(2_000, "inserted".to_string());
    let updated = updated_lines.join("\n");

    let diff = unified_diff(
        "large.psc",
        &format!("{original}\n"),
        &format!("{updated}\n"),
    );

    assert_eq!(diff.matches("@@ ").count(), 1);
    assert!(diff.contains("+inserted\n"));
    assert!(!diff.contains("-line-0\n"));
}
