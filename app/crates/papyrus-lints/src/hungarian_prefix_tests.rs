use super::*;
use crate::config::Config;

fn enabled(source: &str, mode: Hungarian) -> Vec<Diagnostic> {
    let mut config = Config {
        hungarian: mode,
        ..Config::default()
    };
    config.rules.hungarian_prefix = true;
    crate::lint(source, &config)
        .into_iter()
        .filter(|diagnostic| diagnostic.rule == RULE)
        .collect()
}

fn allow(source: &str) -> Vec<Diagnostic> {
    enabled(source, Hungarian::Allow)
}

fn forbid(source: &str) -> Vec<Diagnostic> {
    enabled(source, Hungarian::Forbid)
}

#[test]
fn direct_check_uses_the_supplied_ast_and_handles_a_missing_ast() {
    let source = "ScriptName Example\n\nFunction F(Int count)\nEndFunction\n";
    let ast = papyrus_parser::parse(source).expect("fixture parses");
    let config = Config {
        hungarian: Hungarian::Allow,
        ..Config::default()
    };

    let diagnostics = check(
        source,
        Some(&ast),
        None,
        &config,
        &mut crate::NoExternalSignatures,
    );
    assert_eq!(diagnostics.len(), 1);
    assert_eq!(diagnostics[0].rule, RULE);
    assert!(diagnostics[0].message.contains("prefix 'ai'"));

    assert!(check(
        source,
        None,
        None,
        &config,
        &mut crate::NoExternalSignatures,
    )
    .is_empty());
}

#[test]
fn stays_off_unless_the_rule_is_enabled() {
    let source = "ScriptName Example\n\nFunction F(Actor target, Int count)\n  Bool done = false\nEndFunction\n";
    assert!(crate::lint(source, &Config::default())
        .iter()
        .all(|diagnostic| diagnostic.rule != RULE));

    let config = Config {
        hungarian: Hungarian::Forbid,
        ..Config::default()
    };
    assert!(crate::lint(source, &config)
        .iter()
        .all(|diagnostic| diagnostic.rule != RULE));
}

#[test]
fn allow_requires_argument_prefixes_that_start_with_a() {
    let source = "\
ScriptName Example

Function F(Actor target, Int count, Bool ready, Float speed, String name)
EndFunction
";
    let diagnostics = allow(source);
    assert_eq!(diagnostics.len(), 5);
    assert!(diagnostics.iter().all(|diagnostic| diagnostic.line == 3));
    assert!(diagnostics[0].message.contains("prefix 'ak'"));
    assert!(diagnostics[0].message.contains("argument Actor"));
    assert!(diagnostics[1].message.contains("prefix 'ai'"));
    assert!(diagnostics[2].message.contains("prefix 'ab'"));
    assert!(diagnostics[3].message.contains("prefix 'af'"));
    assert!(diagnostics[4].message.contains("prefix 'as'"));
    assert!(diagnostics.iter().all(|diagnostic| diagnostic.message.starts_with("[info]")));
}

#[test]
fn allow_accepts_argument_prefixes() {
    let source = "\
ScriptName Example

Function F(Actor akTarget, Int aiCount, Bool abReady, Float afSpeed, String asName)
EndFunction
";
    assert!(allow(source).is_empty());
}

#[test]
fn allow_rejects_a_local_prefix_on_an_argument() {
    let diagnostics = allow("ScriptName Example\n\nFunction F(Int iCount)\nEndFunction\n");
    assert_eq!(diagnostics.len(), 1);
    assert!(diagnostics[0].message.contains("prefix 'ai'"));
    assert!(diagnostics[0].message.contains("not 'i'"));
}

#[test]
fn allow_requires_local_prefixes_without_a_leading_a() {
    let source = "\
ScriptName Example

Function F()
  Bool done = false
  Int count = 0
  Float speed = 0.0
  String name = \"\"
  Actor target = None
EndFunction
";
    let diagnostics = allow(source);
    assert_eq!(diagnostics.len(), 5);
    assert!(diagnostics[0].message.contains("prefix 'b'"));
    assert!(diagnostics[0].message.contains("for Bool"));
    assert!(!diagnostics[0].message.contains("argument"));
    assert!(diagnostics[1].message.contains("prefix 'i'"));
    assert!(diagnostics[4].message.contains("prefix 'k'"));
}

#[test]
fn allow_accepts_local_prefixes() {
    let source = "\
ScriptName Example

Function F()
  Bool bDone = false
  Int iCount = 0
  Float fSpeed = 0.0
  String sName = \"\"
  Actor kTarget = None
  Int i2 = 1
EndFunction
";
    assert!(allow(source).is_empty());
}

#[test]
fn allow_uses_the_element_type_for_arrays() {
    let bad = "\
ScriptName Example

Function F(Int[] values, Actor[] actors)
  Int[] counts = new Int[1]
EndFunction
";
    let diagnostics = allow(bad);
    assert_eq!(diagnostics.len(), 3);
    assert!(diagnostics[0].message.contains("prefix 'ai'"));
    assert!(diagnostics[0].message.contains("Int[]"));
    assert!(diagnostics[1].message.contains("prefix 'ak'"));
    assert!(diagnostics[2].message.contains("prefix 'i'"));

    let good = "\
ScriptName Example

Function F(Int[] aiValues, Actor[] akActors)
  Int[] iCounts = new Int[1]
  Actor[] kActors = new Actor[1]
EndFunction
";
    assert!(allow(good).is_empty());
}

#[test]
fn a_word_that_only_starts_with_the_letter_is_not_a_prefix() {
    let source = "ScriptName Example\n\nFunction F()\n  Int index = 0\n  Bool ready = false\nEndFunction\n";
    assert_eq!(allow(source).len(), 2);
    assert!(forbid(source).is_empty());
}

#[test]
fn allow_checks_script_variables_and_state_parameters_but_not_properties_or_functions() {
    let source = "\
ScriptName Example

Int count
Actor Property PlayerRef Auto
Bool Property bIsReady Auto

Function bDoThing(Int count)
EndFunction

State Active
  Function Run(Actor target)
  EndFunction
EndState
";
    let diagnostics = allow(source);
    let messages: Vec<_> = diagnostics
        .iter()
        .map(|diagnostic| diagnostic.message.as_str())
        .collect();
    assert_eq!(messages.len(), 3, "{messages:?}");
    assert!(messages[0].contains("Variable 'count'"));
    assert!(messages[1].contains("Parameter 'count'"));
    assert!(messages[2].contains("Parameter 'target'"));
}

#[test]
fn forbid_flags_prefixes_and_leaves_plain_names() {
    let source = "\
ScriptName Example

Function F(Actor akTarget, Int aiCount, Int plain)
  Bool bDone = false
  Int count = 0
EndFunction
";
    let diagnostics = forbid(source);
    assert_eq!(diagnostics.len(), 3);
    assert!(diagnostics[0].message.contains("Parameter 'akTarget' uses Hungarian prefix 'ak'"));
    assert!(diagnostics[1].message.contains("Parameter 'aiCount' uses Hungarian prefix 'ai'"));
    assert!(diagnostics[2].message.contains("Variable 'bDone' uses Hungarian prefix 'b'"));
}

#[test]
fn forbid_exempts_event_parameters_but_not_event_locals() {
    let source = "\
ScriptName Example

Event OnHit(ObjectReference akAggressor, Form akSource)
  Bool bDone = false
  Int count = 0
EndEvent
";
    let diagnostics = forbid(source);
    assert_eq!(diagnostics.len(), 1);
    assert!(diagnostics[0].message.contains("Variable 'bDone'"));

    let missing = allow("ScriptName Example\n\nEvent OnHit(ObjectReference aggressor)\nEndEvent\n");
    assert_eq!(missing.len(), 1);
    assert!(missing[0].message.contains("prefix 'ak'"));

    assert!(allow(
        "ScriptName Example\n\nEvent OnHit(ObjectReference akAggressor, Int aiItemCount)\nEndEvent\n"
    )
    .is_empty());
}

#[test]
fn prefix_match_is_case_sensitive() {
    let diagnostics = allow("ScriptName Example\n\nFunction F(Int AICount)\nEndFunction\n");
    assert_eq!(diagnostics.len(), 1);
    assert!(!diagnostics[0].message.contains("not '"));
}

#[test]
fn bool_type_spelling_does_not_matter() {
    assert!(allow("ScriptName Example\n\nFunction F(bool abReady)\nEndFunction\n").is_empty());
}

#[test]
fn custom_types_use_k() {
    assert!(allow("ScriptName Example\n\nFunction F(MyQuest akQuest)\nEndFunction\n").is_empty());
    let diagnostics = allow("ScriptName Example\n\nFunction F(MyQuest quest)\nEndFunction\n");
    assert!(diagnostics[0].message.contains("prefix 'ak'"));
    assert!(diagnostics[0].message.contains("MyQuest"));
}

#[test]
fn fragment_wrapper_signatures_are_skipped_and_begin_code_locals_are_not() {
    let source = "\
;BEGIN FRAGMENT CODE - Do not edit anything between this and the end comment
Scriptname Example Extends TopicInfo Hidden
Function Fragment_0(ObjectReference speaker)
;BEGIN CODE
Int total = 1
;END CODE
EndFunction
;END FRAGMENT CODE - Do not edit anything between this and the begin comment
";
    let diagnostics = allow(source);
    assert_eq!(diagnostics.len(), 1);
    assert!(diagnostics[0].message.contains("Variable 'total'"));
    assert!(diagnostics[0].message.contains("prefix 'i'"));
}

#[test]
fn line_and_file_disable_comments_suppress_the_rule() {
    let mut config = Config::default();
    config.rules.hungarian_prefix = true;

    assert!(crate::lint(
        "ScriptName Example\n\nFunction F(Actor target) ; @disable hungarian-prefix\nEndFunction\n",
        &config,
    )
    .iter()
    .all(|diagnostic| diagnostic.rule != RULE));
    assert!(crate::lint(
        "; @disable-file hungarian-prefix\nScriptName Example\n\nFunction F(Actor target)\nEndFunction\n",
        &config,
    )
    .iter()
    .all(|diagnostic| diagnostic.rule != RULE));
    assert!(crate::lint(
        "ScriptName Example\n\nFunction F()\n  Bool done = false ; @disable hungarian-prefix\nEndFunction\n",
        &config,
    )
    .iter()
    .all(|diagnostic| diagnostic.rule != RULE));
}
