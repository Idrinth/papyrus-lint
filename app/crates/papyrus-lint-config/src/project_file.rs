//! Shared project-file model, config-file discovery, and YAML persistence.
//!
//! Concern-specific modules use these primitives to update one part of a
//! [`ProjectFile`] while preserving all unrelated settings.

use std::fs;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::comments::with_field_comments;
use crate::script_roots::{merge_detected_lookup_script_roots, seed_lookup_script_roots};

/// Candidate config file names, checked in order, inside a project's
/// directory (conventionally the directory containing its `.achlist`
/// file).
pub(crate) const CONFIG_FILE_NAMES: [&str; 2] = ["papyrus-lint.yaml", "papyrus-lint.yml"];

/// The full contents of a project's papyrus-lint YAML config file: the
/// lint/fix settings (flattened at the top level, unchanged from before)
/// plus app-level settings that aren't lint-related, currently just an
/// optional explicit PapyrusCompiler.exe path override.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub(crate) struct ProjectFile {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) compiler_path: Option<String>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub(crate) additional_script_roots: Vec<String>,
    /// Extra directories searched only as a last-resort fallback when
    /// resolving a script by name for analysis (cross-script type/function
    /// lookups, `Extends`, autocompletion). Scripts found only here are
    /// never linted, and these directories are never considered by
    /// `conflicting_script_versions`. Intended for the game's own vanilla
    /// sources (e.g. Skyrim Special Edition's `Data/Scripts/Source` and
    /// `Data/Source/Scripts`).
    pub(crate) lookup_script_roots: Vec<String>,
    /// Whether the loaded YAML actually contained a `lookup_script_roots`
    /// key. Missing is treated as "not yet configured", so creating or
    /// updating a config can fill the configured `game`'s vanilla source
    /// directories from the Windows registry. An explicit empty list is
    /// left empty rather than re-filled.
    #[serde(skip)]
    pub(crate) lookup_script_roots_explicit: bool,
    /// Whether the desktop app and the CLI also run PapyrusCompiler.exe (at
    /// `compiler_path`, above) against a `.psc` as part of linting it,
    /// surfacing any errors it reports as additional `[error]` diagnostics
    /// alongside the lint engine's own findings. `false` by default: it's
    /// opt-in since it requires a compiler path — configured or
    /// auto-detected — and is slower than the lint engine's own,
    /// dependency-free checks. Compiles to a
    /// throwaway temporary directory rather than the project's real output
    /// directory, so enabling it never touches (or requires write access
    /// to) the project's actual compiled `.pex` output.
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub(crate) compile_check: bool,
    /// Whether the CLI resolves cross-script lookups among an `.achlist`'s
    /// own entries by registering each listed `.psc` directly (see
    /// `FunctionTable::with_known_scripts`) instead of treating every listed
    /// entry's parent directory as an additional script root. `false` by
    /// default, which preserves the resolution and diagnostics an achlist
    /// project may already depend on: an unlisted sibling script sitting in
    /// the same non-conventional directory as a listed one still resolves,
    /// and `conflicting_script_versions` still scans every such directory
    /// rather than just the achlist's other listed entries. `true` instead
    /// scopes resolution (and `conflicting_script_versions`) strictly to the
    /// achlist's own listed entries — dramatically faster, and immune to
    /// unlisted files leaking in, on a large achlist whose entries are
    /// spread across many directories (see #311) — but requires every
    /// `.psc` an achlist's listed entries depend on to be listed itself.
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub(crate) strict_achlist_scope: bool,
    #[serde(flatten)]
    pub(crate) lint: papyrus_lints::Config,
}

/// Finds a project's existing config file in `dir`, if any.
pub(crate) fn existing_config_path(dir: &Path) -> Option<PathBuf> {
    CONFIG_FILE_NAMES
        .iter()
        .map(|name| dir.join(name))
        .find(|path| path.is_file())
}

/// Returns the configuration file selected for `dir`, if the project has
/// either of the supported configuration file names.
pub fn config_file_path(dir: &Path) -> Option<PathBuf> {
    existing_config_path(dir)
}

/// Reads and parses `dir`'s papyrus-lint config file, if it has one.
/// Returns [`ProjectFile::default`] if `dir` has none of the candidate
/// file names.
pub(crate) fn load_project_file(dir: &Path) -> Result<ProjectFile, String> {
    let Some(path) = existing_config_path(dir) else {
        return Ok(ProjectFile::default());
    };
    load_project_file_from_path(&path)
}

/// Reads and parses the papyrus-lint YAML at `path`. Empty files become
/// [`ProjectFile::default`]. A file that does not yet contain
/// `lookup_script_roots` is seeded in memory with the project's configured
/// game's vanilla source directories when those can be found (see
/// [`crate::game_install::detected_script_lookup_dirs_for_game`]); an
/// explicit empty list is kept.
pub(crate) fn load_project_file_from_path(path: &Path) -> Result<ProjectFile, String> {
    let contents = fs::read_to_string(path).map_err(|err| format!("{}: {err}", path.display()))?;
    project_file_from_yaml(&contents).map_err(|err| format!("{}: {err}", path.display()))
}

pub(crate) fn project_file_from_yaml(contents: &str) -> Result<ProjectFile, String> {
    if contents.trim().is_empty() {
        return Ok(ProjectFile::default());
    }
    let mut project: ProjectFile =
        serde_norway::from_str(contents).map_err(|err| err.to_string())?;
    project.lookup_script_roots_explicit = yaml_has_top_level_key(contents, "lookup_script_roots");
    if !project.lookup_script_roots_explicit {
        merge_detected_lookup_script_roots(&mut project);
    }
    Ok(project)
}

fn yaml_has_top_level_key(contents: &str, key: &str) -> bool {
    let Ok(serde_norway::Value::Mapping(map)) = serde_norway::from_str(contents) else {
        return false;
    };
    map.contains_key(serde_norway::Value::String(key.to_string()))
}

/// Writes `project` to `dir`'s papyrus-lint YAML config file. Overwrites
/// whichever candidate name (`papyrus-lint.yaml`/`.yml`) already exists in
/// `dir`, or creates `papyrus-lint.yaml` if `dir` has neither yet. Each
/// top-level key is preceded by the same explanatory comment the README
/// shows for it (see [`crate::comments::with_field_comments`]).
pub(crate) fn save_project_file(dir: &Path, project: &ProjectFile) -> Result<(), String> {
    let path = existing_config_path(dir).unwrap_or_else(|| dir.join(CONFIG_FILE_NAMES[0]));
    save_project_file_at(&path, project)
}

/// Writes `project` to the exact file at `path`, creating it if it doesn't
/// exist yet. Shared by [`save_project_file`] (which first resolves `path`
/// from a project directory) and [`save_config_at_path`] (which targets an
/// explicit file directly).
pub(crate) fn save_project_file_at(path: &Path, project: &ProjectFile) -> Result<(), String> {
    let mut project = project.clone();
    seed_lookup_script_roots(&mut project);
    let yaml = serde_norway::to_string(&project).map_err(|err| err.to_string())?;
    let yaml = game_key_first(&yaml);
    fs::write(path, with_field_comments(&yaml)).map_err(|err| err.to_string())
}

/// Moves the serialized `game` key ahead of the app-only settings. `game`
/// belongs to the flattened lint config, so serde otherwise emits it after
/// every field declared directly on [`ProjectFile`]. Keeping it first makes
/// saved files match the checked-in default configuration and presents the
/// project's target before settings whose behavior depends on that target.
pub(crate) fn game_key_first(yaml: &str) -> String {
    let mut game = None;
    let mut rest = String::with_capacity(yaml.len());
    for line in yaml.split_inclusive('\n') {
        if game.is_none() && line.starts_with("game:") {
            game = Some(line);
        } else {
            rest.push_str(line);
        }
    }

    match game {
        Some(game) => format!("{game}{rest}"),
        None => rest,
    }
}

/// Serializes the app-only (non-lint) settings of `project`, always
/// including every field regardless of [`ProjectFile`]'s `skip_serializing_if`
/// attributes, so an initialized file serves as a complete, discoverable
/// template rather than omitting whichever settings happen to match their
/// default.
pub(crate) fn non_lint_yaml(project: &ProjectFile) -> Result<String, String> {
    #[derive(Serialize)]
    struct NonLintFields<'a> {
        compiler_path: &'a Option<String>,
        additional_script_roots: &'a [String],
        lookup_script_roots: &'a [String],
        compile_check: bool,
        strict_achlist_scope: bool,
    }

    serde_norway::to_string(&NonLintFields {
        compiler_path: &project.compiler_path,
        additional_script_roots: &project.additional_script_roots,
        lookup_script_roots: &project.lookup_script_roots,
        compile_check: project.compile_check,
        strict_achlist_scope: project.strict_achlist_scope,
    })
    .map_err(|err| err.to_string())
}
