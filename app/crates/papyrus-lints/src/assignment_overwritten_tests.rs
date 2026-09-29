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

fn check_starfield(source: &str) -> Vec<Diagnostic> {
    let ast = papyrus_parser::parse_with_mode(
        source,
        papyrus_parser::parser::GameEdition::Starfield,
    )
    .ok();
    let tokens = papyrus_parser::tokenize(source).ok();
    let config = crate::config::Config {
        game: crate::Game::Starfield,
        ..Default::default()
    };
    super::check(
        source,
        ast.as_ref(),
        tokens.as_deref(),
        &config,
        &mut crate::external_signatures::NoExternalSignatures,
    )
}

fn script(body: &str) -> String {
    format!("ScriptName Example\n\nFunction Test()\n{body}EndFunction\n")
}

fn guarded_script(body: &str) -> String {
    format!("ScriptName Example\n\nGuard WorkGuard\n\nFunction Test()\n{body}EndFunction\n")
}

#[test]
fn flags_assignment_overwritten_before_read() {
    let diagnostics = check(&script("    Int x = 1\n    x = 2\n    Debug.Trace(x)\n"));

    assert_eq!(diagnostics.len(), 1);
    assert_eq!(diagnostics[0].line, 4);
    assert_eq!(diagnostics[0].rule, RULE);
    assert!(diagnostics[0].message.starts_with("[warning]"));
    assert!(diagnostics[0].message.contains("'x'"));
    assert!(diagnostics[0].message.contains("overwritten"));
}

#[test]
fn flags_back_to_back_assignments() {
    let diagnostics = check(&script("    Int x\n    x = 1\n    x = 2\n    Debug.Trace(x)\n"));

    assert_eq!(diagnostics.len(), 1);
    assert_eq!(diagnostics[0].line, 5);
}

#[test]
fn flags_compound_assignment_overwritten_before_read() {
    let diagnostics = check(&script("    Int x = 1\n    x += 1\n    x = 3\n    Debug.Trace(x)\n"));

    assert_eq!(diagnostics.len(), 1);
    assert_eq!(diagnostics[0].line, 5);
}

#[test]
fn does_not_flag_when_value_is_read_before_next_write() {
    let diagnostics = check(&script("    Int x = 1\n    Debug.Trace(x)\n    x = 2\n"));

    assert!(diagnostics.is_empty());
}

#[test]
fn does_not_flag_when_right_hand_side_reads_the_variable() {
    let diagnostics = check(&script("    Int x = 1\n    x = x + 1\n    Debug.Trace(x)\n"));

    assert!(diagnostics.is_empty());
}

#[test]
fn does_not_flag_a_single_assignment_that_is_later_read() {
    let diagnostics = check(&script("    Int x = 1\n    Debug.Trace(x)\n"));

    assert!(diagnostics.is_empty());
}

#[test]
fn matches_variable_name_case_insensitively() {
    let diagnostics = check(&script("    Int x = 1\n    X = 2\n    Debug.Trace(X)\n"));

    assert_eq!(diagnostics.len(), 1);
    assert_eq!(diagnostics[0].line, 4);
}

#[test]
fn flags_parameter_overwritten_before_incoming_value_is_read() {
    let diagnostics =
        check("ScriptName Example\n\nFunction Test(Int total)\n    total = 1\n    Debug.Trace(total)\nEndFunction\n");

    assert_eq!(diagnostics.len(), 1);
    assert_eq!(diagnostics[0].line, 3);
    assert_eq!(diagnostics[0].rule, RULE);
    assert!(diagnostics[0].message.starts_with("[warning]"));
    assert!(diagnostics[0].message.contains("'total'"));
    assert!(diagnostics[0].message.contains("incoming"));
}

#[test]
fn flags_parameter_assignment_overwritten_before_read() {
    let diagnostics =
        check("ScriptName Example\n\nFunction Test(Int total)\n    Debug.Trace(total)\n    total = 1\n    total = 2\n    Debug.Trace(total)\nEndFunction\n");

    assert_eq!(diagnostics.len(), 1);
    assert_eq!(diagnostics[0].line, 5);
    assert!(diagnostics[0].message.contains("parameter"));
    assert!(diagnostics[0].message.contains("'total'"));
}

#[test]
fn does_not_flag_parameter_read_before_it_is_assigned() {
    let diagnostics =
        check("ScriptName Example\n\nFunction Test(Int total)\n    Debug.Trace(total)\n    total = 1\nEndFunction\n");

    assert!(diagnostics.is_empty());
}

#[test]
fn does_not_flag_script_properties() {
    let diagnostics = check(
        "ScriptName Example\n\nInt Property Count Auto\n\nFunction Test()\n    Count = 1\n    Count = 2\nEndFunction\n",
    );

    assert!(diagnostics.is_empty());
}

#[test]
fn does_not_flag_member_or_index_assignment_as_a_local_write() {
    let diagnostics = check(&script(
        "    Int[] arr = new Int[3]\n    arr[0] = 1\n    arr[0] = 2\n",
    ));

    assert!(diagnostics.is_empty());
}

#[test]
fn flags_overwritten_write_inside_if_block() {
    let diagnostics = check(&script(
        "    If true\n        Int x = 1\n        x = 2\n        Debug.Trace(x)\n    EndIf\n",
    ));

    assert_eq!(diagnostics.len(), 1);
    assert_eq!(diagnostics[0].line, 5);
}

#[test]
fn does_not_flag_one_sided_if_overwrite_when_value_is_used_after() {
    let diagnostics = check(&script(
        "    Int x = 1\n    If true\n        x = 2\n    EndIf\n    Debug.Trace(x)\n",
    ));

    assert!(diagnostics.is_empty());
}

#[test]
fn flags_incoming_write_overwritten_on_every_branch() {
    let diagnostics = check(&script(
        "    Int x = 1\n    If true\n        x = 2\n    Else\n        x = 3\n    EndIf\n    Debug.Trace(x)\n",
    ));

    assert_eq!(diagnostics.len(), 1);
    assert_eq!(diagnostics[0].line, 4);
}

#[test]
fn does_not_flag_incoming_write_read_on_one_branch_before_overwrite() {
    let diagnostics = check(&script(
        "    Int x = 1\n    If true\n        Debug.Trace(x)\n        x = 2\n    Else\n        x = 3\n    EndIf\n",
    ));

    assert!(diagnostics.is_empty());
}

#[test]
fn does_not_flag_write_before_while_overwritten_only_inside_the_loop() {
    let diagnostics = check(&script(
        "    Int x = 1\n    While false\n        x = 2\n    EndWhile\n    Debug.Trace(x)\n",
    ));

    assert!(diagnostics.is_empty());
}

#[test]
fn flags_overwritten_write_inside_while_body() {
    let diagnostics = check(&script(
        "    While true\n        Int x = 1\n        x = 2\n        Debug.Trace(x)\n    EndWhile\n",
    ));

    assert_eq!(diagnostics.len(), 1);
    assert_eq!(diagnostics[0].line, 5);
}

#[test]
fn flags_overwritten_write_inside_lock_guard() {
    let diagnostics = check_starfield(&guarded_script(
        "    LockGuard WorkGuard\n        Int x = 1\n        x = 2\n        Debug.Trace(x)\n    EndLockGuard\n",
    ));

    assert_eq!(diagnostics.len(), 1);
    assert_eq!(diagnostics[0].line, 7);
}

#[test]
fn flags_incoming_write_overwritten_on_both_try_lock_paths() {
    let diagnostics = check_starfield(&guarded_script(
        "    Int x = 1\n    TryLockGuard WorkGuard\n        x = 2\n    ElseTryLockGuard\n        x = 3\n    EndTryLockGuard\n    Debug.Trace(x)\n",
    ));

    assert_eq!(diagnostics.len(), 1);
    assert_eq!(diagnostics[0].line, 6);
}

#[test]
fn does_not_flag_incoming_write_overwritten_on_only_one_try_lock_path() {
    let diagnostics = check_starfield(&guarded_script(
        "    Int x = 1\n    TryLockGuard WorkGuard\n        x = 2\n    EndTryLockGuard\n    Debug.Trace(x)\n",
    ));

    assert!(diagnostics.is_empty());
}

#[test]
fn does_not_flag_try_lock_write_read_on_the_alternate_path() {
    let diagnostics = check_starfield(&guarded_script(
        "    Int x = 1\n    TryLockGuard WorkGuard\n        x = 2\n    ElseTryLockGuard\n        Debug.Trace(x)\n        x = 3\n    EndTryLockGuard\n",
    ));

    assert!(diagnostics.is_empty());
}

#[test]
fn diverging_try_lock_paths_discard_the_incoming_write() {
    let diagnostics = check_starfield(&guarded_script(
        "    Int x = 1\n    TryLockGuard WorkGuard\n        Return\n    ElseTryLockGuard\n        Return\n    EndTryLockGuard\n    x = 2\n",
    ));

    assert!(diagnostics.is_empty());
}

#[test]
fn a_named_argument_reads_its_value_before_a_later_write() {
    let diagnostics = check(&script(
        "    Int x = 1\n    Consume(value = x)\n    x = 2\n",
    ));

    assert!(diagnostics.is_empty());
}

#[test]
fn checks_functions_declared_in_states_too() {
    let diagnostics = check(
        "ScriptName Example\n\nState Active\n    Function Test()\n        Int x = 1\n        x = 2\n        Debug.Trace(x)\n    EndFunction\nEndState\n",
    );

    assert_eq!(diagnostics.len(), 1);
    assert!(diagnostics[0].message.contains("'x'"));
}

#[test]
fn does_not_crash_on_unparseable_source() {
    assert!(check("ScriptName Example\n\nFunction Test(\nEndFunction\n").is_empty());
}

#[test]
fn does_not_flag_a_generated_assignment_in_a_fragment_wrapper() {
    let source = "\
;BEGIN FRAGMENT CODE - Do not edit anything between this and the end comment\n;NEXT FRAGMENT INDEX 0\nScriptname IDR__TIF__05000235 Extends TopicInfo Hidden\n\n;BEGIN FRAGMENT Fragment_0\nFunction Fragment_0(ObjectReference akSpeakerRef)\nActor akSpeaker = akSpeakerRef as Actor\nakSpeaker = None\n;BEGIN CODE\nPlayerRef.RemoveItem(Gold001, 5)\n;END CODE\nEndFunction\n;END FRAGMENT\n\n;END FRAGMENT CODE - Do not edit anything between this and the begin comment\nActor Property PlayerRef Auto\nMiscObject Property Gold001 Auto\n";

    assert!(check(source).is_empty());
}

#[test]
fn still_flags_an_overwritten_local_inside_the_code_block() {
    let source = "\
;BEGIN FRAGMENT CODE - Do not edit anything between this and the end comment\n;NEXT FRAGMENT INDEX 0\nScriptname IDR__TIF__05000235 Extends TopicInfo Hidden\n\n;BEGIN FRAGMENT Fragment_0\nFunction Fragment_0(ObjectReference akSpeakerRef)\nActor akSpeaker = akSpeakerRef as Actor\n;BEGIN CODE\nInt x = 1\nx = 2\nDebug.Trace(x)\n;END CODE\nEndFunction\n;END FRAGMENT\n\n;END FRAGMENT CODE - Do not edit anything between this and the begin comment\n";

    let diagnostics = check(source);
    assert_eq!(diagnostics.len(), 1);
    assert_eq!(diagnostics[0].rule, RULE);
}

#[test]
fn line_and_file_disable_comments_suppress_diagnostics() {
    let line_disabled = crate::lint(
        &script("    Int x = 1 ; @disable assignment-overwritten\n    x = 2\n    Debug.Trace(x)\n"),
        &crate::config::Config::default(),
    );
    let file_disabled = crate::lint(
        &format!(
            "; @disable-file assignment-overwritten\n{}",
            script("    Int x = 1\n    x = 2\n    Debug.Trace(x)\n")
        ),
        &crate::config::Config::default(),
    );

    assert!(line_disabled
        .iter()
        .all(|diagnostic| diagnostic.line != 4 || diagnostic.rule != RULE));
    assert!(file_disabled.iter().all(|diagnostic| diagnostic.rule != RULE));
}

#[test]
fn config_off_switch_suppresses_diagnostics() {
    let source = script("    Int x = 1\n    x = 2\n    Debug.Trace(x)\n");
    let mut config = crate::config::Config::default();
    config.rules.assignment_overwritten = false;

    let diagnostics = crate::lint(&source, &config);

    assert!(diagnostics.iter().all(|diagnostic| diagnostic.rule != RULE));
}
