use super::*;

fn check(source: &str) -> Vec<Diagnostic> {
    let ast = papyrus_parser::parse(source).ok();
    let tokens = papyrus_parser::tokenize(source).ok();
    super::check(
        source,
        ast.as_ref(),
        tokens.as_deref(),
        &crate::config::Config::default(),
        &mut crate::external_signatures::NoExternalSignatures,
    )
}

#[test]
fn flags_a_literal_index_past_the_declared_size() {
    let diagnostics = check(
            "ScriptName Example\n\nFunction Test()\n    Float[] a = new Float[3]\n    a[5] = 0.1\nEndFunction\n",
        );

    assert_eq!(diagnostics.len(), 1);
    assert_eq!(diagnostics[0].line, 5);
    assert_eq!(diagnostics[0].rule, RULE);
    assert!(diagnostics[0].message.starts_with("[warning]"));
    assert!(diagnostics[0].message.contains("'a'"));
    assert!(diagnostics[0].message.contains('5'));
    assert!(diagnostics[0].message.contains('3'));
}

#[test]
fn flags_a_negative_literal_index() {
    let diagnostics = check(
            "ScriptName Example\n\nFunction Test()\n    Int[] a = new Int[3]\n    a[-1] = 1\nEndFunction\n",
        );

    assert_eq!(diagnostics.len(), 1);
    assert_eq!(diagnostics[0].line, 5);
}

#[test]
fn flags_an_out_of_bounds_read() {
    let diagnostics = check(
            "ScriptName Example\n\nFunction Test()\n    Int[] a = new Int[2]\n    Int v = a[2]\nEndFunction\n",
        );

    assert_eq!(diagnostics.len(), 1);
    assert_eq!(diagnostics[0].line, 5);
}

#[test]
fn does_not_flag_an_index_within_bounds() {
    let diagnostics = check(
            "ScriptName Example\n\nFunction Test()\n    Float[] a = new Float[3]\n    a[0] = 0.1\n    a[2] = 0.2\nEndFunction\n",
        );

    assert!(diagnostics.is_empty());
}

#[test]
fn does_not_flag_an_index_at_the_last_valid_position() {
    let diagnostics = check(
            "ScriptName Example\n\nFunction Test()\n    Int[] a = new Int[1]\n    a[0] = 1\nEndFunction\n",
        );

    assert!(diagnostics.is_empty());
}

#[test]
fn does_not_flag_a_non_literal_index() {
    let diagnostics = check(
            "ScriptName Example\n\nFunction Test(Int i)\n    Int[] a = new Int[3]\n    a[i] = 1\nEndFunction\n",
        );

    assert!(diagnostics.is_empty());
}

#[test]
fn does_not_flag_an_array_whose_size_is_not_a_literal() {
    let diagnostics = check(
            "ScriptName Example\n\nFunction Test(Int n)\n    Int[] a = new Int[n]\n    a[50] = 1\nEndFunction\n",
        );

    assert!(diagnostics.is_empty());
}

#[test]
fn does_not_flag_an_unrelated_identifier() {
    let diagnostics =
        check("ScriptName Example\n\nFunction Test(Int[] a)\n    a[50] = 1\nEndFunction\n");

    assert!(diagnostics.is_empty());
}

#[test]
fn stops_tracking_a_variable_reassigned_to_something_else() {
    let diagnostics = check(
            "ScriptName Example\n\nFunction Test(Int[] other)\n    Int[] a = new Int[2]\n    a = other\n    a[50] = 1\nEndFunction\n",
        );

    assert!(diagnostics.is_empty());
}

#[test]
fn tracks_a_size_updated_by_a_later_reassignment() {
    let diagnostics = check(
            "ScriptName Example\n\nFunction Test()\n    Int[] a = new Int[2]\n    a = new Int[5]\n    a[4] = 1\n    a[5] = 1\nEndFunction\n",
        );

    assert_eq!(diagnostics.len(), 1);
    assert_eq!(diagnostics[0].line, 7);
}

#[test]
fn constant_folding_handles_literal_arithmetic_in_the_size_and_index() {
    let diagnostics = check(
            "ScriptName Example\n\nFunction Test()\n    Int[] a = new Int[1 + 2]\n    a[1 + 2] = 1\nEndFunction\n",
        );

    assert_eq!(diagnostics.len(), 1);
    assert_eq!(diagnostics[0].line, 5);
}

#[test]
fn constant_folding_handles_division_in_the_index() {
    let diagnostics = check(
            "ScriptName Example\n\nFunction Test()\n    Int[] a = new Int[3]\n    a[6 / 2] = 1\nEndFunction\n",
        );

    assert_eq!(diagnostics.len(), 1);
    assert_eq!(diagnostics[0].line, 5);
}

#[test]
fn does_not_flag_after_both_if_branches_agree_on_the_size() {
    let diagnostics = check(
            "ScriptName Example\n\nFunction Test(Bool flag)\n    Int[] a\n    If flag\n        a = new Int[3]\n    Else\n        a = new Int[3]\n    EndIf\n    a[2] = 1\nEndFunction\n",
        );

    assert!(diagnostics.is_empty());
}

#[test]
fn does_not_flag_after_if_branches_disagree_on_the_size() {
    let diagnostics = check(
            "ScriptName Example\n\nFunction Test(Bool flag)\n    Int[] a\n    If flag\n        a = new Int[3]\n    Else\n        a = new Int[5]\n    EndIf\n    a[4] = 1\nEndFunction\n",
        );

    assert!(diagnostics.is_empty());
}

#[test]
fn flags_use_after_while_loop_since_it_may_run_zero_times() {
    let diagnostics = check(
            "ScriptName Example\n\nFunction Test(Bool flag)\n    Int[] a = new Int[1]\n    While flag\n        a = new Int[5]\n        flag = false\n    EndWhile\n    a[4] = 1\nEndFunction\n",
        );

    assert_eq!(diagnostics.len(), 1);
    assert_eq!(diagnostics[0].line, 9);
}

#[test]
fn does_not_flag_a_new_array_at_the_engine_maximum() {
    let diagnostics = check(
            "ScriptName Example\n\nFunction Test()\n    Int[] a = new Int[128]\n    a[127] = 1\nEndFunction\n",
        );

    assert!(diagnostics.is_empty());
}

#[test]
fn does_not_flag_indexing_a_property_array_beyond_128() {
    // Editor-populated array Properties (and arrays returned by native
    // functions) aren't subject to the 128-element `new`/`Add()` cap, and
    // this lint has no way to know such a property's actual configured
    // size from source, so an index like this is deliberately left
    // unflagged rather than risk a false positive.
    let diagnostics = check(
            "ScriptName Example\n\nFloat[] Property a Auto\n\nFloat Function DoA()\n    Return a[128]\nEndFunction\n",
        );

    assert!(diagnostics.is_empty());
}

#[test]
fn checks_call_arguments_and_nested_indices() {
    let diagnostics = check(
            "ScriptName Example\n\nFunction Test()\n    Int[] a = new Int[2]\n    Int[] b = new Int[1]\n    Consume(a[5])\n    Int v = a[b[0]]\nEndFunction\n",
        );

    assert_eq!(diagnostics.len(), 1);
    assert_eq!(diagnostics[0].line, 6);
}

#[test]
fn checks_functions_declared_in_states_too() {
    let diagnostics = check(
            "ScriptName Example\n\nState Active\n    Function Test()\n        Int[] a = new Int[1]\n        a[5] = 1\n    EndFunction\nEndState\n",
        );

    assert_eq!(diagnostics.len(), 1);
    assert_eq!(diagnostics[0].line, 6);
}

#[test]
fn each_function_starts_fresh() {
    let diagnostics = check(
            "ScriptName Example\n\nFunction First()\n    Int[] a = new Int[5]\nEndFunction\n\nFunction Second()\n    Int[] a = new Int[1]\n    a[4] = 1\nEndFunction\n",
        );

    assert_eq!(diagnostics.len(), 1);
    assert_eq!(diagnostics[0].line, 9);
}

#[test]
fn does_not_crash_on_unparseable_source() {
    assert!(check("ScriptName Example\n\nFunction Test(\nEndFunction\n").is_empty());
}
