use super::*;

#[test]
fn extracts_multiple_function_annotations_from_one_comment() {
    let source = "ScriptName Foo\n\nInt Function Old() ; @deprecated Use New() @nodiscard @public\n    Return 1\nEndFunction\n";
    let functions = functions_of(source);
    let old = functions.get("old").expect("function should be extracted");
    assert!(old.nodiscard);
    assert!(old.deprecated);
}

fn functions_of(source: &str) -> HashMap<String, FunctionSignature> {
    let script = papyrus_parser::parse(source).expect("test source should parse");
    ScriptFunctions::from_script(&script, source).functions
}

fn has_side_effects(source: &str, function_name: &str) -> bool {
    functions_of(source)
        .get(&function_name.to_ascii_lowercase())
        .unwrap_or_else(|| panic!("function '{function_name}' should be in the function list"))
        .has_side_effects
}

#[test]
fn function_with_no_writes_or_calls_has_no_side_effects() {
    let source =
        "ScriptName Foo\n\nInt Function Bar(Int a)\n    Int b = a + 1\n    Return b\nEndFunction\n";
    assert!(!has_side_effects(source, "Bar"));
}

#[test]
fn writing_a_local_variable_is_not_a_side_effect() {
    let source = "ScriptName Foo\n\nFunction Bar()\n    Int a = 1\n    a = 2\nEndFunction\n";
    assert!(!has_side_effects(source, "Bar"));
}

#[test]
fn writing_a_bare_identifier_that_is_not_a_local_is_a_side_effect() {
    // `Count` isn't declared as a local anywhere in `Bar`, so it resolves to
    // a property or field (this script's own, or an inherited one) - which
    // we can't tell apart from here, but either way writing it is a side
    // effect.
    let source =
        "ScriptName Foo\n\nInt Property Count Auto\n\nFunction Bar()\n    Count = 1\nEndFunction\n";
    assert!(has_side_effects(source, "Bar"));
}

#[test]
fn writing_through_self_is_a_side_effect() {
    let source =
        "ScriptName Foo\n\nInt Property Count Auto\n\nFunction Bar()\n    Self.Count = 1\nEndFunction\n";
    assert!(has_side_effects(source, "Bar"));
}

#[test]
fn writing_a_property_on_another_object_is_a_side_effect() {
    let source =
        "ScriptName Foo\n\nFunction Bar(Foo akOther)\n    akOther.Count = 1\nEndFunction\n";
    assert!(has_side_effects(source, "Bar"));
}

#[test]
fn writing_an_array_index_is_not_treated_as_a_property_write() {
    let source =
        "ScriptName Foo\n\nInt[] Property Values Auto\n\nFunction Bar()\n    Int[] local = Values\n    local[0] = 1\nEndFunction\n";
    assert!(!has_side_effects(source, "Bar"));
}

#[test]
fn calling_a_same_script_function_with_side_effects_is_transitive() {
    let source = "ScriptName Foo\n\nInt Property Count Auto\n\nFunction Bar()\n    Count = 1\nEndFunction\n\nFunction Baz()\n    Bar()\nEndFunction\n";
    assert!(has_side_effects(source, "Bar"));
    assert!(has_side_effects(source, "Baz"));
}

#[test]
fn calling_a_same_script_function_through_self_is_transitive() {
    let source = "ScriptName Foo\n\nInt Property Count Auto\n\nFunction Bar()\n    Count = 1\nEndFunction\n\nFunction Baz()\n    Self.Bar()\nEndFunction\n";
    assert!(has_side_effects(source, "Baz"));
}

#[test]
fn calling_a_same_script_function_with_no_side_effects_is_not_transitive() {
    let source = "ScriptName Foo\n\nInt Function Bar(Int a)\n    Return a + 1\nEndFunction\n\nFunction Baz()\n    Bar(1)\nEndFunction\n";
    assert!(!has_side_effects(source, "Bar"));
    assert!(!has_side_effects(source, "Baz"));
}

#[test]
fn calling_an_unresolvable_function_alone_is_not_a_side_effect() {
    // `akOther.Bar()` calls through another object, which this script's own
    // function list can't see into, so it can't prove a side effect either
    // way.
    let source = "ScriptName Foo\n\nFunction Baz(Foo akOther)\n    akOther.Bar()\nEndFunction\n";
    assert!(!has_side_effects(source, "Baz"));
}

#[test]
fn mutual_recursion_propagates_side_effects_to_both_functions() {
    let source = "ScriptName Foo\n\nInt Property Count Auto\n\nFunction A()\n    Count = 1\n    B()\nEndFunction\n\nFunction B()\n    A()\nEndFunction\n";
    assert!(has_side_effects(source, "A"));
    assert!(has_side_effects(source, "B"));
}

#[test]
fn side_effect_nested_in_a_call_argument_is_still_found() {
    let source = "ScriptName Foo\n\nInt Property Count Auto\n\nFunction Bar()\n    Count = 1\nEndFunction\n\nInt Function Baz()\n    Return Abs(Bar())\nEndFunction\n";
    assert!(has_side_effects(source, "Baz"));
}

#[test]
fn side_effect_inside_an_if_branch_is_found() {
    let source = "ScriptName Foo\n\nInt Property Count Auto\n\nFunction Bar(Bool cond)\n    If cond\n        Count = 1\n    EndIf\nEndFunction\n";
    assert!(has_side_effects(source, "Bar"));
}

#[test]
fn side_effect_inside_a_while_loop_is_found() {
    let source = "ScriptName Foo\n\nInt Property Count Auto\n\nFunction Bar(Bool cond)\n    While cond\n        Count = 1\n    EndWhile\nEndFunction\n";
    assert!(has_side_effects(source, "Bar"));
}

fn nodiscard(source: &str, function_name: &str) -> bool {
    functions_of(source)
        .get(&function_name.to_ascii_lowercase())
        .unwrap_or_else(|| panic!("function '{function_name}' should be in the function list"))
        .nodiscard
}

#[test]
fn function_without_nodiscard_comment_is_not_marked() {
    let source = "ScriptName Foo\n\nInt Function Bar()\n    Return 1\nEndFunction\n";
    assert!(!nodiscard(source, "Bar"));
}

#[test]
fn trailing_nodiscard_comment_on_the_header_is_tracked() {
    let source =
        "ScriptName Foo\n\nInt Function RegisterFoo() ; @nodiscard\n    Return 1\nEndFunction\n";
    assert!(nodiscard(source, "RegisterFoo"));
}

#[test]
fn nodiscard_comment_on_the_line_above_the_header_is_tracked() {
    let source =
        "ScriptName Foo\n\n; @nodiscard\nInt Function RegisterFoo()\n    Return 1\nEndFunction\n";
    assert!(nodiscard(source, "RegisterFoo"));
}

#[test]
fn nodiscard_matching_is_case_insensitive() {
    let source =
        "ScriptName Foo\n\nInt Function RegisterFoo() ; @NoDiscard\n    Return 1\nEndFunction\n";
    assert!(nodiscard(source, "RegisterFoo"));
}

#[test]
fn nodiscardable_is_not_treated_as_nodiscard() {
    let source =
        "ScriptName Foo\n\nInt Function RegisterFoo() ; @nodiscardable\n    Return 1\nEndFunction\n";
    assert!(!nodiscard(source, "RegisterFoo"));
}

#[test]
fn nodiscard_inside_a_string_is_ignored() {
    let source =
        "ScriptName Foo\n\nInt Function RegisterFoo(String s = \"; @nodiscard\")\n    Return 1\nEndFunction\n";
    assert!(!nodiscard(source, "RegisterFoo"));
}

#[test]
fn nodiscard_on_one_function_does_not_mark_its_neighbor() {
    let source = "ScriptName Foo\n\nInt Function RegisterFoo() ; @nodiscard\n    Return 1\nEndFunction\n\nInt Function Other()\n    Return 2\nEndFunction\n";
    assert!(nodiscard(source, "RegisterFoo"));
    assert!(!nodiscard(source, "Other"));
}

#[test]
fn nodiscard_on_a_backslash_continued_header_is_tracked() {
    let source = "ScriptName Foo\n\nInt Function RegisterFoo( \\\n    Int a \\\n) ; @nodiscard\n    Return a\nEndFunction\n";
    assert!(nodiscard(source, "RegisterFoo"));
}

#[test]
fn nodiscard_inside_a_state_only_function_is_tracked() {
    let source = "ScriptName Foo\n\nState Active\n    Int Function RegisterFoo() ; @nodiscard\n        Return 1\n    EndFunction\nEndState\n";
    assert!(nodiscard(source, "RegisterFoo"));
}

#[test]
fn deprecated_directive_is_tracked_on_function_signatures() {
    let source = "ScriptName Foo\n\n; @deprecated\nFunction OldWay()\nEndFunction\n\nFunction CurrentWay() ; @deprecatedSoon\nEndFunction\n";
    let functions = functions_of(source);
    assert!(functions["oldway"].deprecated);
    assert!(!functions["currentway"].deprecated);
}
