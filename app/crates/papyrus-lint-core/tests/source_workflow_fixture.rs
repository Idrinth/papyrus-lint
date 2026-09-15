//! Public-API integration coverage for the source-file utilities used by
//! both frontends. These tests keep the achlist, encoding, hashing, diff,
//! write-back, and stale-output contracts aligned as one workflow instead
//! of exercising each module only in isolation.

use std::fs;
use std::path::Path;
use std::time::{Duration, SystemTime};

use papyrus_lint_core::achlist::parse_achlist;
use papyrus_lint_core::content_hash::md5_hex;
use papyrus_lint_core::diff::unified_diff;
use papyrus_lint_core::source_encoding::{
    read_psc_source_with_encoding, write_psc_source, PscEncoding,
};
use papyrus_lint_core::stale_pex;

fn write(path: &Path, contents: impl AsRef<[u8]>) {
    fs::create_dir_all(path.parent().expect("fixture path should have a parent"))
        .expect("failed to create fixture directory");
    fs::write(path, contents).expect("failed to write fixture");
}

fn set_modified(path: &Path, modified: SystemTime) {
    fs::File::open(path)
        .expect("fixture should exist")
        .set_modified(modified)
        .expect("failed to set fixture modification time");
}

#[test]
fn achlist_source_can_be_repaired_without_changing_its_windows_1252_encoding() {
    let project = tempfile::tempdir().expect("failed to create project directory");
    let data = project.path().join("Data");
    let script = data.join("Scripts/Source/Encoded.psc");
    write(&script, b"ScriptName Encoded\r\n; caf\xE9   \r\n");
    let achlist = data.join("project.achlist");
    write(&achlist, br#"["Data/Scripts/Source/Encoded.psc"]"#);

    let paths = parse_achlist(&achlist).expect("achlist should parse");
    let (source, encoding) =
        read_psc_source_with_encoding(&paths[0]).expect("source should be readable");
    let repaired = source.replace("   \r\n", "\r\n");
    write_psc_source(&paths[0], &repaired, encoding).expect("repair should be writable");

    assert_eq!(paths.as_slice(), std::slice::from_ref(&script));
    assert_eq!(encoding, PscEncoding::Windows1252);
    assert_eq!(
        fs::read(script).expect("repaired source should be readable"),
        b"ScriptName Encoded\r\n; caf\xE9\r\n"
    );
}

#[test]
fn repair_preview_reports_the_change_but_leaves_the_source_untouched() {
    let project = tempfile::tempdir().expect("failed to create project directory");
    let script = project.path().join("Scripts/Source/Example.psc");
    let original = "ScriptName Example\nFunction Run()   \nEndFunction\n";
    write(&script, original);

    let (source, encoding) =
        read_psc_source_with_encoding(&script).expect("source should be readable");
    let repaired = source.replace("()   \n", "()\n");
    let preview = unified_diff(&script.display().to_string(), &source, &repaired);

    assert_eq!(encoding, PscEncoding::Utf8);
    assert!(preview.contains("-Function Run()   \n+Function Run()\n"));
    assert_eq!(
        fs::read_to_string(script).expect("source should remain readable"),
        original
    );
}

#[test]
fn source_hash_tracks_the_decoded_text_across_a_write_back() {
    let project = tempfile::tempdir().expect("failed to create project directory");
    let script = project.path().join("Scripts/Source/Encoded.psc");
    write(&script, b"ScriptName Encoded\r\n; \x93quoted\x94\r\n");

    let (source, encoding) =
        read_psc_source_with_encoding(&script).expect("source should be readable");
    let before = md5_hex(&source);
    write_psc_source(&script, &source, encoding).expect("source should be writable");
    let (round_tripped, round_tripped_encoding) =
        read_psc_source_with_encoding(&script).expect("source should still be readable");

    assert_eq!(round_tripped_encoding, PscEncoding::Windows1252);
    assert_eq!(round_tripped, source);
    assert_eq!(md5_hex(&round_tripped), before);
}

#[test]
fn stale_output_diagnostic_clears_after_the_compiled_file_is_refreshed() {
    let project = tempfile::tempdir().expect("failed to create project directory");
    let script = project.path().join("Scripts/Source/Example.psc");
    let compiled = project.path().join("Scripts/Example.pex");
    write(&script, "ScriptName Example\n");
    write(&compiled, b"compiled");
    let baseline = SystemTime::now() - Duration::from_secs(120);
    set_modified(&compiled, baseline);
    set_modified(&script, baseline + Duration::from_secs(30));

    let diagnostic = stale_pex::check(&script).expect("output should initially be stale");
    set_modified(&compiled, baseline + Duration::from_secs(60));

    assert_eq!(diagnostic.rule, stale_pex::RULE);
    assert!(diagnostic.message.contains(&compiled.display().to_string()));
    assert!(stale_pex::check(&script).is_none());
}
