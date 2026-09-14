//! Flags a `.psc` file whose compiled `.pex` output is older than the
//! source itself — usually a sign that someone edited the script and
//! forgot to recompile it before shipping/testing.
//!
//! The compiled `.pex` is looked for at the conventional location
//! [`crate::compiler`] itself compiles to: the source directory's own
//! parent (e.g. `Scripts/` for a script under `Scripts/Source/`), named
//! after the script's file stem. A script whose `.pex` can't be found
//! there at all (never compiled yet, or compiled somewhere else this
//! crate doesn't know about) is left unflagged rather than guessed at —
//! this only compares timestamps once both files are known to exist.

use std::path::Path;

use papyrus_lints::Diagnostic;

/// This diagnostic's [`Diagnostic::rule`] id, for `@disable` line comments
/// and the `rules.stale_compiled_output` config key.
pub const RULE: &str = "stale-compiled-output";

/// Resolves the conventional compiled `.pex` path for `script_path`: its
/// source directory's own parent, joined with the script's file stem and a
/// `.pex` extension — the same location [`crate::compiler::compile_psc_file`]
/// writes to. Returns `None` if `script_path` doesn't have both a parent
/// and a grandparent directory to derive that location from.
fn pex_path_for(script_path: &Path) -> Option<std::path::PathBuf> {
    let stem = script_path.file_stem()?;
    let output_dir = script_path.parent()?.parent()?;
    Some(output_dir.join(stem).with_extension("pex"))
}

/// Compares `script_path`'s last-modified time against its conventionally
/// located compiled `.pex` output (see [`pex_path_for`]), reporting an
/// `[info]` diagnostic when the source has been modified more recently.
/// Returns `None` when the `.pex` doesn't exist yet, `script_path` has no
/// conventional output location to check, or either file's modification
/// time can't be read — none of which should be treated as "stale", only
/// as "nothing to compare".
pub fn check(script_path: &Path) -> Option<Diagnostic> {
    let pex_path = pex_path_for(script_path)?;

    let script_modified = std::fs::metadata(script_path)
        .and_then(|metadata| metadata.modified())
        .ok()?;
    let pex_modified = std::fs::metadata(&pex_path)
        .and_then(|metadata| metadata.modified())
        .ok()?;

    (script_modified > pex_modified).then(|| Diagnostic {
        line: 1,
        column: 1,
        rule: RULE,
        message: format!(
            "[info] The compiled output at {} is older than this script; recompile it to pick up recent changes",
            pex_path.display()
        ),
    })
}

#[cfg(test)]
mod tests {
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
}
