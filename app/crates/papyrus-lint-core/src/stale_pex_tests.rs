use super::*;
use std::fs;
use std::time::{Duration, SystemTime};

fn touch_with_time(path: &Path, modified: SystemTime) {
    fs::write(path, "").expect("failed to write file");
    let file = fs::File::open(path).expect("failed to open file");
    file.set_modified(modified).expect("failed to set mtime");
}

#[test]
fn flags_a_script_newer_than_its_compiled_output() {
    let root = tempfile::tempdir().expect("failed to create temp dir");
    let source_dir = root.path().join("Scripts").join("Source");
    fs::create_dir_all(&source_dir).expect("failed to create source dir");
    let script_path = source_dir.join("Example.psc");
    let pex_path = root.path().join("Scripts").join("Example.pex");

    let now = SystemTime::now();
    touch_with_time(&pex_path, now - Duration::from_secs(60));
    touch_with_time(&script_path, now);

    let diagnostic = check(&script_path).expect("script is newer than its compiled output");

    assert_eq!(diagnostic.line, 1);
    assert_eq!(diagnostic.column, 1);
    assert_eq!(diagnostic.rule, RULE);
    assert!(diagnostic.message.starts_with("[info]"));
    assert!(diagnostic.message.contains(&pex_path.display().to_string()));
}

#[test]
fn does_not_flag_a_script_older_than_its_compiled_output() {
    let root = tempfile::tempdir().expect("failed to create temp dir");
    let source_dir = root.path().join("Scripts").join("Source");
    fs::create_dir_all(&source_dir).expect("failed to create source dir");
    let script_path = source_dir.join("Example.psc");
    let pex_path = root.path().join("Scripts").join("Example.pex");

    let now = SystemTime::now();
    touch_with_time(&script_path, now - Duration::from_secs(60));
    touch_with_time(&pex_path, now);

    assert!(check(&script_path).is_none());
}

#[test]
fn does_not_flag_a_script_with_the_same_modification_time_as_its_output() {
    let root = tempfile::tempdir().expect("failed to create temp dir");
    let source_dir = root.path().join("Scripts").join("Source");
    fs::create_dir_all(&source_dir).expect("failed to create source dir");
    let script_path = source_dir.join("Example.psc");
    let pex_path = root.path().join("Scripts").join("Example.pex");

    let now = SystemTime::now();
    touch_with_time(&script_path, now);
    touch_with_time(&pex_path, now);

    assert!(check(&script_path).is_none());
}

#[test]
fn ignores_a_script_with_no_compiled_output_yet() {
    let root = tempfile::tempdir().expect("failed to create temp dir");
    let source_dir = root.path().join("Scripts").join("Source");
    fs::create_dir_all(&source_dir).expect("failed to create source dir");
    let script_path = source_dir.join("Example.psc");
    fs::write(&script_path, "").expect("failed to write script");

    assert!(check(&script_path).is_none());
}

#[test]
fn ignores_a_missing_script_even_when_the_compiled_output_exists() {
    let root = tempfile::tempdir().expect("failed to create temp dir");
    let source_dir = root.path().join("Scripts").join("Source");
    fs::create_dir_all(&source_dir).expect("failed to create source dir");
    let script_path = source_dir.join("Example.psc");
    let pex_path = root.path().join("Scripts").join("Example.pex");
    fs::write(pex_path, "").expect("failed to write compiled output");

    assert!(check(&script_path).is_none());
}

#[test]
fn ignores_a_script_with_no_grandparent_directory() {
    let script_path = Path::new("Example.psc");

    assert!(check(script_path).is_none());
}

#[test]
fn pex_path_for_uses_the_source_directorys_parent() {
    let script_path = Path::new("/game/Data/Scripts/Source/Example.psc");

    assert_eq!(
        pex_path_for(script_path),
        Some(std::path::PathBuf::from("/game/Data/Scripts/Example.pex"))
    );
}

#[test]
fn pex_path_for_replaces_only_the_final_source_extension() {
    let script_path = Path::new("/game/Data/Scripts/Source/Quest.v2.PSC");

    assert_eq!(
        pex_path_for(script_path),
        Some(std::path::PathBuf::from("/game/Data/Scripts/Quest.v2.pex"))
    );
}

#[test]
fn pex_path_for_rejects_a_path_without_a_file_name() {
    assert_eq!(pex_path_for(Path::new("/")), None);
}
