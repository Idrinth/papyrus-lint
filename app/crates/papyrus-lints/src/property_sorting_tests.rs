use super::*;

#[test]
fn ignores_already_sorted_and_positioned_properties() {
    let source =
            "ScriptName Example\n\nActor Property PlayerRef Auto\nInt Property Count = 1 Auto\n\nFunction DoThing()\nEndFunction\n";
    assert!(check(source).is_empty());
}

#[test]
fn flags_property_out_of_type_order() {
    let source =
        "ScriptName Example\n\nInt Property Count = 1 Auto\nActor Property PlayerRef Auto\n";
    let diagnostics = check(source);

    assert_eq!(diagnostics.len(), 1);
    assert_eq!(diagnostics[0].line, 4);
    assert_eq!(diagnostics[0].rule, RULE);
    assert!(diagnostics[0].message.contains("PlayerRef"));
    assert!(diagnostics[0].message.contains("Count"));
}

#[test]
fn flags_property_out_of_name_order_within_the_same_type() {
    let source = "ScriptName Example\n\nInt Property Zulu = 1 Auto\nInt Property Alpha = 1 Auto\n";
    let diagnostics = check(source);

    assert_eq!(diagnostics.len(), 1);
    assert_eq!(diagnostics[0].line, 4);
    assert!(diagnostics[0].message.contains("Alpha"));
    assert!(diagnostics[0].message.contains("Zulu"));
}

#[test]
fn flags_property_declared_after_a_function() {
    let source =
        "ScriptName Example\n\nFunction DoThing()\nEndFunction\n\nInt Property Count = 1 Auto\n";
    let diagnostics = check(source);

    assert_eq!(diagnostics.len(), 1);
    assert_eq!(diagnostics[0].line, 6);
    assert!(diagnostics[0].message.contains("Count"));
    assert!(diagnostics[0].message.contains("ScriptName"));
}

#[test]
fn flags_only_the_property_declared_after_other_members() {
    let source =
            "ScriptName Example\n\nInt Property Early = 1 Auto\n\nFunction DoThing()\nEndFunction\n\nInt Property Late = 1 Auto\n";
    let diagnostics = check(source);

    assert_eq!(diagnostics.len(), 1);
    assert!(diagnostics[0].message.contains("Late"));
}

#[test]
fn imports_do_not_interrupt_the_initial_property_block() {
    let source = "ScriptName Example\n\nImport Utility\nImport Debug\n\nActor Property PlayerRef Auto\nInt Property Count Auto\n";

    assert!(check(source).is_empty());
}

#[test]
fn flags_properties_after_each_kind_of_non_property_member() {
    let sources = [
        "ScriptName Example\nInt value\nInt Property Count Auto\n",
        "ScriptName Example\nFunction Run()\nEndFunction\nInt Property Count Auto\n",
        "ScriptName Example\nState Active\nEndState\nInt Property Count Auto\n",
    ];

    for source in sources {
        let diagnostics = check(source);
        assert_eq!(diagnostics.len(), 1, "source: {source}");
        assert!(diagnostics[0].message.contains("Count"));
    }
}

#[test]
fn sorting_is_case_insensitive_and_distinguishes_array_types() {
    let sorted = "ScriptName Example\nActor Property alpha Auto\nactor Property Zulu Auto\nActor[] Property Actors Auto\nInt Property Count Auto\n";
    assert!(check(sorted).is_empty());

    let unsorted = "ScriptName Example\nActor[] Property Actors Auto\nactor Property Zulu Auto\n";
    let diagnostics = check(unsorted);
    assert_eq!(diagnostics.len(), 1);
    assert_eq!(diagnostics[0].line, 3);
    assert!(diagnostics[0].message.contains("Actor[]"));
}

#[test]
fn reports_position_and_sorting_violations_for_the_same_property() {
    let source = "ScriptName Example\nFunction Run()\nEndFunction\nInt Property Zulu Auto\nActor Property Alpha Auto\n";
    let diagnostics = check(source);

    assert_eq!(diagnostics.len(), 3);
    assert_eq!(
        diagnostics
            .iter()
            .map(|diagnostic| diagnostic.line)
            .collect::<Vec<_>>(),
        vec![4, 5, 5]
    );
    assert_eq!(
        diagnostics
            .iter()
            .filter(|diagnostic| diagnostic.message.contains("out of order"))
            .count(),
        1
    );
}

#[test]
fn does_not_flag_a_single_property() {
    assert!(check("ScriptName Example\n\nInt Property Count = 1 Auto\n").is_empty());
}

#[test]
fn does_not_crash_on_unparseable_source() {
    assert!(check("ScriptName Example\n\nInt Property Count = \"unterminated\n").is_empty());
}

#[test]
fn repair_sorts_by_type_then_name() {
    let source = "ScriptName Example\n\nInt Property Zulu = 1 Auto\n\nActor Property PlayerRef Auto\n\nInt Property Alpha = 1 Auto\n";
    let repaired = repair(source);

    assert!(repaired.starts_with("ScriptName Example\n"));
    let player_ref = repaired.find("PlayerRef").unwrap();
    let alpha = repaired.find("Alpha").unwrap();
    let zulu = repaired.find("Zulu").unwrap();
    assert!(player_ref < alpha && alpha < zulu);
    assert!(check(&repaired).is_empty());
}

#[test]
fn repair_relocates_a_property_declared_after_a_function() {
    let source =
        "ScriptName Example\n\nFunction DoThing()\nEndFunction\n\nInt Property Count = 1 Auto\n";
    let repaired = repair(source);

    assert!(repaired.starts_with("ScriptName Example\n"));
    assert!(repaired.find("Property Count").unwrap() < repaired.find("Function DoThing").unwrap());
    assert!(repaired.contains("Function DoThing()\nEndFunction\n"));
    assert!(check(&repaired).is_empty());
}

#[test]
fn repair_moves_a_full_property_block_intact() {
    let source = "ScriptName Example\n\nInt Property Zulu = 1 Auto\n\nInt Property Alpha\n\tInt Function Get()\n\t\tReturn 1\n\tEndFunction\nEndProperty\n";
    let repaired = repair(source);

    assert!(repaired.contains(
        "Int Property Alpha\n\tInt Function Get()\n\t\tReturn 1\n\tEndFunction\nEndProperty\n"
    ));
    assert!(repaired.find("Alpha").unwrap() < repaired.find("Zulu").unwrap());
    assert!(check(&repaired).is_empty());
}

#[test]
fn repair_recognizes_case_insensitive_endproperty_with_a_comment() {
    let source = "ScriptName Example\nInt Property Zulu Auto\nActor Property Alpha\n\tActor Function Get()\n\t\tReturn None\n\tEndFunction\neNdPrOpErTy ; accessor ends here\nFunction Run()\nEndFunction\n";
    let repaired = repair(source);

    assert!(repaired.contains("eNdPrOpErTy ; accessor ends here\n\nInt Property Zulu Auto"));
    assert!(repaired.contains("Function Run()\nEndFunction\n"));
    assert!(check(&repaired).is_empty());
}

#[test]
fn repair_preserves_a_final_line_without_a_newline() {
    let source = "ScriptName Example\nInt Property Zulu Auto\nActor Property Alpha Auto";
    let repaired = repair(source);

    assert_eq!(
        repaired,
        "ScriptName Example\nActor Property Alpha Auto\n\nInt Property Zulu Auto\n"
    );
    assert!(check(&repaired).is_empty());
}

#[test]
fn repair_leaves_property_documentation_at_its_original_location() {
    let source = "ScriptName Example\nFunction Run()\nEndFunction\n; Count is used by Run.\nInt Property Count Auto\n";
    let repaired = repair(source);

    assert!(repaired.starts_with("ScriptName Example\nInt Property Count Auto\n"));
    assert!(repaired.contains("Function Run()\nEndFunction\n; Count is used by Run.\n"));
    assert!(check(&repaired).is_empty());
}

#[test]
fn repair_leaves_already_conforming_source_untouched() {
    let source =
            "ScriptName Example\n\nActor Property PlayerRef Auto\nInt Property Count = 1 Auto\n\nFunction DoThing()\nEndFunction\n";
    assert_eq!(repair(source), source);
}

#[test]
fn repair_preserves_crlf_line_endings() {
    let source = "ScriptName Example\r\n\r\nInt Property Zulu = 1 Auto\r\n\r\nActor Property PlayerRef Auto\r\n";
    let repaired = repair(source);

    assert!(!repaired.contains("\r\n\r\n\n"));
    assert!(repaired.matches("\r\n").count() >= 3);
    assert!(!repaired.replace("\r\n", "").contains('\n'));
    assert!(repaired.find("PlayerRef").unwrap() < repaired.find("Zulu").unwrap());
    assert!(check(&repaired).is_empty());
}

#[test]
fn repair_does_not_crash_on_unparseable_source() {
    let source = "ScriptName Example\n\nInt Property Count = \"unterminated\n";
    assert_eq!(repair(source), source);
}
