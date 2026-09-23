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
//! [`crate::script_locator`]) and `<output dir>` is its parent, matching
//! the layout Bethesda's tooling expects: a `Source` directory holding
//! `.psc` files sits inside the `Scripts` directory that receives the
//! compiled `.pex` output.
//!
//! `-i` accepts multiple import directories separated by `;`, so it's
//! always given both of [`crate::script_locator`]'s known source
//! directories under the project root, not just the one the script being
//! compiled happens to live in — letting it import from either layout —
//! plus any of the project's configured `additional_script_roots` (see
//! [`papyrus_lint_config::load_script_roots`]), so a script that
//! imports from a shared library location outside those two conventional
//! directories still compiles. The compiler is run with its own containing
//! directory as the working directory, so it can resolve the target game's
//! bundled flags file named by the trailing `-f` argument via its relative
//! path.

use std::path::{Path, PathBuf};
use std::process::Command;

use serde::Serialize;

use crate::pex_header;

/// Returns the compiler flags file shipped for `game`.
///
/// Keeping this selection exhaustive means adding another [`papyrus_lints::Game`]
/// cannot silently reuse Skyrim's flags.
fn flags_file(game: papyrus_lints::Game) -> &'static str {
    match game {
        papyrus_lints::Game::Skyrim => "TESV_Papyrus_Flags.flg",
        papyrus_lints::Game::Fallout4 => "Institute_Papyrus_Flags.flg",
        papyrus_lints::Game::Starfield => panic!("Starfield is not supported yet"),
    }
}

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
/// (see [`crate::script_locator::CANDIDATE_DIRS`]) plus
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

    crate::script_locator::CANDIDATE_DIRS
        .iter()
        .map(|dir| root.join(dir))
        .chain(crate::script_locator::resolve_additional_roots(
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
/// the flags file selected for `game` (distributed beside
/// `PapyrusCompiler.exe`) that the trailing `-f` argument names by its
/// relative path. `personal_data_stripped` is always `false` on the
/// returned [`CompileOutcome`] — stripping (when wanted) is the caller's
/// job, since it depends on where the `.pex` actually landed.
fn run_compiler(
    game: papyrus_lints::Game,
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
        .arg(format!("-f={}", flags_file(game)));

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
/// at `compiler_path` and the flags file selected for `game`.
/// `additional_roots` are the project's configured
/// `additional_script_roots` (see
/// [`papyrus_lint_config::load_script_roots`]), included in the `-i`
/// argument alongside the two conventional source directories.
///
/// Returns `Err` when the compiler process itself couldn't be run or its
/// arguments couldn't be determined (executable missing or not
/// executable, `script_path` lacking a source/output directory to derive
/// `-i`/`-o` from, etc.); see [`CompileOutcome`] for how an actual compile
/// failure is reported instead.
pub fn compile_psc_file(
    game: papyrus_lints::Game,
    compiler_path: &Path,
    script_path: &Path,
    additional_roots: &[String],
) -> Result<CompileOutcome, String> {
    let (source_dir, output_dir) = resolve_locations(script_path)?;
    let import_dirs = import_dirs(source_dir, output_dir.parent(), additional_roots);

    let mut outcome = run_compiler(game, compiler_path, script_path, &import_dirs, &output_dir)?;
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
    game: papyrus_lints::Game,
    compiler_path: &Path,
    script_path: &Path,
    additional_roots: &[String],
) -> Result<CompileOutcome, String> {
    let (source_dir, output_dir) = resolve_locations(script_path)?;
    let import_dirs = import_dirs(source_dir, output_dir.parent(), additional_roots);

    let temp_dir = tempfile::tempdir()
        .map_err(|err| format!("failed to create a temporary output directory: {err}"))?;
    run_compiler(
        game,
        compiler_path,
        script_path,
        &import_dirs,
        temp_dir.path(),
    )
}

#[cfg(test)]
#[path = "compiler_tests.rs"]
mod tests;
