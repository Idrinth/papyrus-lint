//! Runs `PapyrusCompiler.exe` against a single `.psc` script, reproducing
//! the invocation Creation Kit tooling uses to compile one script out of
//! its source directory:
//!
//! ```text
//! PapyrusCompiler.exe "<script path>" -i="<source dir 1>;<source dir 2>" -o="<output dir>" -f="TESV_Papyrus_Flags.flg"
//! ```
//!
//! `<script path>` names the `.psc` file being compiled. Its parent is
//! conventionally `scripts/source` or `source/scripts` under a project's root — see
//! [`papyrus_lint_core::script_locator`]) and `<output dir>` is its parent, matching
//! the layout Bethesda's tooling expects: a `Source` directory holding
//! `.psc` files sits inside the `Scripts` directory that receives the
//! compiled `.pex` output.
//!
//! `-i` accepts multiple import directories separated by `;`, so it's
//! always given both of [`papyrus_lint_core::script_locator`]'s known source
//! directories under the project root, not just the one the script being
//! compiled happens to live in — letting it import from either layout —
//! plus any of the project's configured `additional_script_roots` (see
//! [`papyrus_lint_core::config::load_script_roots`]), so a script that
//! imports from a shared library location outside those two conventional
//! directories still compiles. The compiler is run with its own containing
//! directory as the working directory, so it can resolve the bundled
//! `TESV_Papyrus_Flags.flg` the trailing `-f` argument names by its
//! relative path.

use std::path::{Path, PathBuf};
use std::process::Command;

use serde::Serialize;

use crate::pex_header;

/// The result of running the compiler against a script.
///
/// A script that fails to *compile* (a syntax error, a missing import,
/// etc.) is still represented as `Ok` with `success: false` — the process
/// ran and reported the failure, which is the normal case a caller needs
/// to display, not an error running the compiler itself. See
/// [`compile_psc_file`]'s `Err` cases for the difference.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct CompileOutcome {
    pub success: bool,
    pub stdout: String,
    pub stderr: String,
    /// Whether the compiling machine's Windows username/computer name
    /// (which `PapyrusCompiler.exe` embeds in every `.pex` it writes) was
    /// found and stripped from the compiled output. Always `false` when
    /// `success` is `false`, since there's no `.pex` to clean.
    pub personal_data_stripped: bool,
}

/// Strips the compiling machine's username/computer name (see
/// [`pex_header::strip_personal_data`]) from the `.pex` file compiled from
/// `script_path` into `output_dir`, rewriting it in place. Returns
/// `false`, without error, if the `.pex` file can't be found/read, doesn't
/// look like a `.pex` header, or already has no personal data to strip —
/// none of which should fail an otherwise-successful compile.
fn strip_pex_personal_data(script_path: &Path, output_dir: &Path) -> bool {
    let Some(stem) = script_path.file_stem() else {
        return false;
    };
    let pex_path = output_dir.join(stem).with_extension("pex");

    let Ok(bytes) = std::fs::read(&pex_path) else {
        return false;
    };
    let Some(patched) = pex_header::strip_personal_data(&bytes) else {
        return false;
    };

    std::fs::write(&pex_path, patched).is_ok()
}

/// Resolves the source directory (`script_path`'s parent) and the
/// conventional output directory (that source directory's own parent —
/// the project's real `Scripts` directory, matching the layout described
/// at the top of this module).
///
/// The conventional output directory is always used to resolve the
/// project root for the `-i` argument (see [`import_dirs`]), even for
/// [`check_psc_file`], which actually writes its compiled output
/// elsewhere — the root two levels above `source_dir` doesn't depend on
/// where the compiled `.pex` ends up.
fn resolve_locations(script_path: &Path) -> Result<(&Path, PathBuf), String> {
    let source_dir = script_path
        .parent()
        .filter(|dir| !dir.as_os_str().is_empty())
        .ok_or_else(|| {
            format!(
                "could not determine the source directory of {}",
                script_path.display()
            )
        })?;
    let output_dir = source_dir
        .parent()
        .ok_or_else(|| {
            format!(
                "could not determine an output directory above {}",
                source_dir.display()
            )
        })?
        .to_path_buf();
    Ok((source_dir, output_dir))
}

/// Builds the `-i` argument's value: `root`'s two known source directories
/// (see [`papyrus_lint_core::script_locator::CANDIDATE_DIRS`]) plus
/// `additional_roots` (the project's configured `additional_script_roots`,
/// resolved relative to `root` unless already absolute), joined with `;`
/// as PapyrusCompiler.exe expects for multiple import directories, so a
/// script can import from either conventional layout, or a configured
/// additional root, regardless of which one it lives in. Falls back to
/// `source_dir` alone if `root` is `None` (the project root two levels
/// above `source_dir` couldn't be determined).
fn import_dirs(source_dir: &Path, root: Option<&Path>, additional_roots: &[String]) -> String {
    let Some(root) = root else {
        return source_dir.display().to_string();
    };

    papyrus_lint_core::script_locator::CANDIDATE_DIRS
        .iter()
        .map(|dir| root.join(dir))
        .chain(papyrus_lint_core::script_locator::resolve_additional_roots(
            root,
            additional_roots,
        ))
        .map(|dir| dir.display().to_string())
        .collect::<Vec<_>>()
        .join(";")
}

/// Runs `compiler_path` against `script_path`, searching `import_dirs` and
/// writing whatever it compiles into `output_dir`. Run with the compiler
/// executable's own directory as the working directory, so it can resolve
/// the bundled `TESV_Papyrus_Flags.flg` (distributed beside
/// `PapyrusCompiler.exe`) that the fixed trailing `-f` argument names by
/// its relative path. `personal_data_stripped` is always `false` on the
/// returned [`CompileOutcome`] — stripping (when wanted) is the caller's
/// job, since it depends on where the `.pex` actually landed.
fn run_compiler(
    compiler_path: &Path,
    script_path: &Path,
    import_dirs: &str,
    output_dir: &Path,
) -> Result<CompileOutcome, String> {
    let mut command = Command::new(compiler_path);
    command
        .arg(script_path)
        .arg(format!("-i={import_dirs}"))
        .arg(format!("-o={}", output_dir.display()))
        .arg("-f=TESV_Papyrus_Flags.flg");

    if let Some(compiler_dir) = compiler_path
        .parent()
        .filter(|dir| !dir.as_os_str().is_empty())
    {
        command.current_dir(compiler_dir);
    }

    let output = command
        .output()
        .map_err(|err| format!("failed to run {}: {err}", compiler_path.display()))?;

    Ok(CompileOutcome {
        success: output.status.success(),
        stdout: String::from_utf8_lossy(&output.stdout).into_owned(),
        stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
        personal_data_stripped: false,
    })
}

/// Compiles the `.psc` file at `script_path` using the compiler executable
/// at `compiler_path`. `additional_roots` are the project's configured
/// `additional_script_roots` (see
/// [`papyrus_lint_core::config::load_script_roots`]), included in the `-i`
/// argument alongside the two conventional source directories.
///
/// Returns `Err` when the compiler process itself couldn't be run or its
/// arguments couldn't be determined (executable missing or not
/// executable, `script_path` lacking a source/output directory to derive
/// `-i`/`-o` from, etc.); see [`CompileOutcome`] for how an actual compile
/// failure is reported instead.
pub fn compile_psc_file(
    compiler_path: &Path,
    script_path: &Path,
    additional_roots: &[String],
) -> Result<CompileOutcome, String> {
    let (source_dir, output_dir) = resolve_locations(script_path)?;
    let import_dirs = import_dirs(source_dir, output_dir.parent(), additional_roots);

    let mut outcome = run_compiler(compiler_path, script_path, &import_dirs, &output_dir)?;
    outcome.personal_data_stripped =
        outcome.success && strip_pex_personal_data(script_path, &output_dir);
    Ok(outcome)
}

/// Compiles `script_path` the same way [`compile_psc_file`] does, but
/// writes the compiled `.pex` to a throwaway temporary directory (removed
/// again before returning) instead of the project's real `Scripts` output
/// directory. Used to run PapyrusCompiler.exe as a syntax check during
/// linting — see `compile_diagnostics` — without touching (or requiring
/// write access to) the project's actual compiled output, or leaving a
/// personal-data-bearing `.pex` behind for a script that was only being
/// checked, not deliberately compiled. Since the compiled output is
/// discarded either way, `personal_data_stripped` is always `false` on the
/// returned [`CompileOutcome`], unlike [`compile_psc_file`].
pub fn check_psc_file(
    compiler_path: &Path,
    script_path: &Path,
    additional_roots: &[String],
) -> Result<CompileOutcome, String> {
    let (source_dir, output_dir) = resolve_locations(script_path)?;
    let import_dirs = import_dirs(source_dir, output_dir.parent(), additional_roots);

    let temp_dir = tempfile::tempdir()
        .map_err(|err| format!("failed to create a temporary output directory: {err}"))?;
    run_compiler(compiler_path, script_path, &import_dirs, temp_dir.path())
}

#[cfg(test)]
mod tests {
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
        for attempt in 0.. {
            match compile_psc_file(compiler_path, script_path, &[]) {
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
        let compiler_path =
            write_stub_compiler(root.path(), "#!/bin/sh\necho compiled ok\nexit 0\n");

        let outcome =
            compile_stub_with_retry(&compiler_path, &script_path).expect("should succeed");

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
        let compiler_path =
            write_stub_compiler(root.path(), "#!/bin/sh\necho compiled ok\nexit 0\n");

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

        let outcome =
            compile_stub_with_retry(&compiler_path, &script_path).expect("should succeed");

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

        let outcome =
            compile_stub_with_retry(&compiler_path, &script_path).expect("should succeed");

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
    fn runs_from_the_compiler_directory() {
        let root = tempfile::tempdir().expect("failed to create temp dir");
        let compiler_dir = root.path().join("Papyrus Compiler");
        let source_dir = root.path().join("Data/Scripts/Source");
        fs::create_dir_all(&compiler_dir).expect("failed to create compiler dir");
        fs::create_dir_all(&source_dir).expect("failed to create source dir");
        let script_path = source_dir.join("Example.psc");
        fs::write(&script_path, "").expect("failed to write stub script");
        let compiler_path = write_stub_compiler(&compiler_dir, "#!/bin/sh\npwd\n");

        let outcome =
            compile_stub_with_retry(&compiler_path, &script_path).expect("should succeed");

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
        let root = Path::new("/game/Data");
        let source_dir = root.join("scripts/source");

        let dirs = import_dirs(
            &source_dir,
            Some(root),
            &[
                "../SharedScripts".to_string(),
                "/abs/OtherScripts".to_string(),
            ],
        );

        assert_eq!(
            dirs,
            format!(
                "{};{};{};{}",
                root.join("scripts/source").display(),
                root.join("source/scripts").display(),
                root.join("../SharedScripts").display(),
                Path::new("/abs/OtherScripts").display(),
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
    fn errors_when_compiler_cannot_be_run() {
        let root = tempfile::tempdir().expect("failed to create temp dir");
        let source_dir = root.path().join("Scripts").join("Source");
        fs::create_dir_all(&source_dir).expect("failed to create source dir");
        let script_path = source_dir.join("AchievementInjector.psc");
        fs::write(&script_path, "").expect("failed to write stub script");
        let missing_compiler = root.path().join("does-not-exist.exe");

        let result = compile_psc_file(&missing_compiler, &script_path, &[]);

        assert!(result
            .unwrap_err()
            .contains(&format!("failed to run {}", missing_compiler.display())));
    }

    #[test]
    fn errors_when_script_path_has_no_parent_directory() {
        let compiler_path = Path::new("compiler");
        let script_path = Path::new("Foo.psc");

        let result = compile_psc_file(compiler_path, script_path, &[]);

        assert_eq!(
            result.unwrap_err(),
            "could not determine the source directory of Foo.psc"
        );
    }

    #[test]
    fn errors_when_source_directory_has_no_parent_output_directory() {
        let result = compile_psc_file(Path::new("compiler"), Path::new("/Foo.psc"), &[]);

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
            match check_psc_file(compiler_path, script_path, &[]) {
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

        let outcome =
            check_stub_with_retry(&compiler_path, &script_path).expect("should still be Ok");

        assert!(!outcome.success);
        assert_eq!(outcome.stderr.trim(), "compilation failed");
        assert!(!outcome.personal_data_stripped);
    }

    #[test]
    fn check_psc_file_errors_when_script_path_has_no_parent_directory() {
        let result = check_psc_file(Path::new("compiler"), Path::new("Foo.psc"), &[]);

        assert_eq!(
            result.unwrap_err(),
            "could not determine the source directory of Foo.psc"
        );
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
}
