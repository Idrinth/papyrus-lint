use super::*;

#[test]
fn ignores_clean_chains() {
    let source =
        "ScriptName Example\n\nFunction Test()\n    SomeProperty.DoThing().Other()\nEndFunction\n";
    assert!(check(source).is_empty());
}

#[test]
fn flags_whitespace_before_dot() {
    let diagnostics = check("SomeProperty .DoThing()\n");
    assert_eq!(diagnostics.len(), 1);
    assert_eq!(diagnostics[0].line, 1);
    assert!(diagnostics[0].message.starts_with("[warning]"));
    assert!(diagnostics[0].message.contains("before"));
}

#[test]
fn flags_whitespace_after_dot() {
    let diagnostics = check("SomeProperty. DoThing()\n");
    assert_eq!(diagnostics.len(), 1);
    assert_eq!(diagnostics[0].line, 1);
    assert!(diagnostics[0].message.contains("after"));
}

#[test]
fn flags_whitespace_on_both_sides_as_two_diagnostics() {
    let diagnostics = check("SomeProperty . DoThing()\n");
    assert_eq!(diagnostics.len(), 2);
    assert!(diagnostics.iter().any(|d| d.message.contains("before")));
    assert!(diagnostics.iter().any(|d| d.message.contains("after")));
}

#[test]
fn flags_a_tab_the_same_as_a_space() {
    let diagnostics = check("SomeProperty\t.DoThing()\n");
    assert_eq!(diagnostics.len(), 1);
    assert!(diagnostics[0].message.contains("before"));
}

#[test]
fn flags_each_interrupted_dot_in_a_longer_chain_independently() {
    let diagnostics = check("a. b.c .d\n");
    assert_eq!(diagnostics.len(), 2);
}

#[test]
fn ignores_float_literals() {
    assert!(
        check("ScriptName Example\n\nFunction Test()\n    Float x = 1.5\nEndFunction\n").is_empty()
    );
}

#[test]
fn fragment_code_wrapper_dots_are_left_alone() {
    let source = "\
;BEGIN FRAGMENT CODE - Do not edit anything between this and the end comment
Function Fragment_0 (ObjectReference akSpeakerRef)
;BEGIN CODE
akSpeaker . RemoveItem(x, 1, false, PlayerRef)
;END CODE
EndFunction
;END FRAGMENT CODE - Do not edit anything between this and the begin comment
";
    let diagnostics = check(source);
    assert!(diagnostics.iter().all(|d| d.line == 4));
    assert_eq!(diagnostics.len(), 2);
}

#[test]
fn repair_leaves_clean_chains_untouched() {
    let source =
        "ScriptName Example\n\nFunction Test()\n    SomeProperty.DoThing().Other()\nEndFunction\n";
    assert_eq!(repair(source), source);
}

#[test]
fn repair_closes_whitespace_before_a_dot() {
    assert_eq!(
        repair("SomeProperty .DoThing()\n"),
        "SomeProperty.DoThing()\n"
    );
}

#[test]
fn repair_closes_whitespace_after_a_dot() {
    assert_eq!(
        repair("SomeProperty. DoThing()\n"),
        "SomeProperty.DoThing()\n"
    );
}

#[test]
fn repair_closes_whitespace_on_both_sides() {
    assert_eq!(
        repair("SomeProperty . DoThing()\n"),
        "SomeProperty.DoThing()\n"
    );
}

#[test]
fn repair_closes_runs_of_multiple_spaces_and_tabs() {
    assert_eq!(
        repair("SomeProperty  \t . \t  DoThing()\n"),
        "SomeProperty.DoThing()\n"
    );
}

#[test]
fn repair_closes_every_interrupted_dot_in_a_longer_chain() {
    assert_eq!(repair("a. b.c .d\n"), "a.b.c.d\n");
}

#[test]
fn repair_result_has_no_remaining_diagnostics() {
    let source = "a . b . c\n";
    let repaired = repair(source);
    assert_eq!(repaired, "a.b.c\n");
    assert!(check(&repaired).is_empty());
}

#[test]
fn repair_leaves_float_literals_alone() {
    let source = "ScriptName Example\n\nFunction Test()\n    Float x = 1.5\nEndFunction\n";
    assert_eq!(repair(source), source);
}

#[test]
fn repair_fixes_the_code_body_but_leaves_the_wrapper_alone() {
    // Mirrors `fragment_code_wrapper_dots_are_left_alone` above: the
    // wrapper boilerplate (including the generated function signature,
    // deliberately given a space-interrupted default-value chain here to
    // prove it's left alone) must come out byte-for-byte identical, while
    // the actual code between `;BEGIN CODE`/`;END CODE` gets fixed.
    let source = "\
;BEGIN FRAGMENT CODE - Do not edit anything between this and the end comment
Function Fragment_0 (ObjectReference akSpeakerRef = akRoot . GetRef())
;BEGIN CODE
akSpeaker . RemoveItem(x, 1, false, PlayerRef)
;END CODE
EndFunction
;END FRAGMENT CODE - Do not edit anything between this and the begin comment
";
    assert_eq!(
        repair(source),
        "\
;BEGIN FRAGMENT CODE - Do not edit anything between this and the end comment
Function Fragment_0 (ObjectReference akSpeakerRef = akRoot . GetRef())
;BEGIN CODE
akSpeaker.RemoveItem(x, 1, false, PlayerRef)
;END CODE
EndFunction
;END FRAGMENT CODE - Do not edit anything between this and the begin comment
"
    );
}

#[test]
fn repair_does_not_reach_across_a_newline() {
    // A chain continued onto another physical line (no trailing "\" line
    // continuation) is a different statement as far as the lexer is
    // concerned, not whitespace interrupting a single dot access.
    let source = "a\n.b\n";
    assert_eq!(repair(source), source);
}

#[test]
fn repair_is_idempotent() {
    let repaired = repair("SomeProperty  .  DoThing() .Other()\n");
    assert_eq!(repair(&repaired), repaired);
}
