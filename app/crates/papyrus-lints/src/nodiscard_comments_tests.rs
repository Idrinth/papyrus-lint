use super::*;

#[test]
fn recognizes_nodiscard_among_other_annotations() {
    assert!(crate::unused_nodiscard::line_has_nodiscard(
        "Function Old() ; @deprecated Use New() @nodiscard @public"
    ));
}

#[test]
fn adds_a_bare_nodiscard_comment_to_a_header_with_no_comment() {
    let updated =
        add_nodiscard_directive("Int Function RegisterFoo()\n    Return 1\nEndFunction\n", 1);
    assert_eq!(
        updated,
        "Int Function RegisterFoo() ; @nodiscard\n    Return 1\nEndFunction\n"
    );
}

#[test]
fn extends_an_existing_trailing_comment() {
    let updated = add_nodiscard_directive("Int Function RegisterFoo() ; keep this\n", 1);
    assert_eq!(
        updated,
        "Int Function RegisterFoo() ; keep this @nodiscard\n"
    );
}

#[test]
fn leaves_the_line_untouched_if_already_flagged() {
    let source = "Int Function RegisterFoo() ; @nodiscard\n";
    assert_eq!(add_nodiscard_directive(source, 1), source);
}

#[test]
fn leaves_the_line_untouched_if_flagged_on_the_line_above() {
    let source = "; @nodiscard\nInt Function RegisterFoo()\n";
    assert_eq!(add_nodiscard_directive(source, 2), source);
}

#[test]
fn does_not_match_nodiscardable_as_already_flagged() {
    let updated = add_nodiscard_directive("Int Function RegisterFoo() ; @nodiscardable\n", 1);
    assert_eq!(
        updated,
        "Int Function RegisterFoo() ; @nodiscardable @nodiscard\n"
    );
}

#[test]
fn leaves_source_untouched_for_an_out_of_range_line() {
    let source = "Int Function RegisterFoo()\n";
    assert_eq!(add_nodiscard_directive(source, 5), source);
    assert_eq!(add_nodiscard_directive(source, 0), source);
}

#[test]
fn preserves_a_trailing_carriage_return() {
    let updated = add_nodiscard_directive("Int Function RegisterFoo()\r\n", 1);
    assert_eq!(updated, "Int Function RegisterFoo() ; @nodiscard\r\n");
}
