//! Runs `PapyrusCompiler.exe` against a single `.psc` script, reproducing
//! the invocation Creation Kit tooling uses to compile one script out of
//! its source directory:
//!
//! ```text
//! PapyrusCompiler.exe "<source dir>" -f="<script name>.psc" -i="<source dir 1>;<source dir 2>" -o="<output dir>"
//! ```
//!
//! `<source dir>` is the directory the `.psc` file lives in (conventionally
//! `scripts/source` or `source/scripts` under a project's root — see
//! [`papyrus_lint_core::script_locator`]) and `<output dir>` is its parent, matching
//! the layout Bethesda's tooling expects: a `Source` directory holding
//! `.psc` files sits inside the `Scripts` directory that receives the
//! compiled `.pex` output.
//!
//! `-i` accepts multiple import directories separated by `;`, so it's
//! always given both of [`papyrus_lint_core::script_locator`]'s known source
//! directories under the project root, not just the one the script being
//! compiled happens to live in — letting it import from either layout.

use std::path::Path;
use std::process::Command;

use serde::Serialize;

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
}

/// Builds the `-i` argument's value: the project root's two known source
/// directories (see [`papyrus_lint_core::script_locator::CANDIDATE_DIRS`]), joined with
/// `;` as PapyrusCompiler.exe expects for multiple import directories, so a
/// script can import from either layout regardless of which one it lives
/// in. Falls back to `source_dir` alone if the project root (two levels
/// above `source_dir`, i.e. `output_dir`'s parent) can't be determined.
fn import_dirs(source_dir: &Path, output_dir: &Path) -> String {
    let Some(root) = output_dir.parent() else {
        return source_dir.display().to_string();
    };

    papyrus_lint_core::script_locator::CANDIDATE_DIRS
        .iter()
        .map(|dir| root.join(dir).display().to_string())
        .collect::<Vec<_>>()
        .join(";")
}

/// Compiles the `.psc` file at `script_path` using the compiler executable
/// at `compiler_path`.
///
/// Returns `Err` when the compiler process itself couldn't be run or its
/// arguments couldn't be determined (executable missing or not
/// executable, `script_path` lacking a source/output directory to derive
/// `-i`/`-o` from, etc.); see [`CompileOutcome`] for how an actual compile
/// failure is reported instead.
pub fn compile_psc_file(
    compiler_path: &Path,
    script_path: &Path,
) -> Result<CompileOutcome, String> {
    let source_dir = script_path
        .parent()
        .filter(|dir| !dir.as_os_str().is_empty())
        .ok_or_else(|| {
            format!(
                "could not determine the source directory of {}",
                script_path.display()
            )
        })?;
    let output_dir = source_dir.parent().ok_or_else(|| {
        format!(
            "could not determine an output directory above {}",
            source_dir.display()
        )
    })?;
    let file_name = script_path
        .file_name()
        .ok_or_else(|| format!("{} has no file name", script_path.display()))?;
    let import_dirs = import_dirs(source_dir, output_dir);

    let output = Command::new(compiler_path)
        .arg(source_dir)
        .arg(format!("-f={}", file_name.to_string_lossy()))
        .arg(format!("-i={import_dirs}"))
        .arg(format!("-o={}", output_dir.display()))
        .output()
        .map_err(|err| format!("failed to run {}: {err}", compiler_path.display()))?;

    Ok(CompileOutcome {
        success: output.status.success(),
        stdout: String::from_utf8_lossy(&output.stdout).into_owned(),
        stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
    })
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

        let outcome = compile_psc_file(&compiler_path, &script_path).expect("should succeed");

        assert!(outcome.success);
        assert_eq!(outcome.stdout.trim(), "compiled ok");
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

        let outcome = compile_psc_file(&compiler_path, &script_path).expect("should still be Ok");

        assert!(!outcome.success);
        assert_eq!(outcome.stderr.trim(), "compilation failed");
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

        let outcome = compile_psc_file(&compiler_path, &script_path).expect("should succeed");

        let output_dir = root.path().join("Scripts");
        let expected = format!(
            "{}\n-f=AchievementInjector.psc\n-i={};{}\n-o={}\n",
            source_dir.display(),
            root.path().join("scripts/source").display(),
            root.path().join("source/scripts").display(),
            output_dir.display(),
        );
        assert_eq!(outcome.stdout, expected);
    }

    #[test]
    fn import_dirs_joins_both_known_source_dirs_with_semicolon() {
        let root = Path::new("/game/Data");
        let source_dir = root.join("scripts/source");
        let output_dir = source_dir.parent().expect("has a parent");

        let dirs = import_dirs(&source_dir, output_dir);

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
    fn import_dirs_falls_back_to_source_dir_without_a_root() {
        let source_dir = Path::new("source");
        let output_dir = Path::new("");

        let dirs = import_dirs(source_dir, output_dir);

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

        let result = compile_psc_file(&missing_compiler, &script_path);

        assert!(result
            .unwrap_err()
            .contains(&format!("failed to run {}", missing_compiler.display())));
    }

    #[test]
    fn errors_when_script_path_has_no_parent_directory() {
        let compiler_path = Path::new("compiler");
        let script_path = Path::new("Foo.psc");

        let result = compile_psc_file(compiler_path, script_path);

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

        let outcome = compile_psc_file(&compiler_path, &script_path).expect("should run");

        assert!(outcome.success);
        assert_eq!(outcome.stdout, "�stdout");
        assert_eq!(outcome.stderr, "�stderr");
    }
}
