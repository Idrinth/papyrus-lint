use super::*;
use std::fs;

#[cfg(unix)]
fn write_stub_compiler(dir: &Path, script: &str) -> std::path::PathBuf {
    use std::os::unix::fs::PermissionsExt;

    let path = dir.join("stub-compiler.sh");
    fs::write(&path, script).expect("failed to write stub compiler");
    fs::set_permissions(&path, fs::Permissions::from_mode(0o755))
        .expect("failed to make stub compiler executable");
    path
}

/// Executing a script file immediately after writing and chmod'ing it
/// (as every test here does with its stub compiler) occasionally hits
/// `ETXTBSY`/"Text file busy" under CI's tmpfs when many tests spawn
/// processes concurrently, even though the file's own write handle has
/// already been closed. Retries a couple of times before giving up,
/// since that's an environmental race unrelated to what these tests
/// actually check.
#[cfg(unix)]
fn compile_stub_with_retry(
    compiler_path: &Path,
    script_path: &Path,
) -> Result<CompileOutcome, String> {
    compile_stub_with_retry_and_roots(compiler_path, script_path, &[])
}

/// Like [`compile_stub_with_retry`], but for a test that also needs to
/// pass `additional_roots` through to [`compile_psc_file`].
#[cfg(unix)]
fn compile_stub_with_retry_and_roots(
    compiler_path: &Path,
    script_path: &Path,
    roots: &[String],
) -> Result<CompileOutcome, String> {
    for attempt in 0.. {
        match compile_psc_file(
            papyrus_lints::Game::Skyrim,
            compiler_path,
            script_path,
            roots,
        ) {
            Err(err) if attempt < 5 && err.contains("Text file busy") => {
                std::thread::sleep(std::time::Duration::from_millis(20));
            }
            result => return result,
        }
    }
    unreachable!()
}

#[test]
#[cfg(unix)]
fn success_reports_captured_stdout() {
    let root = tempfile::tempdir().expect("failed to create temp dir");
    let source_dir = root.path().join("Scripts").join("Source");
    fs::create_dir_all(&source_dir).expect("failed to create source dir");
    let script_path = source_dir.join("AchievementInjector.psc");
    fs::write(&script_path, "").expect("failed to write stub script");
    let compiler_path = write_stub_compiler(root.path(), "#!/bin/sh\necho compiled ok\nexit 0\n");

    let outcome = compile_stub_with_retry(&compiler_path, &script_path).expect("should succeed");

    assert!(outcome.success);
    assert_eq!(outcome.stdout.trim(), "compiled ok");
    assert!(!outcome.personal_data_stripped);
}

#[test]
#[cfg(unix)]
fn success_strips_personal_data_from_the_compiled_pex() {
    let root = tempfile::tempdir().expect("failed to create temp dir");
    let source_dir = root.path().join("Scripts").join("Source");
    fs::create_dir_all(&source_dir).expect("failed to create source dir");
    let script_path = source_dir.join("AchievementInjector.psc");
    fs::write(&script_path, "").expect("failed to write stub script");
    let compiler_path = write_stub_compiler(root.path(), "#!/bin/sh\necho compiled ok\nexit 0\n");

    // Simulate PapyrusCompiler.exe having already dropped a compiled
    // .pex (embedding personal data) next to the stub's own stdout.
    let pex_path = root.path().join("Scripts").join("AchievementInjector.pex");
    let mut pex_bytes = vec![0xFA, 0x57, 0xC0, 0xDE, 3, 9];
    pex_bytes.extend_from_slice(&1u16.to_be_bytes());
    pex_bytes.extend_from_slice(&0u64.to_be_bytes());
    for s in ["AchievementInjector.psc", "SomeUser", "SOME-PC"] {
        pex_bytes.extend_from_slice(&(s.len() as u16).to_be_bytes());
        pex_bytes.extend_from_slice(s.as_bytes());
    }
    fs::write(&pex_path, &pex_bytes).expect("failed to write stub pex");

    let outcome = compile_stub_with_retry(&compiler_path, &script_path).expect("should succeed");

    assert!(outcome.success);
    assert!(outcome.personal_data_stripped);
    let patched = fs::read(&pex_path).expect("pex should still exist");
    assert!(!contains_bytes(&patched, b"SomeUser"));
    assert!(!contains_bytes(&patched, b"SOME-PC"));
}

fn contains_bytes(haystack: &[u8], needle: &[u8]) -> bool {
    haystack.windows(needle.len()).any(|w| w == needle)
}

#[test]
#[cfg(unix)]
fn failure_is_reported_as_ok_with_success_false() {
    let root = tempfile::tempdir().expect("failed to create temp dir");
    let source_dir = root.path().join("Scripts").join("Source");
    fs::create_dir_all(&source_dir).expect("failed to create source dir");
    let script_path = source_dir.join("Broken.psc");
    fs::write(&script_path, "").expect("failed to write stub script");
    let compiler_path = write_stub_compiler(
        root.path(),
        "#!/bin/sh\necho compilation failed >&2\nexit 1\n",
    );

    let outcome =
        compile_stub_with_retry(&compiler_path, &script_path).expect("should still be Ok");

    assert!(!outcome.success);
    assert_eq!(outcome.stderr.trim(), "compilation failed");
    assert!(!outcome.personal_data_stripped);
}

#[test]
#[cfg(unix)]
fn failed_compile_does_not_modify_an_existing_pex() {
    let root = tempfile::tempdir().expect("failed to create temp dir");
    let source_dir = root.path().join("Scripts").join("Source");
    fs::create_dir_all(&source_dir).expect("failed to create source dir");
    let script_path = source_dir.join("Broken.psc");
    fs::write(&script_path, "").expect("failed to write stub script");
    let compiler_path = write_stub_compiler(root.path(), "#!/bin/sh\nexit 1\n");

    let pex_path = root.path().join("Scripts").join("Broken.pex");
    let mut pex_bytes = vec![0xFA, 0x57, 0xC0, 0xDE, 3, 9];
    pex_bytes.extend_from_slice(&1u16.to_be_bytes());
    pex_bytes.extend_from_slice(&0u64.to_be_bytes());
    for s in ["Broken.psc", "SomeUser", "SOME-PC"] {
        pex_bytes.extend_from_slice(&(s.len() as u16).to_be_bytes());
        pex_bytes.extend_from_slice(s.as_bytes());
    }
    fs::write(&pex_path, &pex_bytes).expect("failed to write existing pex");

    let outcome =
        compile_stub_with_retry(&compiler_path, &script_path).expect("compiler should run");

    assert!(!outcome.success);
    assert!(!outcome.personal_data_stripped);
    assert_eq!(
        fs::read(pex_path).expect("pex should remain readable"),
        pex_bytes
    );
}

#[test]
#[cfg(unix)]
fn successful_compile_ignores_an_invalid_pex() {
    let root = tempfile::tempdir().expect("failed to create temp dir");
    let source_dir = root.path().join("Scripts").join("Source");
    fs::create_dir_all(&source_dir).expect("failed to create source dir");
    let script_path = source_dir.join("Example.psc");
    fs::write(&script_path, "").expect("failed to write stub script");
    let compiler_path = write_stub_compiler(root.path(), "#!/bin/sh\nexit 0\n");
    let pex_path = root.path().join("Scripts").join("Example.pex");
    fs::write(&pex_path, b"not a pex").expect("failed to write invalid pex");

    let outcome =
        compile_stub_with_retry(&compiler_path, &script_path).expect("compiler should run");

    assert!(outcome.success);
    assert!(!outcome.personal_data_stripped);
    assert_eq!(
        fs::read(pex_path).expect("pex should remain readable"),
        b"not a pex"
    );
}

#[test]
#[cfg(unix)]
fn successful_compile_leaves_an_already_sanitized_pex_unchanged() {
    let root = tempfile::tempdir().expect("failed to create temp dir");
    let source_dir = root.path().join("Scripts").join("Source");
    fs::create_dir_all(&source_dir).expect("failed to create source dir");
    let script_path = source_dir.join("Example.psc");
    fs::write(&script_path, "").expect("failed to write stub script");
    let compiler_path = write_stub_compiler(root.path(), "#!/bin/sh\nexit 0\n");
    let pex_path = root.path().join("Scripts").join("Example.pex");
    let mut pex_bytes = vec![0xFA, 0x57, 0xC0, 0xDE, 3, 9];
    pex_bytes.extend_from_slice(&1u16.to_be_bytes());
    pex_bytes.extend_from_slice(&0u64.to_be_bytes());
    for value in ["Example.psc", "", ""] {
        pex_bytes.extend_from_slice(&(value.len() as u16).to_be_bytes());
        pex_bytes.extend_from_slice(value.as_bytes());
    }
    fs::write(&pex_path, &pex_bytes).expect("failed to write sanitized pex");

    let outcome =
        compile_stub_with_retry(&compiler_path, &script_path).expect("compiler should run");

    assert!(outcome.success);
    assert!(!outcome.personal_data_stripped);
    assert_eq!(
        fs::read(pex_path).expect("pex should remain readable"),
        pex_bytes
    );
}

#[test]
#[cfg(unix)]
fn successful_compile_without_a_pex_does_not_report_stripping() {
    let root = tempfile::tempdir().expect("failed to create temp dir");
    let source_dir = root.path().join("Scripts").join("Source");
    fs::create_dir_all(&source_dir).expect("failed to create source dir");
    let script_path = source_dir.join("Example.psc");
    fs::write(&script_path, "").expect("failed to write stub script");
    let compiler_path = write_stub_compiler(root.path(), "#!/bin/sh\nexit 0\n");

    let outcome =
        compile_stub_with_retry(&compiler_path, &script_path).expect("compiler should run");

    assert!(outcome.success);
    assert!(!outcome.personal_data_stripped);
    assert!(!root.path().join("Scripts/Example.pex").exists());
}

#[test]
#[cfg(unix)]
fn passes_expected_arguments() {
    let root = tempfile::tempdir().expect("failed to create temp dir");
    let source_dir = root.path().join("Scripts").join("Source");
    fs::create_dir_all(&source_dir).expect("failed to create source dir");
    let script_path = source_dir.join("AchievementInjector.psc");
    fs::write(&script_path, "").expect("failed to write stub script");
    let compiler_path = write_stub_compiler(
        root.path(),
        "#!/bin/sh\nfor arg in \"$@\"; do echo \"$arg\"; done\n",
    );

    let outcome = compile_stub_with_retry(&compiler_path, &script_path).expect("should succeed");

    let output_dir = root.path().join("Scripts");
    let expected = format!(
        "{}\n-i={};{}\n-o={}\n-f=TESV_Papyrus_Flags.flg\n",
        script_path.display(),
        root.path().join("scripts/source").display(),
        root.path().join("source/scripts").display(),
        output_dir.display(),
    );
    assert_eq!(outcome.stdout, expected);
}

#[test]
#[cfg(unix)]
fn passes_relative_and_absolute_additional_import_roots() {
    let root = tempfile::tempdir().expect("failed to create temp dir");
    let source_dir = root.path().join("Scripts").join("Source");
    fs::create_dir_all(&source_dir).expect("failed to create source dir");
    let script_path = source_dir.join("Example.psc");
    fs::write(&script_path, "").expect("failed to write stub script");
    let compiler_path = write_stub_compiler(
        root.path(),
        "#!/bin/sh\nfor arg in \"$@\"; do echo \"$arg\"; done\n",
    );
    fs::create_dir_all(root.path().join("Shared/Source"))
        .expect("failed to create relative additional root");
    let absolute = tempfile::tempdir().expect("failed to create additional root");

    let outcome = compile_stub_with_retry_and_roots(
        &compiler_path,
        &script_path,
        &[
            "Shared/Source".to_string(),
            absolute.path().to_string_lossy().into_owned(),
        ],
    )
    .expect("compiler should run");

    let import_arg = outcome
        .stdout
        .lines()
        .find(|line| line.starts_with("-i="))
        .expect("stub should echo the import argument");
    assert_eq!(
        import_arg,
        format!(
            "-i={};{};{};{}",
            root.path().join("scripts/source").display(),
            root.path().join("source/scripts").display(),
            root.path().join("Shared/Source").display(),
            absolute.path().display(),
        )
    );
}

#[test]
#[cfg(unix)]
fn runs_from_the_compiler_directory() {
    let root = tempfile::tempdir().expect("failed to create temp dir");
    let compiler_dir = root.path().join("Papyrus Compiler");
    let source_dir = root.path().join("Data/Scripts/Source");
    fs::create_dir_all(&compiler_dir).expect("failed to create compiler dir");
    fs::create_dir_all(&source_dir).expect("failed to create source dir");
    let script_path = source_dir.join("Example.psc");
    fs::write(&script_path, "").expect("failed to write stub script");
    let compiler_path = write_stub_compiler(&compiler_dir, "#!/bin/sh\npwd\n");

    let outcome = compile_stub_with_retry(&compiler_path, &script_path).expect("should succeed");

    assert_eq!(outcome.stdout.trim(), compiler_dir.display().to_string());
}

#[test]
fn import_dirs_joins_both_known_source_dirs_with_semicolon() {
    let root = Path::new("/game/Data");
    let source_dir = root.join("scripts/source");

    let dirs = import_dirs(&source_dir, Some(root), &[]);

    assert_eq!(
        dirs,
        format!(
            "{};{}",
            root.join("scripts/source").display(),
            root.join("source/scripts").display(),
        )
    );
}

#[test]
fn import_dirs_appends_additional_script_roots() {
    let project = tempfile::tempdir().expect("failed to create project directory");
    let root = project.path().join("Data");
    let source_dir = root.join("scripts/source");
    let relative_root = root.join("../SharedScripts");
    let absolute_root = tempfile::tempdir().expect("failed to create absolute script root");
    fs::create_dir_all(&relative_root).expect("failed to create relative script root");

    let dirs = import_dirs(
        &source_dir,
        Some(&root),
        &[
            "../SharedScripts".to_string(),
            absolute_root.path().to_string_lossy().into_owned(),
        ],
    );

    assert_eq!(
        dirs,
        format!(
            "{};{};{};{}",
            root.join("scripts/source").display(),
            root.join("source/scripts").display(),
            relative_root.display(),
            absolute_root.path().display(),
        )
    );
}

#[test]
fn import_dirs_falls_back_to_source_dir_without_a_root() {
    let source_dir = Path::new("source");

    let dirs = import_dirs(source_dir, None, &[]);

    assert_eq!(dirs, source_dir.display().to_string());
}

#[test]
fn import_dirs_without_a_root_cannot_resolve_additional_roots() {
    let source_dir = Path::new("source");

    let dirs = import_dirs(
        source_dir,
        None,
        &["../shared".to_string(), "/absolute/shared".to_string()],
    );

    assert_eq!(dirs, source_dir.display().to_string());
}

#[test]
fn resolve_locations_supports_relative_project_paths() {
    let (source_dir, output_dir) = resolve_locations(Path::new("Data/Scripts/Source/Example.psc"))
        .expect("nested script path should resolve");

    assert_eq!(source_dir, Path::new("Data/Scripts/Source"));
    assert_eq!(output_dir, Path::new("Data/Scripts"));
}

#[test]
fn errors_when_compiler_cannot_be_run() {
    let root = tempfile::tempdir().expect("failed to create temp dir");
    let source_dir = root.path().join("Scripts").join("Source");
    fs::create_dir_all(&source_dir).expect("failed to create source dir");
    let script_path = source_dir.join("AchievementInjector.psc");
    fs::write(&script_path, "").expect("failed to write stub script");
    let missing_compiler = root.path().join("does-not-exist.exe");

    let result = compile_psc_file(
        papyrus_lints::Game::Skyrim,
        &missing_compiler,
        &script_path,
        &[],
    );

    assert!(result
        .unwrap_err()
        .contains(&format!("failed to run {}", missing_compiler.display())));
}

#[test]
fn errors_when_script_path_has_no_parent_directory() {
    let compiler_path = Path::new("compiler");
    let script_path = Path::new("Foo.psc");

    let result = compile_psc_file(papyrus_lints::Game::Skyrim, compiler_path, script_path, &[]);

    assert_eq!(
        result.unwrap_err(),
        "could not determine the source directory of Foo.psc"
    );
}

#[test]
fn errors_when_source_directory_has_no_parent_output_directory() {
    let result = compile_psc_file(
        papyrus_lints::Game::Skyrim,
        Path::new("compiler"),
        Path::new("/Foo.psc"),
        &[],
    );

    assert_eq!(
        result.unwrap_err(),
        "could not determine an output directory above /"
    );
}

#[test]
fn personal_data_stripping_skips_paths_without_a_file_stem() {
    assert!(!strip_pex_personal_data(Path::new("/"), Path::new("/tmp")));
}

/// See [`compile_stub_with_retry`]'s note above; the same race applies
/// to [`check_psc_file`].
#[cfg(unix)]
fn check_stub_with_retry(
    compiler_path: &Path,
    script_path: &Path,
) -> Result<CompileOutcome, String> {
    for attempt in 0.. {
        match check_psc_file(papyrus_lints::Game::Skyrim, compiler_path, script_path, &[]) {
            Err(err) if attempt < 5 && err.contains("Text file busy") => {
                std::thread::sleep(std::time::Duration::from_millis(20));
            }
            result => return result,
        }
    }
    unreachable!()
}

#[test]
#[cfg(unix)]
fn check_psc_file_never_writes_into_the_projects_real_output_directory() {
    let root = tempfile::tempdir().expect("failed to create temp dir");
    let source_dir = root.path().join("Scripts").join("Source");
    fs::create_dir_all(&source_dir).expect("failed to create source dir");
    let script_path = source_dir.join("Example.psc");
    fs::write(&script_path, "").expect("failed to write stub script");
    // Writes a .pex into whichever directory it's told to (-o), the
    // same way a real compile would, so this can confirm that
    // directory is a throwaway temp dir rather than the project's
    // real `Scripts` output directory.
    let compiler_path = write_stub_compiler(
        root.path(),
        "#!/bin/sh\nfor arg in \"$@\"; do\n  case \"$arg\" in\n    -o=*) out=\"${arg#-o=}\" ;;\n  esac\ndone\ntouch \"$out/Example.pex\"\nexit 0\n",
    );

    let outcome = check_stub_with_retry(&compiler_path, &script_path).expect("should succeed");

    assert!(outcome.success);
    // check_psc_file discards its compiled output, so it never
    // attempts (and never needs) personal-data stripping.
    assert!(!outcome.personal_data_stripped);
    assert!(!root.path().join("Scripts").join("Example.pex").exists());
}

#[test]
#[cfg(unix)]
fn check_psc_file_still_resolves_import_dirs_from_the_real_project_root() {
    let root = tempfile::tempdir().expect("failed to create temp dir");
    let source_dir = root.path().join("Scripts").join("Source");
    fs::create_dir_all(&source_dir).expect("failed to create source dir");
    let script_path = source_dir.join("AchievementInjector.psc");
    fs::write(&script_path, "").expect("failed to write stub script");
    let compiler_path = write_stub_compiler(
        root.path(),
        "#!/bin/sh\nfor arg in \"$@\"; do echo \"$arg\"; done\n",
    );

    let outcome = check_stub_with_retry(&compiler_path, &script_path).expect("should succeed");

    let printed_import_dirs = outcome
        .stdout
        .lines()
        .find_map(|line| line.strip_prefix("-i="))
        .expect("stub compiler should have echoed its -i argument");
    assert_eq!(
        printed_import_dirs,
        format!(
            "{};{}",
            root.path().join("scripts/source").display(),
            root.path().join("source/scripts").display(),
        )
    );

    let printed_output_dir = outcome
        .stdout
        .lines()
        .find_map(|line| line.strip_prefix("-o="))
        .expect("stub compiler should have echoed its -o argument");
    let real_output_dir = root.path().join("Scripts");
    assert_ne!(Path::new(printed_output_dir), real_output_dir);
}

#[test]
#[cfg(unix)]
fn check_psc_file_reports_a_failed_compile_as_ok_with_success_false() {
    let root = tempfile::tempdir().expect("failed to create temp dir");
    let source_dir = root.path().join("Scripts").join("Source");
    fs::create_dir_all(&source_dir).expect("failed to create source dir");
    let script_path = source_dir.join("Broken.psc");
    fs::write(&script_path, "").expect("failed to write stub script");
    let compiler_path = write_stub_compiler(
        root.path(),
        "#!/bin/sh\necho compilation failed >&2\nexit 1\n",
    );

    let outcome = check_stub_with_retry(&compiler_path, &script_path).expect("should still be Ok");

    assert!(!outcome.success);
    assert_eq!(outcome.stderr.trim(), "compilation failed");
    assert!(!outcome.personal_data_stripped);
}

#[test]
fn check_psc_file_errors_when_script_path_has_no_parent_directory() {
    let result = check_psc_file(
        papyrus_lints::Game::Skyrim,
        Path::new("compiler"),
        Path::new("Foo.psc"),
        &[],
    );

    assert_eq!(
        result.unwrap_err(),
        "could not determine the source directory of Foo.psc"
    );
}

#[test]
fn check_psc_file_errors_when_source_directory_has_no_parent_output_directory() {
    let result = check_psc_file(
        papyrus_lints::Game::Skyrim,
        Path::new("compiler"),
        Path::new("/Foo.psc"),
        &[],
    );

    assert_eq!(
        result.unwrap_err(),
        "could not determine an output directory above /"
    );
}

#[test]
fn check_psc_file_reports_when_the_compiler_cannot_be_started() {
    let root = tempfile::tempdir().expect("failed to create temp dir");
    let source_dir = root.path().join("Scripts/Source");
    fs::create_dir_all(&source_dir).expect("failed to create source dir");
    let script_path = source_dir.join("Example.psc");
    fs::write(&script_path, "ScriptName Example\n").expect("failed to write script");
    let missing_compiler = root.path().join("missing-compiler");

    let result = check_psc_file(
        papyrus_lints::Game::Skyrim,
        &missing_compiler,
        &script_path,
        &[],
    );

    assert!(result
        .unwrap_err()
        .contains(&format!("failed to run {}", missing_compiler.display())));
    assert!(!root.path().join("Scripts/Example.pex").exists());
}

#[test]
#[cfg(unix)]
fn captures_lossy_output_from_both_streams() {
    let root = tempfile::tempdir().expect("failed to create temp dir");
    let source_dir = root.path().join("Scripts").join("Source");
    fs::create_dir_all(&source_dir).expect("failed to create source dir");
    let script_path = source_dir.join("Example.psc");
    fs::write(&script_path, "").expect("failed to write stub script");
    let compiler_path = write_stub_compiler(
        root.path(),
        "#!/bin/sh\nprintf '\\377stdout'\nprintf '\\377stderr' >&2\n",
    );

    let outcome = compile_stub_with_retry(&compiler_path, &script_path).expect("should run");

    assert!(outcome.success);
    assert_eq!(outcome.stdout, "�stdout");
    assert_eq!(outcome.stderr, "�stderr");
}
