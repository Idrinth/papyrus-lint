//! Black-box tests for the crate-level published rule catalog and
//! filtered-repair coverage of every advertised fixable rule.

use papyrus_lints::{
    repair_filtered, tags::tags_for, Config, NamedArguments, FIXABLE_RULE_IDS, KNOWN_RULE_IDS,
};
use std::collections::HashSet;

#[test]
fn published_rule_id_lists_are_unique_and_fixable_rules_are_known() {
    let known: HashSet<_> = KNOWN_RULE_IDS.iter().copied().collect();
    let fixable: HashSet<_> = FIXABLE_RULE_IDS.iter().copied().collect();

    assert_eq!(known.len(), KNOWN_RULE_IDS.len(), "duplicate known rule id");
    assert_eq!(
        fixable.len(),
        FIXABLE_RULE_IDS.len(),
        "duplicate fixable rule id"
    );
    assert!(
        fixable.is_subset(&known),
        "every fixable rule must also be advertised as known"
    );
}

#[test]
fn every_known_rule_id_resolves_to_published_tags() {
    for rule in KNOWN_RULE_IDS {
        let tags = tags_for(rule).unwrap_or_else(|| panic!("{rule:?} has no published tags"));
        assert!(!tags.kinds.is_empty());
        assert_eq!(tags.auto_fixable(), FIXABLE_RULE_IDS.contains(rule));
    }
}

#[test]
fn every_published_fixable_rule_works_through_the_filtered_public_api() {
    let mut property_config = Config::default();
    property_config.rules.property_sorting = true;

    let named_arguments_config = Config {
        named_arguments: NamedArguments::Always,
        ..Config::default()
    };

    let mut unused_disable_config = Config::default();
    unused_disable_config.rules.unused_disable = true;

    let default_config = Config::default();
    let cases = [
        (
            "identifier-casing",
            "ScriptName Example\n\nFunction Run(Int left)\nEndFunction\n",
            "ScriptName Example\n\nFunction Run(Int Left)\nEndFunction\n",
            &default_config,
        ),
        (
            "slow-functions",
            "Value.SetValueInt(3)\n",
            "Value.SetValue(3 As Float)\n",
            &default_config,
        ),
        (
            "semicolon",
            "Int Value = 1;\n",
            "Int Value = 1\n",
            &default_config,
        ),
        (
            "indentation",
            "Function Run()\n  Call()\nEndFunction\n",
            "Function Run()\n\tCall()\nEndFunction\n",
            &default_config,
        ),
        (
            "property-sorting",
            "ScriptName Example\n\nInt Property Zulu Auto\nActor Property Alpha Auto\n",
            "ScriptName Example\nActor Property Alpha Auto\n\nInt Property Zulu Auto\n\n",
            &property_config,
        ),
        (
            "comma-spacing",
            "Call(1,2)\n",
            "Call(1, 2)\n",
            &default_config,
        ),
        (
            "chain-whitespace",
            "Value . Call()\n",
            "Value.Call()\n",
            &default_config,
        ),
        (
            "exclamation-spacing",
            "If !Ready\nEndIf\n",
            "If ! Ready\nEndIf\n",
            &default_config,
        ),
        (
            "operator-spacing",
            "If Left==Right\nEndIf\n",
            "If Left == Right\nEndIf\n",
            &default_config,
        ),
        (
            "assignment-operator-spacing",
            "Left=Right\n",
            "Left = Right\n",
            &default_config,
        ),
        (
            "type-casing",
            "ScriptName myScript\n",
            "ScriptName MyScript\n",
            &default_config,
        ),
        (
            "trailing-whitespace",
            "Call()  \n",
            "Call()\n",
            &default_config,
        ),
        (
            "global-variable-increment",
            "ScriptName Example\n\nFunction Run(GlobalVariable gv, Float x)\n    gv.SetValue(gv.GetValue() + x)\nEndFunction\n",
            "ScriptName Example\n\nFunction Run(GlobalVariable gv, Float x)\n    gv.Mod(x)\nEndFunction\n",
            &default_config,
        ),
        (
            "named-arguments",
            "ScriptName Example\n\nFunction Greet(String name)\nEndFunction\n\nFunction Test()\n    Greet(\"hi\")\nEndFunction\n",
            "ScriptName Example\n\nFunction Greet(String name)\nEndFunction\n\nFunction Test()\n    Greet(name = \"hi\")\nEndFunction\n",
            &named_arguments_config,
        ),
        (
            "unnecessary-function",
            "ScriptName Example\n\nFunction A()\n    B()\nEndFunction\n\nFunction Caller()\n    A()\nEndFunction\n",
            "ScriptName Example\n\nFunction A()\n    B()\nEndFunction\n\nFunction Caller()\n    B()\nEndFunction\n",
            &default_config,
        ),
        (
            // Unlike every other fixable rule, "unused-import" can only ever
            // be resolved through a project-wide external resolver (see
            // `repair_with_external_arguments`), so the plain, resolver-less
            // public API this test exercises is always a no-op for it.
            "unused-import",
            "ScriptName Example\n\nImport Utility\n",
            "ScriptName Example\n\nImport Utility\n",
            &default_config,
        ),
        (
            "formid-hex-notation",
            "ScriptName Example\n\nFunction Test(Actor akActor)\n    If akActor.GetFormID() == 76935\n    EndIf\nEndFunction\n",
            "ScriptName Example\n\nFunction Test(Actor akActor)\n    If akActor.GetFormID() == 0x12C87\n    EndIf\nEndFunction\n",
            &default_config,
        ),
        (
            "get-form-from-file-skyrim-esm",
            "ScriptName Example\n\nFunction Test()\n    Form theForm = Game.GetFormFromFile(0x12345, \"Skyrim.esm\")\nEndFunction\n",
            "ScriptName Example\n\nFunction Test()\n    Form theForm = Game.GetForm(0x12345)\nEndFunction\n",
            &default_config,
        ),
        (
            "final-newline",
            "ScriptName Example",
            "ScriptName Example\n",
            &default_config,
        ),
        (
            "get-form-from-file-load-index",
            "ScriptName Example\n\nFunction Test()\n    Form theForm = Game.GetFormFromFile(0x01012345, \"Update.esm\")\nEndFunction\n",
            "ScriptName Example\n\nFunction Test()\n    Form theForm = Game.GetFormFromFile(0x12345, \"Update.esm\")\nEndFunction\n",
            &default_config,
        ),
        (
            "useless-downcast",
            "ScriptName Example\n\nFunction Test()\n    Int i = 1\n    Int j = i as Int\nEndFunction\n",
            "ScriptName Example\n\nFunction Test()\n    Int i = 1\n    Int j = i\nEndFunction\n",
            &default_config,
        ),
        (
            "self-assignment",
            "ScriptName Example\n\nFunction Test()\n    Int a = 1\n    a = a\n    a = 2\nEndFunction\n",
            "ScriptName Example\n\nFunction Test()\n    Int a = 1\n    a = 2\nEndFunction\n",
            &default_config,
        ),
        (
            "unused-disable",
            "ScriptName Example\nFunction Test()\n    Int x = 1 ; @disable mystery-rule\nEndFunction\n",
            "ScriptName Example\nFunction Test()\n    Int x = 1\nEndFunction\n",
            &unused_disable_config,
        ),
        (
            // Like "unused-import", "argument-naming" only resolves through
            // `repair_with_external_arguments`, so the resolver-less public
            // API this test exercises is a no-op.
            "argument-naming",
            "ScriptName Example Extends ParentScript\n\nFunction DoThing(ObjectReference akRef)\nEndFunction\n",
            "ScriptName Example Extends ParentScript\n\nFunction DoThing(ObjectReference akRef)\nEndFunction\n",
            &default_config,
        ),
        (
            "boolean-simplification",
            "ScriptName Example\n\nFunction Test(Bool ready)\n    If ready == true\n    EndIf\n    If ready == false\n    EndIf\nEndFunction\n",
            "ScriptName Example\n\nFunction Test(Bool ready)\n    If ready\n    EndIf\n    If ! ready\n    EndIf\nEndFunction\n",
            &default_config,
        ),
        (
            "redundant-bool-assignment",
            "ScriptName Example\n\nFunction Test(Int x)\n    Bool bResult = false\n    If x > 5\n        bResult = true\n    Else\n        bResult = false\n    EndIf\nEndFunction\n",
            "ScriptName Example\n\nFunction Test(Int x)\n    Bool bResult = false\n    bResult = x > 5\nEndFunction\n",
            &default_config,
        ),
        (
            "duplicate-import",
            "ScriptName Example\n\nImport Utility\nImport Utility\n",
            "ScriptName Example\n\nImport Utility\n",
            &default_config,
        ),
    ];

    let exercised: HashSet<_> = cases.iter().map(|(rule, ..)| *rule).collect();
    let published: HashSet<_> = FIXABLE_RULE_IDS.iter().copied().collect();
    assert_eq!(
        exercised, published,
        "add a filtered-repair case whenever the public fixable list changes"
    );

    for (rule, source, expected, config) in cases {
        assert_eq!(
            repair_filtered(source, config, Some(rule)),
            expected,
            "filtered repair failed for {rule}"
        );
    }
}
