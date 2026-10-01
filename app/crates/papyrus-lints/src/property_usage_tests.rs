use super::*;

fn parse(source: &str) -> papyrus_parser::ast::Script {
    papyrus_parser::parse(source).expect("test script should parse")
}

fn parse_for_game(source: &str, game: crate::Game) -> papyrus_parser::ast::Script {
    papyrus_parser::parse_for_game(source, game).expect("test script should parse")
}

#[test]
fn line_has_external_handles_annotations_and_out_of_range_lines() {
    let source = "ScriptName Example\nInt Property Value Auto ; @ExTeRnAl\n";

    assert!(line_has_external(source, 2));
    assert!(!line_has_external(source, 1));
    assert!(!line_has_external(source, 0));
    assert!(!line_has_external(source, 20));
}

#[test]
fn collect_usage_walks_conditions_indexes_calls_and_state_functions() {
    let script = parse(
        "ScriptName Example\n\
         Bool Property Enabled Auto\n\
         Int[] Property Values Auto\n\
         Int Property Index Auto\n\
         Int Property Result Auto\n\
         Function Fill()\n\
           If Enabled\n\
             Values[Index] = Result\n\
           EndIf\n\
         EndFunction\n\
         State Running\n\
           Event OnBeginState()\n\
             Result = Values[Index]\n\
           EndEvent\n\
         EndState\n",
    );

    let usage = collect_usage(&script);

    assert!(usage["enabled"].read);
    assert!(usage["values"].read);
    assert!(!usage["values"].written);
    assert!(usage["index"].read);
    assert!(usage["result"].read);
    assert!(usage["result"].written);
}

#[test]
fn collect_usage_walks_property_accessors() {
    let script = parse(
        "ScriptName Example\n\
         Int Property Source Auto\n\
         Int Property Computed\n\
           Int Function Get()\n\
             Return Source\n\
           EndFunction\n\
           Function Set(Int value)\n\
             Source = value\n\
           EndFunction\n\
         EndProperty\n",
    );

    let usage = collect_usage(&script);

    assert!(usage["source"].read);
    assert!(usage["source"].written);
}

#[test]
fn backing_fields_find_nested_fields_and_ignore_setter_parameters() {
    let script = parse(
        "ScriptName Example\n\
         Int first\n\
         Int second\n\
         Int Property Value\n\
           Int Function Get()\n\
             If first > 0\n\
               Return first\n\
             Else\n\
               While second < 0\n\
                 second = 0\n\
               EndWhile\n\
               Return second\n\
             EndIf\n\
           EndFunction\n\
           Function Set(Int value)\n\
             value = 1\n\
             first = value\n\
           EndFunction\n\
         EndProperty\n",
    );

    let fields = backing_fields(&script.properties[0]);

    assert_eq!(fields, vec!["first", "second"]);
}

#[test]
fn written_outside_accessors_checks_initializers_and_state_bodies() {
    let initialized = parse(
        "ScriptName Example\n\
         Int stored = 1\n\
         Int Property Value\n\
           Int Function Get()\n\
             Return stored\n\
           EndFunction\n\
         EndProperty\n",
    );
    let property = &initialized.properties[0];
    let backing = backing_fields(property);
    assert!(written_outside_accessors(&initialized, property, &backing));

    let state_write = parse(
        "ScriptName Example\n\
         Int stored\n\
         Int Property Value\n\
           Int Function Get()\n\
             Return stored\n\
           EndFunction\n\
         EndProperty\n\
         State Running\n\
           Event OnBeginState()\n\
             stored = 1\n\
           EndEvent\n\
         EndState\n",
    );
    let property = &state_write.properties[0];
    let backing = backing_fields(property);
    assert!(written_outside_accessors(&state_write, property, &backing));
}

#[test]
fn all_properties_includes_groups_and_collect_usage_handles_no_properties() {
    let script = parse_for_game(
        "ScriptName Example\n\
         Int Property TopLevel Auto\n\
         Group Settings\n\
           Bool Property Grouped Auto\n\
         EndGroup\n",
        crate::Game::Fallout4,
    );

    let names: Vec<&str> = all_properties(&script)
        .into_iter()
        .map(|property| property.name.as_str())
        .collect();
    assert_eq!(names, ["TopLevel", "Grouped"]);

    let script = parse("ScriptName Empty\nFunction Run()\nEndFunction\n");
    assert!(collect_usage(&script).is_empty());
}

#[test]
fn collect_usage_distinguishes_self_properties_from_other_objects() {
    let script = parse(
        "ScriptName Example\n\
         Int Property Score Auto\n\
         Int Property RemoteOnly Auto\n\
         Function Update(Example other)\n\
           other.Score = other.Score + 1\n\
           other.RemoteOnly = other.RemoteOnly + 1\n\
           self.Score = 2\n\
           Debug.Trace(self.Score)\n\
         EndFunction\n",
    );

    let usage = collect_usage(&script);

    assert!(usage["score"].read);
    assert!(usage["score"].written);
    assert!(!usage["remoteonly"].read);
    assert!(!usage["remoteonly"].written);
}

#[test]
fn written_outside_accessors_walks_both_lock_guard_paths() {
    let script = parse_for_game(
        "ScriptName Example\n\
         Guard DataGuard\n\
         Int stored\n\
         Int Property Value\n\
           Int Function Get()\n\
             Return stored\n\
           EndFunction\n\
         EndProperty\n\
         Function Update(Bool acquired)\n\
           TryLockGuard DataGuard\n\
             stored = 1\n\
           ElseTryLockGuard\n\
             Value = 2\n\
           EndTryLockGuard\n\
         EndFunction\n",
        crate::Game::Starfield,
    );
    let property = &script.properties[0];

    assert!(written_outside_accessors(
        &script,
        property,
        &backing_fields(property)
    ));

    let accessor_only = parse(
        "ScriptName Example\n\
         Int stored\n\
         Int Property Value\n\
           Int Function Get()\n\
             Return stored\n\
           EndFunction\n\
           Function Set(Int value)\n\
             stored = value\n\
           EndFunction\n\
         EndProperty\n",
    );
    let property = &accessor_only.properties[0];
    assert!(!written_outside_accessors(
        &accessor_only,
        property,
        &backing_fields(property)
    ));
}
