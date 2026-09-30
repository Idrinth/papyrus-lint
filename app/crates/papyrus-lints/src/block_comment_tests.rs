use super::*;

#[test]
fn marks_both_delimiter_lines_and_everything_between() {
    let source =
        "Value = 50\n;/this is a test comment\nwill this compile, we will never know/;\nendevent\n";
    let protected = protected_lines(source);

    assert_eq!(protected, vec![false, false, true, true, false]);
}

#[test]
fn single_line_block_comment_is_protected() {
    let source = "Value = 50 ;/ note /; Value2 = 1\n";
    let protected = protected_lines(source);

    assert_eq!(protected, vec![false, true]);
}

#[test]
fn plain_line_comment_is_left_alone() {
    let source = "; just a note\nValue = 1\n";
    let protected = protected_lines(source);

    assert_eq!(protected, vec![false, false, false]);
}

#[test]
fn semicolons_inside_strings_do_not_open_a_comment() {
    let source = "Debug.Trace(\"a;/b\")\nValue = 1\n";
    let protected = protected_lines(source);

    assert_eq!(protected, vec![false, false, false]);
}

#[test]
fn a_block_comment_closes_on_its_first_slash_semicolon_even_inside_quotes() {
    // Matches `papyrus_parser`'s lexer: once a block comment is open, it
    // scans for a raw `/;` with no string-awareness, so quoted text
    // inside a comment offers no protection from it either.
    let source = ";/ note \"a/;b\" more\n";
    let protected = protected_lines(source);

    assert_eq!(protected, vec![false, true]);
}

#[test]
fn escaped_quotes_keep_comment_delimiters_inside_the_string() {
    let source = "Debug.Trace(\"escaped \\\" quote ;/ still text\")\nValue = 1\n";

    assert_eq!(protected_lines(source), vec![false, false, false]);
}

#[test]
fn scanner_can_close_and_reopen_a_block_comment_on_one_line() {
    let source = ";/ first /; Value = 1 ;/ second\nstill second /; Value = 2\n";

    assert_eq!(protected_lines(source), vec![false, true, true]);
}

#[test]
fn line_comment_prevents_a_later_block_comment_opener() {
    let source = "Value = 1 ; ordinary comment ;/ not a block\nValue = 2\n";

    assert_eq!(protected_lines(source), vec![false, false, false]);
}
