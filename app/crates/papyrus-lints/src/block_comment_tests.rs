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
