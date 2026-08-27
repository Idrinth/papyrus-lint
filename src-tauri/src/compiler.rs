//! Runs `PapyrusCompile.exe` against a single `.psc` script, reproducing
//! the invocation Creation Kit tooling uses to compile one script out of
//! its source directory:
//!
//! ```text
//! PapyrusCompile.exe "<source dir>" -f="<script name>.psc" -i="<source dir>" -o="<output dir>"
//! ```
//!
//! `<source dir>` is the directory the `.psc` file lives in (conventionally
//! `scripts/source` or `source/scripts` under a project's root — see
//! [`crate::script_locator`]) and `<output dir>` is its parent, matching
//! the layout Bethesda's tooling expects: a `Source` directory holding
//! `.psc` files sits inside the `Scripts` directory that receives the
//! compiled `.pex` output.

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

    let output = Command::new(compiler_path)
        .arg(source_dir)
        .arg(format!("-f={}", file_name.to_string_lossy()))
        .arg(format!("-i={}", source_dir.display()))
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
            "{}\n-f=AchievementInjector.psc\n-i={}\n-o={}\n",
            source_dir.display(),
            source_dir.display(),
            output_dir.display(),
        );
        assert_eq!(outcome.stdout, expected);
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

        assert!(result.is_err());
    }

    #[test]
    fn errors_when_script_path_has_no_parent_directory() {
        let compiler_path = Path::new("compiler");
        let script_path = Path::new("Foo.psc");

        let result = compile_psc_file(compiler_path, script_path);

        assert!(result.is_err());
    }
}
