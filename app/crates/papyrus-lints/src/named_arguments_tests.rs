use super::*;

#[test]
fn never_setting_flags_nothing() {
    let diagnostics = check(
            "ScriptName Example\n\nFunction Greet(String name)\nEndFunction\n\nFunction Test()\n    Greet(\"hi\")\nEndFunction\n",
            NamedArguments::Never,
        );

    assert!(diagnostics.is_empty());
}

#[test]
fn always_flags_a_positional_argument_to_a_local_function() {
    let diagnostics = check(
            "ScriptName Example\n\nFunction Greet(String name)\nEndFunction\n\nFunction Test()\n    Greet(\"hi\")\nEndFunction\n",
            NamedArguments::Always,
        );

    assert_eq!(diagnostics.len(), 1);
    assert_eq!(diagnostics[0].line, 7);
    assert_eq!(diagnostics[0].rule, RULE);
    assert!(diagnostics[0].message.starts_with("[warning]"));
    assert!(diagnostics[0].message.contains("Argument 1 to 'Greet'"));
    assert!(diagnostics[0].message.contains("name = ..."));
}

#[test]
fn always_does_not_flag_an_already_named_argument() {
    let diagnostics = check(
            "ScriptName Example\n\nFunction Greet(String name)\nEndFunction\n\nFunction Test()\n    Greet(name = \"hi\")\nEndFunction\n",
            NamedArguments::Always,
        );

    assert!(diagnostics.is_empty());
}

#[test]
fn always_flags_every_positional_argument_including_required_ones() {
    let diagnostics = check(
            "ScriptName Example\n\nFunction MyFunction(Int argA, Int argB = 0)\nEndFunction\n\nFunction Test()\n    MyFunction(1, 2)\nEndFunction\n",
            NamedArguments::Always,
        );

    assert_eq!(diagnostics.len(), 2);
    assert!(diagnostics.iter().any(|d| d.message.contains("argA")));
    assert!(diagnostics.iter().any(|d| d.message.contains("argB")));
}

#[test]
fn instead_of_defaults_only_flags_arguments_with_a_default_value() {
    let diagnostics = check(
            "ScriptName Example\n\nFunction MyFunction(Int argA, Int argB = 0)\nEndFunction\n\nFunction Test()\n    MyFunction(1, 2)\nEndFunction\n",
            NamedArguments::InsteadOfDefaults,
        );

    assert_eq!(diagnostics.len(), 1);
    assert!(diagnostics[0]
        .message
        .contains("Argument 2 to 'MyFunction'"));
    assert!(diagnostics[0].message.contains("argB = ..."));
}

#[test]
fn instead_of_defaults_accepts_a_named_argument_for_a_default_parameter() {
    let diagnostics = check(
            "ScriptName Example\n\nFunction MyFunction(Int argA, Int argB = 0)\nEndFunction\n\nFunction Test()\n    MyFunction(1, argB = 2)\nEndFunction\n",
            NamedArguments::InsteadOfDefaults,
        );

    assert!(diagnostics.is_empty());
}

#[test]
fn resolves_self_qualified_calls_the_same_way() {
    let diagnostics = check(
            "ScriptName Example\n\nFunction Greet(String name)\nEndFunction\n\nFunction Test()\n    self.Greet(\"hi\")\nEndFunction\n",
            NamedArguments::Always,
        );

    assert_eq!(diagnostics.len(), 1);
    assert!(diagnostics[0].message.contains("Greet"));
}

#[test]
fn does_not_flag_calls_to_functions_declared_on_other_scripts() {
    let diagnostics = check(
            "ScriptName Example\n\nFunction Test(Actor akActor)\n    akActor.MoveTo(1, 2, 3)\nEndFunction\n",
            NamedArguments::Always,
        );

    assert!(diagnostics.is_empty());
}

#[test]
fn does_not_flag_calls_beyond_the_declared_parameter_count() {
    let diagnostics = check(
            "ScriptName Example\n\nFunction Greet(String name)\nEndFunction\n\nFunction Test()\n    Greet(\"hi\", 1)\nEndFunction\n",
            NamedArguments::Always,
        );

    assert_eq!(diagnostics.len(), 1);
}

#[test]
fn skips_ambiguous_overrides_across_states() {
    let diagnostics = check(
            "ScriptName Example\n\nFunction Greet(String name)\nEndFunction\n\nState Loud\n    Function Greet(Int volume)\n    EndFunction\nEndState\n\nFunction Test()\n    Greet(\"hi\")\nEndFunction\n",
            NamedArguments::Always,
        );

    assert!(diagnostics.is_empty());
}

#[test]
fn checks_functions_declared_inside_a_state_too() {
    let diagnostics = check(
            "ScriptName Example\n\nState Active\n    Function Greet(String name)\n    EndFunction\n\n    Function Test()\n        Greet(\"hi\")\n    EndFunction\nEndState\n",
            NamedArguments::Always,
        );

    assert_eq!(diagnostics.len(), 1);
}

#[test]
fn flags_a_positional_argument_nested_inside_another_call() {
    let diagnostics = check(
            "ScriptName Example\n\nFunction Greet(String name)\nEndFunction\n\nFunction Test()\n    Debug.Trace(Greet(\"hi\"))\nEndFunction\n",
            NamedArguments::Always,
        );

    assert_eq!(diagnostics.len(), 1);
    assert!(diagnostics[0].message.contains("Greet"));
}

#[test]
fn does_not_crash_on_unparseable_source() {
    let diagnostics = check(
        "ScriptName Example\n\nFunction Test(\nEndFunction\n",
        NamedArguments::Always,
    );
    assert!(diagnostics.is_empty());
}

#[test]
fn flags_a_positional_argument_in_a_var_decl_initializer() {
    let diagnostics = check(
            "ScriptName Example\n\nFunction Greet(String name)\nEndFunction\n\nFunction Test()\n    String result = Greet(\"hi\")\nEndFunction\n",
            NamedArguments::Always,
        );

    assert_eq!(diagnostics.len(), 1);
    assert!(diagnostics[0].message.contains("Greet"));
}

#[test]
fn flags_positional_arguments_on_both_sides_of_an_assignment() {
    let diagnostics = check(
            "ScriptName Example\n\nFunction Greet(String name)\nEndFunction\n\nFunction Test(Int[] values)\n    values[Greet(\"index\") as Int] = 1\nEndFunction\n",
            NamedArguments::Always,
        );

    assert_eq!(diagnostics.len(), 1);
    assert!(diagnostics[0].message.contains("Greet"));
}

#[test]
fn flags_a_positional_argument_in_a_return_value() {
    let diagnostics = check(
            "ScriptName Example\n\nFunction Greet(String name)\nEndFunction\n\nString Function Test()\n    Return Greet(\"hi\")\nEndFunction\n",
            NamedArguments::Always,
        );

    assert_eq!(diagnostics.len(), 1);
    assert!(diagnostics[0].message.contains("Greet"));
}

#[test]
fn does_not_flag_a_bare_return_with_no_value() {
    let diagnostics = check(
            "ScriptName Example\n\nFunction Greet(String name)\nEndFunction\n\nFunction Test()\n    Return\nEndFunction\n",
            NamedArguments::Always,
        );

    assert!(diagnostics.is_empty());
}

#[test]
fn flags_positional_arguments_in_an_if_condition_every_branch_and_the_else_body() {
    let diagnostics = check(
            "ScriptName Example\n\nFunction Greet(String name)\nEndFunction\n\nFunction Test(Bool cond)\n    If Greet(\"cond\") == \"y\"\n        Greet(\"if-body\")\n    ElseIf cond\n        Greet(\"elseif-body\")\n    Else\n        Greet(\"else-body\")\n    EndIf\nEndFunction\n",
            NamedArguments::Always,
        );

    assert_eq!(diagnostics.len(), 4);
}

#[test]
fn flags_positional_arguments_in_a_while_condition_and_body() {
    let diagnostics = check(
            "ScriptName Example\n\nFunction Greet(String name)\nEndFunction\n\nFunction Test()\n    While Greet(\"cond\") == \"y\"\n        Greet(\"body\")\n    EndWhile\nEndFunction\n",
            NamedArguments::Always,
        );

    assert_eq!(diagnostics.len(), 2);
}

#[test]
fn flags_a_positional_argument_nested_in_a_binary_or_unary_expression() {
    let diagnostics = check(
            "ScriptName Example\n\nFunction Greet(String name)\nEndFunction\n\nFunction Test(Bool cond)\n    Bool a = cond && Greet(\"binary\") == \"y\"\n    Bool b = !cond || Greet(\"unary\") == \"y\"\nEndFunction\n",
            NamedArguments::Always,
        );

    assert_eq!(diagnostics.len(), 2);
}

#[test]
fn flags_a_positional_argument_nested_in_a_member_access() {
    let diagnostics = check(
            "ScriptName Example\n\nFunction Greet(String name)\nEndFunction\n\nFunction Test()\n    Greet(\"hi\").Length\nEndFunction\n",
            NamedArguments::Always,
        );

    assert_eq!(diagnostics.len(), 1);
}

#[test]
fn flags_a_positional_argument_nested_in_a_new_array_size_expression() {
    let diagnostics = check(
            "ScriptName Example\n\nFunction Greet(String name)\nEndFunction\n\nFunction GetSize(String label)\nEndFunction\n\nFunction Test()\n    Int[] values = new Int[GetSize(\"n\")]\nEndFunction\n",
            NamedArguments::Always,
        );

    assert_eq!(diagnostics.len(), 1);
    assert!(diagnostics[0].message.contains("GetSize"));
}

#[test]
fn flags_a_positional_argument_nested_inside_a_named_argument_value() {
    let diagnostics = check(
            "ScriptName Example\n\nFunction Greet(String name)\nEndFunction\n\nFunction Foo(String text)\nEndFunction\n\nFunction Test()\n    Foo(text = Greet(\"hi\"))\nEndFunction\n",
            NamedArguments::Always,
        );

    assert_eq!(diagnostics.len(), 1);
    assert!(diagnostics[0].message.contains("Greet"));
}

#[test]
fn never_setting_leaves_source_unchanged() {
    let source =
            "ScriptName Example\n\nFunction Greet(String name)\nEndFunction\n\nFunction Test()\n    Greet(\"hi\")\nEndFunction\n";

    assert_eq!(repair(source, NamedArguments::Never), source);
}

#[test]
fn always_names_a_positional_argument_to_a_local_function() {
    let source =
            "ScriptName Example\n\nFunction Greet(String name)\nEndFunction\n\nFunction Test()\n    Greet(\"hi\")\nEndFunction\n";

    let repaired = repair(source, NamedArguments::Always);

    assert_eq!(
            repaired,
            "ScriptName Example\n\nFunction Greet(String name)\nEndFunction\n\nFunction Test()\n    Greet(name = \"hi\")\nEndFunction\n"
        );
    assert!(check(&repaired, NamedArguments::Always).is_empty());
}

#[test]
fn always_leaves_an_already_named_argument_untouched() {
    let source =
            "ScriptName Example\n\nFunction Greet(String name)\nEndFunction\n\nFunction Test()\n    Greet(name = \"hi\")\nEndFunction\n";

    assert_eq!(repair(source, NamedArguments::Always), source);
}

#[test]
fn always_names_every_positional_argument_including_required_ones() {
    let source = "ScriptName Example\n\nFunction MyFunction(Int argA, Int argB = 0)\nEndFunction\n\nFunction Test()\n    MyFunction(1, 2)\nEndFunction\n";

    let repaired = repair(source, NamedArguments::Always);

    assert_eq!(
            repaired,
            "ScriptName Example\n\nFunction MyFunction(Int argA, Int argB = 0)\nEndFunction\n\nFunction Test()\n    MyFunction(argA = 1, argB = 2)\nEndFunction\n"
        );
}

#[test]
fn instead_of_defaults_only_names_arguments_with_a_default_value() {
    let source = "ScriptName Example\n\nFunction MyFunction(Int argA, Int argB = 0)\nEndFunction\n\nFunction Test()\n    MyFunction(1, 2)\nEndFunction\n";

    let repaired = repair(source, NamedArguments::InsteadOfDefaults);

    assert_eq!(
            repaired,
            "ScriptName Example\n\nFunction MyFunction(Int argA, Int argB = 0)\nEndFunction\n\nFunction Test()\n    MyFunction(1, argB = 2)\nEndFunction\n"
        );
}

#[test]
fn instead_of_defaults_leaves_a_named_default_argument_untouched() {
    let source = "ScriptName Example\n\nFunction MyFunction(Int argA, Int argB = 0)\nEndFunction\n\nFunction Test()\n    MyFunction(1, argB = 2)\nEndFunction\n";

    assert_eq!(repair(source, NamedArguments::InsteadOfDefaults), source);
}

#[test]
fn names_self_qualified_calls_the_same_way() {
    let source =
            "ScriptName Example\n\nFunction Greet(String name)\nEndFunction\n\nFunction Test()\n    self.Greet(\"hi\")\nEndFunction\n";

    let repaired = repair(source, NamedArguments::Always);

    assert_eq!(
            repaired,
            "ScriptName Example\n\nFunction Greet(String name)\nEndFunction\n\nFunction Test()\n    self.Greet(name = \"hi\")\nEndFunction\n"
        );
}

#[test]
fn does_not_touch_calls_to_functions_declared_on_other_scripts() {
    let source =
            "ScriptName Example\n\nFunction Test(Actor akActor)\n    akActor.MoveTo(1, 2, 3)\nEndFunction\n";

    assert_eq!(repair(source, NamedArguments::Always), source);
}

#[test]
fn does_not_touch_arguments_beyond_the_declared_parameter_count() {
    let source =
            "ScriptName Example\n\nFunction Greet(String name)\nEndFunction\n\nFunction Test()\n    Greet(\"hi\", 1)\nEndFunction\n";

    let repaired = repair(source, NamedArguments::Always);

    assert_eq!(
            repaired,
            "ScriptName Example\n\nFunction Greet(String name)\nEndFunction\n\nFunction Test()\n    Greet(name = \"hi\", 1)\nEndFunction\n"
        );
}

#[test]
fn repair_skips_ambiguous_overrides_across_states() {
    let source = "ScriptName Example\n\nFunction Greet(String name)\nEndFunction\n\nState Loud\n    Function Greet(Int volume)\n    EndFunction\nEndState\n\nFunction Test()\n    Greet(\"hi\")\nEndFunction\n";

    assert_eq!(repair(source, NamedArguments::Always), source);
}

#[test]
fn names_a_positional_argument_nested_inside_another_call() {
    let source =
            "ScriptName Example\n\nFunction Greet(String name)\nEndFunction\n\nFunction Test()\n    Debug.Trace(Greet(\"hi\"))\nEndFunction\n";

    let repaired = repair(source, NamedArguments::Always);

    assert_eq!(
            repaired,
            "ScriptName Example\n\nFunction Greet(String name)\nEndFunction\n\nFunction Test()\n    Debug.Trace(Greet(name = \"hi\"))\nEndFunction\n"
        );
}

#[test]
fn names_a_positional_argument_nested_inside_a_named_argument_value() {
    let source = "ScriptName Example\n\nFunction Greet(String name)\nEndFunction\n\nFunction Foo(String text)\nEndFunction\n\nFunction Test()\n    Foo(text = Greet(\"hi\"))\nEndFunction\n";

    let repaired = repair(source, NamedArguments::Always);

    assert_eq!(
            repaired,
            "ScriptName Example\n\nFunction Greet(String name)\nEndFunction\n\nFunction Foo(String text)\nEndFunction\n\nFunction Test()\n    Foo(text = Greet(name = \"hi\"))\nEndFunction\n"
        );
}

#[test]
fn repair_does_not_crash_on_unparseable_source() {
    let source = "ScriptName Example\n\nFunction Test(\nEndFunction\n";

    assert_eq!(repair(source, NamedArguments::Always), source);
}

#[test]
fn repair_is_idempotent() {
    let source = "ScriptName Example\n\nFunction MyFunction(Int argA, Int argB = 0)\nEndFunction\n\nFunction Test()\n    MyFunction(1, 2)\nEndFunction\n";

    let repaired = repair(source, NamedArguments::Always);

    assert_eq!(repair(&repaired, NamedArguments::Always), repaired);
}

#[test]
fn names_positional_arguments_spread_across_continuation_lines() {
    // A trailing `\` suppresses the newline token, Papyrus's own way to
    // let an argument list span multiple physical lines.
    let source = "ScriptName Example\n\nFunction MyFunction(Int argA, Int argB = 0)\nEndFunction\n\nFunction Test()\n    MyFunction(\\\n        1,\\\n        2\\\n    )\nEndFunction\n";

    let repaired = repair(source, NamedArguments::Always);

    assert_eq!(
            repaired,
            "ScriptName Example\n\nFunction MyFunction(Int argA, Int argB = 0)\nEndFunction\n\nFunction Test()\n    MyFunction(\\\n        argA = 1,\\\n        argB = 2\\\n    )\nEndFunction\n"
        );
}
