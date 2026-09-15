//! Public-API integration coverage for compiler checks and their conversion
//! into lint diagnostics.

use std::fs;
use std::path::{Path, PathBuf};

use papyrus_lint_core::compile_diagnostics::{parse_compile_errors, RULE};
use papyrus_lint_core::compiler::{check_psc_file, CompileOutcome};

#[cfg(unix)]
fn write_stub_compiler(directory: &Path, body: &str) -> PathBuf {
    use std::os::unix::fs::PermissionsExt;

    let path = directory.join("PapyrusCompiler");
    fs::write(&path, body).expect("failed to write compiler stub");
    fs::set_permissions(&path, fs::Permissions::from_mode(0o755))
        .expect("failed to make compiler stub executable");
    path
}

#[cfg(unix)]
fn check_with_retry(compiler: &Path, script: &Path) -> Result<CompileOutcome, String> {
    for attempt in 0.. {
        match check_psc_file(compiler, script, &[]) {
            Err(error) if attempt < 5 && error.contains("Text file busy") => {
                std::thread::sleep(std::time::Duration::from_millis(20));
            }
            result => return result,
        }
    }
    unreachable!()
}

#[test]
#[cfg(unix)]
fn failed_compiler_check_becomes_ordered_lint_diagnostics() {
    let project = tempfile::tempdir().expect("failed to create project directory");
    let source_dir = project.path().join("Data/Scripts/Source");
    fs::create_dir_all(&source_dir).expect("failed to create source directory");
    let script = source_dir.join("Broken.psc");
    fs::write(&script, "ScriptName Broken\n").expect("failed to write source fixture");
    let compiler = write_stub_compiler(
        project.path(),
        "#!/bin/sh\n\
         echo \"Broken.psc(4,7): unexpected token\"\n\
         echo \"compiler summary without a location\" >&2\n\
         echo \"<unknown>(0,0): unable to locate imported script\" >&2\n\
         exit 1\n",
    );

    let outcome = check_with_retry(&compiler, &script).expect("compiler stub should run");
    let diagnostics = parse_compile_errors(&outcome);

    assert!(!outcome.success);
    assert_eq!(diagnostics.len(), 2);
    assert_eq!(diagnostics[0].line, 4);
    assert_eq!(diagnostics[0].column, 7);
    assert_eq!(diagnostics[0].message, "[error] unexpected token");
    assert_eq!(diagnostics[1].line, 1);
    assert_eq!(diagnostics[1].column, 1);
    assert_eq!(
        diagnostics[1].message,
        "[error] unable to locate imported script"
    );
    assert!(diagnostics.iter().all(|diagnostic| diagnostic.rule == RULE));
    assert!(!project.path().join("Data/Scripts/Broken.pex").exists());
}

#[test]
#[cfg(unix)]
fn successful_compiler_check_discards_location_shaped_output() {
    let project = tempfile::tempdir().expect("failed to create project directory");
    let source_dir = project.path().join("Data/Scripts/Source");
    fs::create_dir_all(&source_dir).expect("failed to create source directory");
    let script = source_dir.join("Valid.psc");
    fs::write(&script, "ScriptName Valid\n").expect("failed to write source fixture");
    let compiler = write_stub_compiler(
        project.path(),
        "#!/bin/sh\necho \"Valid.psc(2,3): wrapper status message\"\nexit 0\n",
    );

    let outcome = check_with_retry(&compiler, &script).expect("compiler stub should run");

    assert!(outcome.success);
    assert!(!outcome.personal_data_stripped);
    assert!(parse_compile_errors(&outcome).is_empty());
}

#[test]
fn diagnostics_preserve_stdout_before_stderr_at_the_public_boundary() {
    let outcome = CompileOutcome {
        success: false,
        stdout: "First.psc(8,2): first failure\r\n".to_string(),
        stderr: "Second.psc(9,3): second failure\r\n".to_string(),
        personal_data_stripped: false,
    };

    let diagnostics = parse_compile_errors(&outcome);

    assert_eq!(
        diagnostics
            .iter()
            .map(|diagnostic| diagnostic.message.as_str())
            .collect::<Vec<_>>(),
        ["[error] first failure", "[error] second failure"]
    );
}
