//! Tests that each `config.rules` flag gates only its own lint through
//! [`crate::lint`]. Rules that can never fire without an
//! [`crate::ExternalSignatures`] resolver live in [`super::rule_flags_external`].

use super::super::*;
use super::support::config_with;

/// `lint_with_external_arguments` gates each ruleset behind its own `if
/// rules.<field>` check (see [`config::Rules`]). This walks every
/// ruleset except `function_override`, one at a time, confirming that:
/// (a) its own flag being on lets it fire on a source crafted to
/// trigger it, and (b) flipping only that flag off suppresses that
/// rule's diagnostics. `function_override` can never fire through bare
/// `lint()` (it always needs an `external` resolver for the script's
/// `Extends` chain), so its gate is exercised separately in
/// [`super::rule_flags_external`] — a gate accidentally wired to the
/// wrong `Rules` field, or hardcoded to always run, would fail here
/// even though it wouldn't fail any other existing test.
#[test]
fn each_rule_flag_gates_only_its_own_lint() {
    let many_states_source: String = {
        let mut source = "ScriptName Example\n\n".to_string();
        for index in 0..128 {
            source.push_str(&format!("State State{index}\nEndState\n"));
        }
        source
    };
    let cases: Vec<(&str, &str, Config, Config)> = vec![
            (
                "ScriptName Example  \n",
                trailing_whitespace::RULE,
                Config::default(),
                config_with(|c| c.rules.trailing_whitespace = false),
            ),
            (
                "Function Add(Int left,Int right)\n  Use(Add(1,2),3)\nEndFunction\n",
                comma_spacing::RULE,
                Config::default(),
                config_with(|c| c.rules.comma_spacing = false),
            ),
            (
                "ScriptName Example\n\nFunction DoThing()\n    Game.GetPlayer()\nEndFunction\n",
                forbidden_functions::RULE,
                Config::default(),
                config_with(|c| c.rules.forbidden_functions = false),
            ),
            (
                "ScriptName Example\n\nFunction DoThing(GlobalVariable akGlobal)\n    akGlobal.GetValueInt()\nEndFunction\n",
                slow_functions::RULE,
                Config::default(),
                config_with(|c| c.rules.slow_functions = false),
            ),
            (
                "Function Test()\n  GetValue()\nEndFunction\n",
                unused_getter::RULE,
                Config::default(),
                config_with(|c| c.rules.unused_getter = false),
            ),
            (
                "ScriptName Example\n\nInt Function RegisterFoo() ; @nodiscard\n    Return 1\nEndFunction\n\nFunction Test()\n    RegisterFoo()\nEndFunction\n",
                unused_nodiscard::RULE,
                Config::default(),
                config_with(|c| c.rules.unused_nodiscard = false),
            ),
            (
                "ScriptName Example\n\nInt Property MyValue = 1 Auto\n\nFunction DoThing()\nEndFunction\n",
                unused_property::RULE,
                Config::default(),
                config_with(|c| c.rules.unused_property = false),
            ),
            (
                // Default config forbids trailing semicolons, so a semicolon here violates it.
                "ScriptName Example\n\nInt value = 1;\n",
                semicolon::RULE,
                Config::default(),
                config_with(|c| c.rules.semicolon = false),
            ),
            (
                "ScriptName Example\n\nFunction Test()\n    Int x = 1.5\nEndFunction\n",
                float_int_conversion::RULE,
                Config::default(),
                config_with(|c| c.rules.float_int_conversion = false),
            ),
            (
                "ScriptName Example\n\nFunction Test()\n    Float f = 1 / 2\nEndFunction\n",
                int_division_to_float::RULE,
                Config::default(),
                config_with(|c| c.rules.int_division_to_float = false),
            ),
            (
                "ScriptName Example\n\nFunction Test(Int count)\n    If count\n    EndIf\nEndFunction\n",
                strict_boolean::RULE,
                Config::default(),
                config_with(|c| c.rules.strict_boolean = false),
            ),
            (
                "ScriptName Example\n\nFunction Greet(String name)\nEndFunction\n\nFunction Test()\n    Greet(1)\nEndFunction\n",
                argument_types::RULE,
                Config::default(),
                config_with(|c| c.rules.argument_types = false),
            ),
            (
                "ScriptName Example\n\nFunction Test(Int a)\n    If a == 1.0\n    EndIf\nEndFunction\n",
                numeric_comparison::RULE,
                Config::default(),
                config_with(|c| c.rules.numeric_comparison = false),
            ),
            (
                // Default config expects tab indentation; this uses spaces instead.
                "Function Run()\n  If ready\nDoThing()\nEndIf\nEndFunction\n",
                indentation::RULE,
                Config::default(),
                config_with(|c| c.rules.indentation = false),
            ),
            (
                "ScriptName Example\n\nFunction Test()\n    Int i = 1\nEndFunction\n",
                cyclomatic_complexity::RULE,
                // The default warning threshold (10) wouldn't flag this trivial
                // function, so lower it — independent of the `rules` flag under test.
                config_with(|c| c.cyclomatic_complexity_warning = 0),
                config_with(|c| {
                    c.cyclomatic_complexity_warning = 0;
                    c.rules.cyclomatic_complexity = false;
                }),
            ),
            (
                "ScriptName Example\n\nFunction Test()\n    Return\n    Int i = 1\nEndFunction\n",
                unreachable_statement::RULE,
                Config::default(),
                config_with(|c| c.rules.unreachable_statement = false),
            ),
            (
                "ScriptName Example\n\nInt Function Test()\n    Return \"hi\"\nEndFunction\n",
                return_types::RULE,
                Config::default(),
                config_with(|c| c.rules.return_types = false),
            ),
            (
                "ScriptName Example\n\nFunction Test()\n    Int i = 1\nEndFunction\n",
                unused_local_variable::RULE,
                Config::default(),
                config_with(|c| c.rules.unused_local_variable = false),
            ),
            (
                "ScriptName Example\n\nFunction Test()\n    Armor a = None\n    a.GetName()\nEndFunction\n",
                none_form_usage::RULE,
                Config::default(),
                config_with(|c| c.rules.none_form_usage = false),
            ),
            (
                "ScriptName Example\n\nFunction Test()\n    Int i\n    Debug.Trace(i as String)\nEndFunction\n",
                variable_used_before_assignment::RULE,
                Config::default(),
                config_with(|c| c.rules.variable_used_before_assignment = false),
            ),
            (
                "ScriptName Example\n\nInt Property MyValue Auto\n\nFunction Test()\n    Int MyValue = 1\n    Debug.Trace(MyValue)\nEndFunction\n",
                local_variable_shadowing::RULE,
                Config::default(),
                config_with(|c| c.rules.local_variable_shadowing = false),
            ),
            (
                "SomeProperty . DoThing()\n",
                chain_whitespace::RULE,
                Config::default(),
                config_with(|c| c.rules.chain_whitespace = false),
            ),
            (
                "If !bReady\nEndIf\n",
                exclamation_spacing::RULE,
                Config::default(),
                config_with(|c| c.rules.exclamation_spacing = false),
            ),
            (
                "If a==b\nEndIf\n",
                operator_spacing::RULE,
                Config::default(),
                config_with(|c| c.rules.operator_spacing = false),
            ),
            (
                "a=b\n",
                assignment_operator_spacing::RULE,
                Config::default(),
                config_with(|c| c.rules.assignment_operator_spacing = false),
            ),
            (
                "ScriptName Example\n\nInt Property bad_name = 1 Auto\n",
                identifier_casing::RULE,
                Config::default(),
                config_with(|c| c.rules.identifier_casing = false),
            ),
            (
                "ScriptName myExample\n",
                type_casing::RULE,
                Config::default(),
                config_with(|c| c.rules.type_casing = false),
            ),
            (
                "ScriptName Example\n\nFunction Greet(String name)\nEndFunction\n\nFunction Test()\n    Greet(\"hi\")\nEndFunction\n",
                named_arguments::RULE,
                config_with(|c| c.named_arguments = named_arguments::NamedArguments::Always),
                config_with(|c| {
                    c.named_arguments = named_arguments::NamedArguments::Always;
                    c.rules.named_arguments = false;
                }),
            ),
            (
                "ScriptName Example\n\nInt Property Zulu = 1 Auto\nActor Property Alpha Auto\n",
                property_sorting::RULE,
                config_with(|c| c.rules.property_sorting = true),
                config_with(|c| c.rules.property_sorting = false),
            ),
            (
                "ScriptName Example\n\nInt Function Test()\n    Int i = 1\nEndFunction\n",
                explicit_return::RULE,
                Config::default(),
                config_with(|c| c.rules.explicit_return = false),
            ),
            (
                "ScriptName Example\n\nFunction Test(Armor akArmor)\n    akArmor.GetName()\nEndFunction\n",
                unchecked_form_parameter::RULE,
                config_with(|c| c.rules.unchecked_form_parameter = true),
                config_with(|c| c.rules.unchecked_form_parameter = false),
            ),
            (
                "ScriptName Example\n\nFunction Test()\n    Actor[] act = new Actor[3]\n    act[2].Kill()\nEndFunction\n",
                unchecked_array_element::RULE,
                config_with(|c| c.rules.unchecked_array_element = true),
                config_with(|c| c.rules.unchecked_array_element = false),
            ),
            (
                "ScriptName Example\n\nFunction Test(ObjectReference akRef)\n    (akRef as Actor).GetActorValue(\"Health\")\nEndFunction\n",
                unchecked_cast::RULE,
                Config::default(),
                config_with(|c| c.rules.unchecked_cast = false),
            ),
            (
                "ScriptName Example\n\nFunction Test(Actor akActor)\n    Foo(akActor as Actor)\nEndFunction\n",
                useless_downcast::RULE,
                Config::default(),
                config_with(|c| c.rules.useless_downcast = false),
            ),
            (
                "ScriptName Example\n\nFunction Test(Int a)\n    Int b = a / 0\nEndFunction\n",
                division_by_zero::RULE,
                Config::default(),
                config_with(|c| c.rules.division_by_zero = false),
            ),
            (
                "ScriptName Example\n\nFunction Test()\n    While true\n    EndWhile\nEndFunction\n",
                empty_body::RULE,
                Config::default(),
                config_with(|c| c.rules.empty_body = false),
            ),
            (
                "ScriptName Example\n\nFunction Test()\n    Utility.Wait(0.01)\nEndFunction\n",
                short_wait_interval::RULE,
                Config::default(),
                config_with(|c| c.rules.short_wait_interval = false),
            ),
            (
                "ScriptName Example\n\nFunction Test(Actor akActor)\n    If akActor.GetFormID() == 76935\n    EndIf\nEndFunction\n",
                formid_hex_notation::RULE,
                Config::default(),
                config_with(|c| c.rules.formid_hex_notation = false),
            ),
            (
                "ScriptName Example\n\nFunction Greet(String name)\nEndFunction\n\nState Loud\n    Function Greet(Int name)\n    EndFunction\nEndState\n",
                state_function_signature::RULE,
                Config::default(),
                config_with(|c| c.rules.state_function_signature = false),
            ),
            (
                "ScriptName Example\n\nFunction Test()\n    GoToState(\"Missing\")\nEndFunction\n",
                goto_state::RULE,
                Config::default(),
                config_with(|c| c.rules.goto_state = false),
            ),
            (
                "ScriptName Example\n\nFunction Test()\n    If GetState() == \"Missing\"\n    EndIf\nEndFunction\n",
                get_state_comparison::RULE,
                Config::default(),
                config_with(|c| c.rules.get_state_comparison = false),
            ),
            (
                many_states_source.as_str(),
                state_count::TOO_MANY_STATES_RULE,
                Config::default(),
                config_with(|c| c.rules.too_many_states = false),
            ),
            (
                "ScriptName Example\n\nFunction Test()\n    DoThing(42)\nEndFunction\n",
                magic_numbers::RULE,
                config_with(|c| c.rules.magic_numbers = true),
                Config::default(),
            ),
            (
                "ScriptName Example\n\nAuto State Idle\nEndState\n\nAuto State Active\nEndState\n",
                state_count::MULTIPLE_AUTO_STATES_RULE,
                Config::default(),
                config_with(|c| c.rules.multiple_auto_states = false),
            ),
            (
                "ScriptName Example\n\nFunction Test(GlobalVariable gv)\n    If gv.GetValue() == 1.0\n    ElseIf gv.GetValue() == 2.0\n    EndIf\nEndFunction\n",
                repeated_getvalue::RULE,
                config_with(|c| c.rules.repeated_getvalue = true),
                Config::default(),
            ),
            (
                "ScriptName Example\n\nFunction Test(GlobalVariable gv)\n    If gv.GetValue() == 2.0\n        gv.SetValue(2.0)\n    EndIf\nEndFunction\n",
                global_variable_setvalue::RULE,
                config_with(|c| c.rules.global_variable_setvalue = true),
                Config::default(),
            ),
            (
                "ScriptName Example\n\nFunction Test(GlobalVariable gv, Float x)\n    gv.SetValue(gv.GetValue() + x)\nEndFunction\n",
                global_variable_increment::RULE,
                Config::default(),
                config_with(|c| c.rules.global_variable_increment = false),
            ),
            (
                "ScriptName Example\n\nFunction Test(GlobalVariable gv, Int a)\n    While a > 0\n        gv.SetValue(a)\n        a -= 1\n    EndWhile\nEndFunction\n",
                setvalue_in_loop::RULE,
                Config::default(),
                config_with(|c| c.rules.setvalue_in_loop = false),
            ),
            (
                "ScriptName Example\n\nFunction Test()\n    Int n = 5\n    While n < 10\n        Debug.Trace(\"y\")\n    EndWhile\nEndFunction\n",
                invariant_loop_condition::RULE,
                Config::default(),
                config_with(|c| c.rules.invariant_loop_condition = false),
            ),
            (
                "ScriptName Example\n\nInt Property Example Auto\n",
                script_name_collision::RULE,
                Config::default(),
                config_with(|c| c.rules.script_name_collision = false),
            ),
            (
                "ScriptName Example\n\nFunction Test()\n    Int[] a = new Int[3]\n    a[5] = 1\nEndFunction\n",
                array_bounds::RULE,
                Config::default(),
                config_with(|c| c.rules.array_bounds = false),
            ),
            (
                "ScriptName Example\n\nFunction Test()\n    Int[] a = new Int[200]\nEndFunction\n",
                array_size_range::RULE,
                Config::default(),
                config_with(|c| c.rules.array_size_range = false),
            ),
            (
                "ScriptName Example\n\nFunction Test()\n    Int[] a\n    a[0] = 1\nEndFunction\n",
                array_used_before_new::RULE,
                Config::default(),
                config_with(|c| c.rules.array_used_before_new = false),
            ),
            (
                "ScriptName Example\n\nFloat Property a = 0.1 AutoReadOnly\n\nFunction Test()\n    a = 0.2\nEndFunction\n",
                readonly_property_write::RULE,
                Config::default(),
                config_with(|c| c.rules.readonly_property_write = false),
            ),
            (
                "ScriptName Example\n\nInt Property Count Auto\n",
                default_property_value::RULE,
                config_with(|c| c.rules.default_property_value = true),
                Config::default(),
            ),
            (
                "ScriptName Example\n\nFunction Test()\n    Test()\nEndFunction\n",
                unguarded_self_recursion::RULE,
                Config::default(),
                config_with(|c| c.rules.unguarded_self_recursion = false),
            ),
            (
                "ScriptName Example\n\nFunction Test()\n    Int a = 10\n    a = a\nEndFunction\n",
                self_assignment::RULE,
                Config::default(),
                config_with(|c| c.rules.self_assignment = false),
            ),
            (
                "ScriptName Example\n\nFunction Test(Actor akActor, Form item)\n    Debug.Trace(akActor.RemoveItem(item, 1))\nEndFunction\n",
                debug_side_effects::RULE,
                Config::default(),
                config_with(|c| c.rules.debug_side_effects = false),
            ),
            (
                "ScriptName Example\n\nFunction A()\n    B()\nEndFunction\n",
                unnecessary_function::RULE,
                Config::default(),
                config_with(|c| c.rules.unnecessary_function = false),
            ),
            (
                "ScriptName Example\n\nFunction Test(Actor akActor)\n    akActor.GetActorValue(\"NotARealActorValue\")\nEndFunction\n",
                actor_value::RULE,
                config_with(|c| c.rules.unknown_actor_value = true),
                Config::default(),
            ),
            (
                "ScriptName Example\n\nFunction Test(Actor akActor, Outfit MyOutfit)\n    akActor.SetOutfit(MyOutfit)\n    akActor.SetOutfit(MyOutfit)\nEndFunction\n",
                repeated_setoutfit::RULE,
                Config::default(),
                config_with(|c| c.rules.repeated_setoutfit = false),
            ),
            (
                "ScriptName Example\n\nFunction Test()\nEndFunction\n",
                missing_doc_comment::RULE,
                config_with(|c| c.rules.missing_doc_comment = true),
                Config::default(),
            ),
            (
                "ScriptName Example\n\nFunction Test()\n    Int i = Utility.RandomInt(10, 5)\nEndFunction\n",
                invalid_random_range::RULE,
                Config::default(),
                config_with(|c| c.rules.invalid_random_range = false),
            ),
            (
                "ScriptName Example\n\nFunction Test(Float a, Float b)\n    If a == b\n    EndIf\nEndFunction\n",
                float_equality::RULE,
                config_with(|c| c.rules.float_equality = true),
                Config::default(),
            ),
            (
                "ScriptName Example\n\nFunction Start()\n    RegisterForUpdate(7.0)\nEndFunction\n",
                missing_update_handler::RULE,
                config_with(|c| c.rules.missing_update_handler = true),
                Config::default(),
            ),
            (
                "ScriptName Example\n\nEvent OnActivate()\nEndEvent\n",
                event_signature::RULE,
                config_with(|c| c.rules.event_signature_mismatch = true),
                Config::default(),
            ),
        ];

    for (source, rule, enabled_config, disabled_config) in cases {
        let baseline = lint(source, &enabled_config);
        assert!(
            baseline.iter().any(|d| d.rule == rule),
            "expected rule {rule:?} to fire on {source:?}, got {baseline:?}"
        );

        let with_rule_disabled = lint(source, &disabled_config);
        assert!(
            with_rule_disabled.iter().all(|d| d.rule != rule),
            "disabling {rule:?} should suppress its own diagnostics, got {with_rule_disabled:?}"
        );
    }
}
