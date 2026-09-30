use super::*;

fn parse(source: &str) -> papyrus_parser::ast::Script {
    papyrus_parser::parse(source).expect("test script should parse")
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
