//! Integration test using a real-world Papyrus script to make sure the
//! `Length` keyword (Papyrus's array-length property, e.g. `array.Length`)
//! parses as a member access rather than being rejected as a keyword where
//! an identifier is expected.

const FIXTURE: &str = include_str!("fixtures/idrinthDisableImmersiveCitizens.psc");

#[test]
fn parses_array_length_property_access() {
    let script = papyrus_parser::parse(FIXTURE).unwrap();
    assert_eq!(script.name, "idrinthDisableImmersiveCitizens");
    assert_eq!(script.functions.len(), 1);
    assert!(script.functions[0].is_event);
    assert_eq!(script.functions[0].name, "OnInit");
}
