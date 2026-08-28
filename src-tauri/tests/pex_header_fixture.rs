//! Integration test using a real `PapyrusCompiler.exe`-produced `.pex`
//! file, complete with an embedded UTF-8 username, to make sure
//! [`pex_header::strip_personal_data`] handles real-world output and not
//! just synthesized headers.

use papyrus_lint_lib::pex_header;

const FIXTURE: &[u8] = include_bytes!("fixtures/IDR__TIF__05000235.pex");

fn contains_bytes(haystack: &[u8], needle: &[u8]) -> bool {
    haystack.windows(needle.len()).any(|w| w == needle)
}

#[test]
fn fixture_embeds_the_compiling_machines_personal_data() {
    assert!(contains_bytes(FIXTURE, "Björn".as_bytes()));
    assert!(contains_bytes(FIXTURE, b"BJOERN-LENOVOW5"));
}

#[test]
fn strips_the_username_and_machine_name_from_a_real_compiled_pex() {
    let patched = pex_header::strip_personal_data(FIXTURE).expect("should strip");

    assert!(!contains_bytes(&patched, "Björn".as_bytes()));
    assert!(!contains_bytes(&patched, b"BJOERN-LENOVOW5"));
    // The source file name and the rest of the compiled script must survive untouched.
    assert!(contains_bytes(&patched, b"IDR__TIF__05000235.psc"));
    assert_eq!(
        patched.len(),
        FIXTURE.len() - "Björn".len() - "BJOERN-LENOVOW5".len()
    );
}
