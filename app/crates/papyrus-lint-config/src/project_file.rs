//! The full contents of a project's papyrus-lint YAML config file: the
//! [`papyrus_lints::Config`] passed to every check/fix job (flattened at
//! the top level), plus the app-level settings that aren't lint-related —
//! currently the optional explicit PapyrusCompiler.exe path override,
//! additional/lookup script roots, and the compile-check/achlist-scope
//! flags. This module owns the [`ProjectFile`] model, discovering and
//! (de)serializing it as YAML, and the per-field loaders/savers built on
//! top of that: each one loads the whole file, updates its own field, and
//! saves the whole file back, so unrelated settings are never disturbed.

use std::fs;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::comments::with_field_comments;
use crate::skyrim::{detected_skyrim_script_lookup_dirs, merge_lookup_roots};

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
    compiler_path: Option<String>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    additional_script_roots: Vec<String>,
    /// Extra directories searched only as a last-resort fallback when
    /// resolving a script by name for analysis (cross-script type/function
    /// lookups, `Extends`, autocompletion). Scripts found only here are
    /// never linted, and these directories are never considered by
    /// `conflicting_script_versions`. Intended for the game's own vanilla
    /// sources (e.g. Skyrim Special Edition's `Data/Scripts/Source` and
    /// `Data/Source/Scripts`).
    lookup_script_roots: Vec<String>,
    /// Whether the loaded YAML actually contained a `lookup_script_roots`
    /// key. Missing is treated as "not yet configured", so creating or
    /// updating a config can fill Skyrim Special Edition's vanilla source
    /// directories from the Windows registry. An explicit empty list is
    /// left empty rather than re-filled.
    #[serde(skip)]
    lookup_script_roots_explicit: bool,
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
    compile_check: bool,
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
    strict_achlist_scope: bool,
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
fn load_project_file(dir: &Path) -> Result<ProjectFile, String> {
    let Some(path) = existing_config_path(dir) else {
        return Ok(ProjectFile::default());
    };
    load_project_file_from_path(&path)
}

/// Reads and parses the papyrus-lint YAML at `path`. Empty files become
/// [`ProjectFile::default`]. A file that does not yet contain
/// `lookup_script_roots` is seeded in memory with Skyrim Special Edition's
/// vanilla source directories when those can be found (see
/// [`crate::skyrim::detected_skyrim_script_lookup_dirs`]); an explicit empty
/// list is kept.
fn load_project_file_from_path(path: &Path) -> Result<ProjectFile, String> {
    let contents = fs::read_to_string(path).map_err(|err| format!("{}: {err}", path.display()))?;
    project_file_from_yaml(&contents).map_err(|err| format!("{}: {err}", path.display()))
}

fn project_file_from_yaml(contents: &str) -> Result<ProjectFile, String> {
    if contents.trim().is_empty() {
        return Ok(ProjectFile::default());
    }
    let mut project: ProjectFile =
        serde_norway::from_str(contents).map_err(|err| err.to_string())?;
    project.lookup_script_roots_explicit = yaml_has_top_level_key(contents, "lookup_script_roots");
    if !project.lookup_script_roots_explicit {
        merge_lookup_roots(
            &mut project.lookup_script_roots,
            &detected_skyrim_script_lookup_dirs(),
        );
    }
    Ok(project)
}

/// Parses a YAML config document into a [`papyrus_lints::Config`]. An empty
/// document (including a missing/empty config file's contents) yields
/// [`papyrus_lints::Config::default`]; keys the document omits also fall
/// back to their default. App-level keys (`compiler_path`,
/// `additional_script_roots`, ...) are accepted and ignored here — use
/// [`load_config`] / [`load_config_from_path`] when those matter.
pub fn parse_lint_yaml(yaml: &str) -> Result<papyrus_lints::Config, String> {
    Ok(project_file_from_yaml(yaml)?.lint)
}

/// Serializes a [`papyrus_lints::Config`] back into the YAML document
/// format read by [`parse_lint_yaml`], so callers that persist lint
/// settings (user presets, `init`'s lint section) share one serializer.
pub fn lint_config_to_yaml(config: &papyrus_lints::Config) -> Result<String, String> {
    serde_norway::to_string(config).map_err(|err| err.to_string())
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
fn save_project_file(dir: &Path, project: &ProjectFile) -> Result<(), String> {
    let path = existing_config_path(dir).unwrap_or_else(|| dir.join(CONFIG_FILE_NAMES[0]));
    save_project_file_at(&path, project)
}

/// Writes `project` to the exact file at `path`, creating it if it doesn't
/// exist yet. Shared by [`save_project_file`] (which first resolves `path`
/// from a project directory) and [`save_config_at_path`] (which targets an
/// explicit file directly).
fn save_project_file_at(path: &Path, project: &ProjectFile) -> Result<(), String> {
    let mut project = project.clone();
    seed_lookup_script_roots(&mut project);
    let yaml = serde_norway::to_string(&project).map_err(|err| err.to_string())?;
    fs::write(path, with_field_comments(&yaml)).map_err(|err| err.to_string())
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

/// Looks for a papyrus-lint config file in `dir` and parses it into a
/// [`papyrus_lints::Config`]. Returns [`papyrus_lints::Config::default`]
/// if `dir` contains none of the candidate file names.
pub fn load_config(dir: &Path) -> Result<papyrus_lints::Config, String> {
    Ok(load_project_file(dir)?.lint)
}

/// Reads and parses an explicit config file at `path`, bypassing the
/// `papyrus-lint.yaml`/`.yml` discovery [`load_config`] does in a project
/// directory. Used for an explicit override (e.g. a `--config` CLI flag,
/// or an editor plugin's configured path) that names a config file
/// directly, which need not be called `papyrus-lint.yaml`/`.yml` or live
/// in the project root. Returns an error if `path` doesn't exist or fails
/// to parse.
pub fn load_config_from_path(path: &Path) -> Result<papyrus_lints::Config, String> {
    let contents = fs::read_to_string(path).map_err(|err| err.to_string())?;
    parse_lint_yaml(&contents)
}

/// Writes `config` to `dir`'s papyrus-lint YAML config file, preserving
/// any explicit PapyrusCompiler.exe path override already stored there.
pub fn save_config(dir: &Path, config: &papyrus_lints::Config) -> Result<(), String> {
    let mut project = load_project_file(dir)?;
    project.lint = config.clone();
    save_project_file(dir, &project)
}

/// Writes `config` to the exact file at `path`, bypassing the
/// `papyrus-lint.yaml`/`.yml` discovery [`save_config`] does in a project
/// directory — the save-side counterpart of [`load_config_from_path`], used
/// when the desktop app's user-selected config file override (rather than
/// the project's own auto-detected config file) is in effect. Preserves any
/// other settings (compiler path, additional script roots, ...) already
/// stored in that file; creates the file if `path` doesn't exist yet.
pub fn save_config_at_path(path: &Path, config: &papyrus_lints::Config) -> Result<(), String> {
    let mut project = if path.is_file() {
        load_project_file_from_path(path)?
    } else {
        ProjectFile::default()
    };
    project.lint = config.clone();
    save_project_file_at(path, &project)
}

/// Reads `dir`'s papyrus-lint config file and returns the explicit
/// PapyrusCompiler.exe path override it stores, if any (an empty string is
/// treated the same as no override).
pub fn load_compiler_path(dir: &Path) -> Result<Option<String>, String> {
    let path = load_project_file(dir)?.compiler_path;
    Ok(path.filter(|path| !path.trim().is_empty()))
}

/// Persists an explicit PapyrusCompiler.exe path override to `dir`'s
/// papyrus-lint config file, preserving its lint settings. `path: None`
/// (or an empty string) clears the override, reverting to auto-detection.
pub fn save_compiler_path(dir: &Path, path: Option<&str>) -> Result<(), String> {
    let mut project = load_project_file(dir)?;
    project.compiler_path = path
        .map(str::trim)
        .filter(|path| !path.is_empty())
        .map(str::to_owned);
    save_project_file(dir, &project)
}

/// Reads `dir`'s papyrus-lint config file and returns whether it enables
/// running PapyrusCompiler.exe as part of linting a `.psc`, surfacing any
/// errors it reports alongside the lint engine's own findings. `false`
/// (the default) if `dir` has no config file or doesn't set the key.
pub fn load_compile_check(dir: &Path) -> Result<bool, String> {
    Ok(load_project_file(dir)?.compile_check)
}

/// Persists whether the desktop app runs PapyrusCompiler.exe as part of
/// linting a `.psc` to `dir`'s papyrus-lint config file, preserving its
/// other settings.
pub fn save_compile_check(dir: &Path, enabled: bool) -> Result<(), String> {
    let mut project = load_project_file(dir)?;
    project.compile_check = enabled;
    save_project_file(dir, &project)
}

/// Reads `dir`'s papyrus-lint config file and returns whether it scopes
/// cross-script resolution/`conflicting_script_versions` strictly to an
/// `.achlist`'s own listed entries, rather than treating every listed
/// entry's parent directory as a generic search root. `false` (the
/// default, preserving the resolution an achlist project may already
/// depend on) if `dir` has no config file or doesn't set the key.
pub fn load_strict_achlist_scope(dir: &Path) -> Result<bool, String> {
    Ok(load_project_file(dir)?.strict_achlist_scope)
}

/// Reads an explicit config file at `path` (see [`load_config_from_path`])
/// and returns whether it sets `strict_achlist_scope`, the same way
/// [`load_strict_achlist_scope`] does for a project directory's own
/// papyrus-lint.yaml/.yml. Used so a `--config <path>` override still
/// honors the flag from the file it explicitly names, instead of that file
/// being read only for its `papyrus_lints::Config` fields. Returns an
/// error if `path` doesn't exist or fails to parse.
pub fn load_strict_achlist_scope_from_path(path: &Path) -> Result<bool, String> {
    let contents = fs::read_to_string(path).map_err(|err| err.to_string())?;
    if contents.trim().is_empty() {
        return Ok(false);
    }
    Ok(project_file_from_yaml(&contents)?.strict_achlist_scope)
}

/// Reads `dir`'s papyrus-lint config file and returns the additional script
/// root directories it lists, if any (see [`crate::script_locator`] for how
/// they're used alongside the conventional `scripts/source`/`source/scripts`
/// directories to resolve cross-script lookups and the compiler's `-i`
/// argument). Empty (or blank) entries are dropped. Returns an empty `Vec`
/// if `dir` has no config file or it declares none.
pub fn load_script_roots(dir: &Path) -> Result<Vec<String>, String> {
    let roots = load_project_file(dir)?.additional_script_roots;
    Ok(roots
        .into_iter()
        .map(|root| root.trim().to_string())
        .filter(|root| !root.is_empty())
        .collect())
}

/// Persists `roots` as `dir`'s papyrus-lint config file's additional script
/// root directories, preserving its lint settings and compiler path
/// override. Empty (or blank) entries are dropped before saving.
pub fn save_script_roots(dir: &Path, roots: &[String]) -> Result<(), String> {
    let mut project = load_project_file(dir)?;
    project.additional_script_roots = roots
        .iter()
        .map(|root| root.trim().to_string())
        .filter(|root| !root.is_empty())
        .collect();
    save_project_file(dir, &project)
}

/// Reads `dir`'s papyrus-lint config file and returns the analysis-only
/// lookup directories it lists (see [`crate::script_locator`] /
/// [`crate::function_table::FunctionTable::with_lookup_roots`]). These are
/// searched only after the conventional and `additional_script_roots`
/// directories, never linted, and never considered by
/// `conflicting_script_versions`. Empty (or blank) entries are dropped. A
/// config that does not yet set the key is seeded in memory with Skyrim
/// Special Edition's vanilla source directories when those can be found
/// (see [`crate::skyrim::detected_skyrim_script_lookup_dirs`]).
pub fn load_lookup_script_roots(dir: &Path) -> Result<Vec<String>, String> {
    Ok(trimmed_roots(load_project_file(dir)?.lookup_script_roots))
}

/// Reads an explicit config file at `path` (see [`load_config_from_path`])
/// and returns its `lookup_script_roots`, the same way
/// [`load_lookup_script_roots`] does for a project directory's own
/// papyrus-lint.yaml/.yml. Used so a `--config <path>` override still
/// honors analysis-only lookup directories from the file it names.
pub fn load_lookup_script_roots_from_path(path: &Path) -> Result<Vec<String>, String> {
    Ok(trimmed_roots(
        load_project_file_from_path(path)?.lookup_script_roots,
    ))
}

/// Persists `roots` as `dir`'s papyrus-lint config file's analysis-only
/// lookup directories, preserving its other settings. Empty (or blank)
/// entries are dropped. Setting this (including to an empty list) marks
/// the key as explicit so a later save does not re-fill Skyrim's vanilla
/// source directories from the registry.
pub fn save_lookup_script_roots(dir: &Path, roots: &[String]) -> Result<(), String> {
    let mut project = load_project_file(dir)?;
    project.lookup_script_roots = trimmed_roots(roots.iter().cloned());
    project.lookup_script_roots_explicit = true;
    save_project_file(dir, &project)
}

fn trimmed_roots(roots: impl IntoIterator<Item = String>) -> Vec<String> {
    roots
        .into_iter()
        .map(|root| root.trim().to_string())
        .filter(|root| !root.is_empty())
        .collect()
}

/// Seeds `project`'s `lookup_script_roots` with detected Skyrim Special
/// Edition vanilla source directories (see
/// [`crate::skyrim::detected_skyrim_script_lookup_dirs`]) if it hasn't been
/// set explicitly yet, and marks it explicit afterward either way so a
/// later save never re-fills it again.
pub(crate) fn seed_lookup_script_roots(project: &mut ProjectFile) {
    if project.lookup_script_roots_explicit {
        return;
    }
    merge_lookup_roots(
        &mut project.lookup_script_roots,
        &detected_skyrim_script_lookup_dirs(),
    );
    project.lookup_script_roots_explicit = true;
}

#[cfg(test)]
mod tests {
    use super::*;
    use papyrus_lints::config::Indentation;

    use crate::presets::{initialize_default_config, Preset};

    fn write_config(dir: &Path, name: &str, contents: &str) {
        fs::write(dir.join(name), contents).expect("failed to write test config file");
    }

    #[test]
    fn returns_defaults_when_no_config_file_present() {
        let dir = tempfile::tempdir().expect("failed to create temp dir");

        let config = load_config(dir.path()).expect("loading should succeed");

        assert_eq!(config, papyrus_lints::Config::default());
    }

    #[test]
    fn loads_yaml_config_file() {
        let dir = tempfile::tempdir().expect("failed to create temp dir");
        write_config(
            dir.path(),
            "papyrus-lint.yaml",
            "semicolon: true\nindentation: space\n",
        );

        let config = load_config(dir.path()).expect("loading should succeed");

        assert!(config.semicolon);
        assert_eq!(config.indentation, Indentation::Space);
    }

    #[test]
    fn loads_yml_config_file() {
        let dir = tempfile::tempdir().expect("failed to create temp dir");
        write_config(dir.path(), "papyrus-lint.yml", "semicolon: true\n");

        let config = load_config(dir.path()).expect("loading should succeed");

        assert!(config.semicolon);
    }

    #[test]
    fn prefers_yaml_over_yml() {
        let dir = tempfile::tempdir().expect("failed to create temp dir");
        write_config(dir.path(), "papyrus-lint.yaml", "semicolon: true\n");
        write_config(dir.path(), "papyrus-lint.yml", "semicolon: false\n");

        let config = load_config(dir.path()).expect("loading should succeed");

        assert!(config.semicolon);
    }

    #[test]
    fn config_file_path_reports_the_selected_config() {
        let dir = tempfile::tempdir().expect("failed to create temp dir");
        assert_eq!(config_file_path(dir.path()), None);

        let yml = dir.path().join("papyrus-lint.yml");
        write_config(dir.path(), "papyrus-lint.yml", "semicolon: false\n");
        assert_eq!(config_file_path(dir.path()), Some(yml));

        let yaml = dir.path().join("papyrus-lint.yaml");
        write_config(dir.path(), "papyrus-lint.yaml", "semicolon: true\n");
        assert_eq!(config_file_path(dir.path()), Some(yaml));
    }

    #[test]
    fn config_file_path_ignores_directories_with_config_names() {
        let dir = tempfile::tempdir().expect("failed to create temp dir");
        fs::create_dir(dir.path().join("papyrus-lint.yaml"))
            .expect("failed to create misleading config directory");

        assert_eq!(config_file_path(dir.path()), None);
    }

    #[test]
    fn whitespace_only_project_config_returns_defaults() {
        let dir = tempfile::tempdir().expect("failed to create temp dir");
        write_config(dir.path(), "papyrus-lint.yaml", " \t\n\r\n");

        assert_eq!(
            load_config(dir.path()).expect("loading should succeed"),
            papyrus_lints::Config::default()
        );
    }

    #[test]
    fn load_config_from_path_reads_an_explicit_file_regardless_of_name() {
        let dir = tempfile::tempdir().expect("failed to create temp dir");
        let path = dir.path().join("custom-config.yaml");
        fs::write(&path, "semicolon: true\nindentation: space\n")
            .expect("failed to write test config file");

        let config = load_config_from_path(&path).expect("loading should succeed");

        assert!(config.semicolon);
        assert_eq!(config.indentation, Indentation::Space);
    }

    #[test]
    fn load_config_from_path_returns_defaults_for_an_empty_file() {
        let dir = tempfile::tempdir().expect("failed to create temp dir");
        let path = dir.path().join("custom-config.yaml");
        fs::write(&path, "").expect("failed to write test config file");

        let config = load_config_from_path(&path).expect("loading should succeed");

        assert_eq!(config, papyrus_lints::Config::default());
    }

    #[test]
    fn load_config_from_path_errors_when_the_file_is_missing() {
        let dir = tempfile::tempdir().expect("failed to create temp dir");
        let path = dir.path().join("missing.yaml");

        assert!(load_config_from_path(&path).is_err());
    }

    #[test]
    fn save_config_at_path_creates_the_file_when_it_does_not_exist() {
        let dir = tempfile::tempdir().expect("failed to create temp dir");
        let path = dir.path().join("custom-config.yaml");
        let config = papyrus_lints::Config {
            semicolon: true,
            indentation: Indentation::Space,
            ..papyrus_lints::Config::default()
        };

        save_config_at_path(&path, &config).expect("saving should succeed");

        assert!(path.is_file());
        assert_eq!(
            load_config_from_path(&path).expect("loading should succeed"),
            config
        );
    }

    #[test]
    fn save_config_at_path_preserves_other_settings_already_in_the_file() {
        let dir = tempfile::tempdir().expect("failed to create temp dir");
        let path = dir.path().join("custom-config.yaml");
        fs::write(
            &path,
            "compiler_path: C:\\Tools\\PapyrusCompiler.exe\nsemicolon: false\n",
        )
        .expect("failed to write test config file");
        let config = papyrus_lints::Config {
            semicolon: true,
            ..papyrus_lints::Config::default()
        };

        save_config_at_path(&path, &config).expect("saving should succeed");

        assert_eq!(
            load_config_from_path(&path).expect("loading should succeed"),
            config
        );
        let contents = fs::read_to_string(&path).expect("failed to read saved config file");
        assert!(contents.contains("compiler_path: C:\\Tools\\PapyrusCompiler.exe"));
    }

    #[test]
    fn load_config_from_path_errors_on_invalid_yaml() {
        let dir = tempfile::tempdir().expect("failed to create temp dir");
        let path = dir.path().join("custom-config.yaml");
        fs::write(&path, "semicolon: [not a bool\n").expect("failed to write test config file");

        assert!(load_config_from_path(&path).is_err());
    }

    #[test]
    fn parse_lint_yaml_returns_defaults_for_empty_and_whitespace_only_documents() {
        assert_eq!(
            parse_lint_yaml("").expect("empty yaml should parse"),
            papyrus_lints::Config::default()
        );
        assert_eq!(
            parse_lint_yaml(" \t\n\r\n").expect("whitespace-only yaml should parse"),
            papyrus_lints::Config::default()
        );
    }

    #[test]
    fn parse_lint_yaml_applies_omitted_keys_as_defaults() {
        let config = parse_lint_yaml("semicolon: true\nindentation: space\n")
            .expect("partial yaml should parse");

        assert!(config.semicolon);
        assert_eq!(config.indentation, Indentation::Space);
        assert_eq!(config.indentation_width, 4);
        assert!(!config.fail_on_warning);
        assert!(config.rules.trailing_whitespace);
    }

    #[test]
    fn parse_lint_yaml_ignores_app_level_keys() {
        let config = parse_lint_yaml(
            "compiler_path: C:\\Tools\\PapyrusCompiler.exe\ncompile_check: true\nsemicolon: true\n",
        )
        .expect("project yaml should parse as lint config");

        assert!(config.semicolon);
        assert_eq!(config.indentation, Indentation::default());
    }

    #[test]
    fn parse_lint_yaml_rejects_invalid_yaml() {
        assert!(parse_lint_yaml("semicolon: [not a bool\n").is_err());
        assert!(parse_lint_yaml("indentation: eight-spaces\n").is_err());
    }

    #[test]
    fn lint_config_to_yaml_round_trips_through_parse_lint_yaml() {
        let config = papyrus_lints::Config {
            semicolon: true,
            indentation: Indentation::Space,
            indentation_width: 2,
            ..papyrus_lints::Config::default()
        };

        let yaml = lint_config_to_yaml(&config).expect("config should serialize");
        assert_eq!(
            parse_lint_yaml(&yaml).expect("serialized config should parse"),
            config
        );
    }

    #[test]
    fn save_config_at_path_rejects_invalid_existing_yaml_without_overwriting_it() {
        let dir = tempfile::tempdir().expect("failed to create temp dir");
        let path = dir.path().join("custom-config.yaml");
        let invalid = "semicolon: [not a bool\n";
        fs::write(&path, invalid).expect("failed to write test config file");

        assert!(save_config_at_path(&path, &papyrus_lints::Config::default()).is_err());
        assert_eq!(
            fs::read_to_string(path).expect("failed to read test config file"),
            invalid
        );
    }

    #[test]
    fn default_config_matches_the_checked_in_docs_copy() {
        let dir = tempfile::tempdir().expect("failed to create temp dir");

        let path =
            initialize_default_config(dir.path(), Preset::default()).expect("init should succeed");
        let generated = fs::read_to_string(&path).expect("failed to read generated config");

        let docs_path =
            Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../docs/papyrus-lint.default.yaml");
        let docs_copy =
            fs::read_to_string(&docs_path).expect("failed to read docs/papyrus-lint.default.yaml");

        let detected = detected_skyrim_script_lookup_dirs();
        if detected.is_empty() {
            assert_eq!(
                generated, docs_copy,
                "docs/papyrus-lint.default.yaml is out of date; regenerate it with `PapyrusLinterCLI init`"
            );
        } else {
            for dir in &detected {
                assert!(
                    generated.contains(dir),
                    "init should fill lookup_script_roots with {dir}"
                );
            }
        }
    }

    #[test]
    fn errors_on_invalid_yaml() {
        let dir = tempfile::tempdir().expect("failed to create temp dir");
        write_config(dir.path(), "papyrus-lint.yaml", "semicolon: [not a bool\n");

        let result = load_config(dir.path());

        assert!(result.is_err());
    }

    #[test]
    fn save_creates_yaml_file_when_none_exists() {
        let dir = tempfile::tempdir().expect("failed to create temp dir");
        let config = papyrus_lints::Config {
            semicolon: true,
            indentation: Indentation::Space,
            indentation_width: 2,
            ..papyrus_lints::Config::default()
        };

        save_config(dir.path(), &config).expect("saving should succeed");

        assert!(dir.path().join("papyrus-lint.yaml").is_file());
        assert!(!dir.path().join("papyrus-lint.yml").exists());
        let loaded = load_config(dir.path()).expect("loading should succeed");
        assert_eq!(loaded, config);
    }

    #[test]
    fn save_annotates_top_level_keys_with_explanatory_comments() {
        let dir = tempfile::tempdir().expect("failed to create temp dir");
        let config = papyrus_lints::Config {
            semicolon: true,
            ..papyrus_lints::Config::default()
        };

        save_config(dir.path(), &config).expect("saving should succeed");

        let contents = fs::read_to_string(dir.path().join("papyrus-lint.yaml"))
            .expect("failed to read saved config file");
        assert!(contents.contains("# true, false\nsemicolon: true\n"));
        assert!(contents.contains("# tab, space\nindentation: tab\n"));
        assert!(contents.contains("# Each rule accepts true or false\nrules:\n"));
        // Nested rule keys aren't individually commented, matching the
        // README's example, which only comments the `rules:` block itself.
        assert!(!contents.contains("trailing_whitespace:\n  #"));
    }

    #[test]
    fn save_overwrites_existing_yaml_file() {
        let dir = tempfile::tempdir().expect("failed to create temp dir");
        write_config(dir.path(), "papyrus-lint.yaml", "semicolon: false\n");
        let config = papyrus_lints::Config {
            semicolon: true,
            ..papyrus_lints::Config::default()
        };

        save_config(dir.path(), &config).expect("saving should succeed");

        let loaded = load_config(dir.path()).expect("loading should succeed");
        assert_eq!(loaded, config);
    }

    #[test]
    fn save_prefers_existing_yml_file_over_creating_yaml() {
        let dir = tempfile::tempdir().expect("failed to create temp dir");
        write_config(dir.path(), "papyrus-lint.yml", "semicolon: false\n");
        let config = papyrus_lints::Config {
            semicolon: true,
            ..papyrus_lints::Config::default()
        };

        save_config(dir.path(), &config).expect("saving should succeed");

        assert!(!dir.path().join("papyrus-lint.yaml").exists());
        let loaded = load_config(dir.path()).expect("loading should succeed");
        assert_eq!(loaded, config);
    }

    #[test]
    fn load_compiler_path_returns_none_when_unset() {
        let dir = tempfile::tempdir().expect("failed to create temp dir");

        assert_eq!(
            load_compiler_path(dir.path()).expect("should succeed"),
            None
        );
    }

    #[test]
    fn load_compiler_path_reads_explicit_override() {
        let dir = tempfile::tempdir().expect("failed to create temp dir");
        write_config(
            dir.path(),
            "papyrus-lint.yaml",
            "compiler_path: C:\\Tools\\PapyrusCompiler.exe\n",
        );

        assert_eq!(
            load_compiler_path(dir.path()).expect("should succeed"),
            Some("C:\\Tools\\PapyrusCompiler.exe".to_string())
        );
    }

    #[test]
    fn load_compiler_path_treats_blank_override_as_unset() {
        let dir = tempfile::tempdir().expect("failed to create temp dir");
        write_config(dir.path(), "papyrus-lint.yaml", "compiler_path: \"   \"\n");

        assert_eq!(
            load_compiler_path(dir.path()).expect("should succeed"),
            None
        );
    }

    #[test]
    fn save_compiler_path_trims_the_override() {
        let dir = tempfile::tempdir().expect("failed to create temp dir");

        save_compiler_path(dir.path(), Some("  C:\\Tools\\PapyrusCompiler.exe  "))
            .expect("saving compiler path should succeed");

        assert_eq!(
            load_compiler_path(dir.path()).expect("should succeed"),
            Some("C:\\Tools\\PapyrusCompiler.exe".to_string())
        );
    }

    #[test]
    fn save_compiler_path_persists_override_without_disturbing_lint_settings() {
        let dir = tempfile::tempdir().expect("failed to create temp dir");
        let config = papyrus_lints::Config {
            semicolon: true,
            ..papyrus_lints::Config::default()
        };
        save_config(dir.path(), &config).expect("saving lint config should succeed");

        save_compiler_path(dir.path(), Some("C:\\Tools\\PapyrusCompiler.exe"))
            .expect("saving compiler path should succeed");

        assert_eq!(
            load_compiler_path(dir.path()).expect("should succeed"),
            Some("C:\\Tools\\PapyrusCompiler.exe".to_string())
        );
        assert_eq!(load_config(dir.path()).expect("should succeed"), config);
    }

    #[test]
    fn save_config_preserves_existing_compiler_path_override() {
        let dir = tempfile::tempdir().expect("failed to create temp dir");
        save_compiler_path(dir.path(), Some("C:\\Tools\\PapyrusCompiler.exe"))
            .expect("saving compiler path should succeed");

        let config = papyrus_lints::Config {
            semicolon: true,
            ..papyrus_lints::Config::default()
        };
        save_config(dir.path(), &config).expect("saving lint config should succeed");

        assert_eq!(
            load_compiler_path(dir.path()).expect("should succeed"),
            Some("C:\\Tools\\PapyrusCompiler.exe".to_string())
        );
        assert_eq!(load_config(dir.path()).expect("should succeed"), config);
    }

    #[test]
    fn save_compiler_path_none_clears_override() {
        let dir = tempfile::tempdir().expect("failed to create temp dir");
        save_compiler_path(dir.path(), Some("C:\\Tools\\PapyrusCompiler.exe"))
            .expect("saving compiler path should succeed");

        save_compiler_path(dir.path(), None).expect("clearing compiler path should succeed");

        assert_eq!(
            load_compiler_path(dir.path()).expect("should succeed"),
            None
        );
    }

    #[test]
    fn load_compile_check_defaults_to_false_when_unset() {
        let dir = tempfile::tempdir().expect("failed to create temp dir");

        assert!(!load_compile_check(dir.path()).expect("should succeed"));
    }

    #[test]
    fn save_and_load_compile_check_round_trips_without_disturbing_lint_settings() {
        let dir = tempfile::tempdir().expect("failed to create temp dir");
        let config = papyrus_lints::Config {
            semicolon: true,
            ..papyrus_lints::Config::default()
        };
        save_config(dir.path(), &config).expect("saving lint config should succeed");
        save_compiler_path(dir.path(), Some("C:\\Tools\\PapyrusCompiler.exe"))
            .expect("saving compiler path should succeed");

        save_compile_check(dir.path(), true).expect("saving compile check should succeed");

        assert!(load_compile_check(dir.path()).expect("should succeed"));
        assert_eq!(load_config(dir.path()).expect("should succeed"), config);
        assert_eq!(
            load_compiler_path(dir.path()).expect("should succeed"),
            Some("C:\\Tools\\PapyrusCompiler.exe".to_string())
        );
    }

    #[test]
    fn save_compile_check_false_clears_it() {
        let dir = tempfile::tempdir().expect("failed to create temp dir");
        save_compile_check(dir.path(), true).expect("saving compile check should succeed");

        save_compile_check(dir.path(), false).expect("clearing compile check should succeed");

        assert!(!load_compile_check(dir.path()).expect("should succeed"));
    }

    #[test]
    fn saved_config_omits_compile_check_when_disabled() {
        let dir = tempfile::tempdir().expect("failed to create temp dir");

        save_config(dir.path(), &papyrus_lints::Config::default())
            .expect("saving lint config should succeed");

        let contents = fs::read_to_string(dir.path().join("papyrus-lint.yaml"))
            .expect("failed to read saved config file");
        assert!(!contents.contains("compile_check"));
    }

    #[test]
    fn load_strict_achlist_scope_defaults_to_false_when_unset() {
        let dir = tempfile::tempdir().expect("failed to create temp dir");

        assert!(!load_strict_achlist_scope(dir.path()).expect("should succeed"));
    }

    #[test]
    fn load_strict_achlist_scope_reads_the_configured_value() {
        let dir = tempfile::tempdir().expect("failed to create temp dir");
        write_config(
            dir.path(),
            "papyrus-lint.yaml",
            "strict_achlist_scope: true\n",
        );

        assert!(load_strict_achlist_scope(dir.path()).expect("should succeed"));
    }

    #[test]
    fn load_strict_achlist_scope_from_path_reads_an_explicit_file() {
        let dir = tempfile::tempdir().expect("failed to create temp dir");
        let path = dir.path().join("custom-config.yaml");
        fs::write(&path, "strict_achlist_scope: true\n").expect("failed to write test config file");

        assert!(load_strict_achlist_scope_from_path(&path).expect("loading should succeed"));
    }

    #[test]
    fn load_strict_achlist_scope_from_path_defaults_to_false_when_unset() {
        let dir = tempfile::tempdir().expect("failed to create temp dir");
        let path = dir.path().join("custom-config.yaml");
        fs::write(&path, "semicolon: true\n").expect("failed to write test config file");

        assert!(!load_strict_achlist_scope_from_path(&path).expect("loading should succeed"));
    }

    #[test]
    fn load_strict_achlist_scope_from_path_returns_false_for_an_empty_file() {
        let dir = tempfile::tempdir().expect("failed to create temp dir");
        let path = dir.path().join("custom-config.yaml");
        fs::write(&path, "").expect("failed to write test config file");

        assert!(!load_strict_achlist_scope_from_path(&path).expect("loading should succeed"));
    }

    #[test]
    fn load_strict_achlist_scope_from_path_errors_when_the_file_is_missing() {
        let dir = tempfile::tempdir().expect("failed to create temp dir");
        let path = dir.path().join("missing-config.yaml");

        assert!(load_strict_achlist_scope_from_path(&path).is_err());
    }

    #[test]
    fn load_strict_achlist_scope_from_path_errors_on_invalid_yaml() {
        let dir = tempfile::tempdir().expect("failed to create temp dir");
        let path = dir.path().join("custom-config.yaml");
        fs::write(&path, "strict_achlist_scope: [not a bool\n")
            .expect("failed to write test config file");

        assert!(load_strict_achlist_scope_from_path(&path).is_err());
    }

    #[test]
    fn saved_config_omits_strict_achlist_scope_when_disabled() {
        let dir = tempfile::tempdir().expect("failed to create temp dir");

        save_config(dir.path(), &papyrus_lints::Config::default())
            .expect("saving lint config should succeed");

        let contents = fs::read_to_string(dir.path().join("papyrus-lint.yaml"))
            .expect("failed to read saved config file");
        assert!(!contents.contains("strict_achlist_scope"));
    }

    #[test]
    fn load_script_roots_returns_empty_when_unset() {
        let dir = tempfile::tempdir().expect("failed to create temp dir");

        assert_eq!(
            load_script_roots(dir.path()).expect("should succeed"),
            Vec::<String>::new()
        );
    }

    #[test]
    fn save_and_load_script_roots_round_trips_without_disturbing_lint_settings() {
        let dir = tempfile::tempdir().expect("failed to create temp dir");
        let config = papyrus_lints::Config {
            semicolon: true,
            ..papyrus_lints::Config::default()
        };
        save_config(dir.path(), &config).expect("saving lint config should succeed");

        save_script_roots(
            dir.path(),
            &[
                "../SharedScripts".to_string(),
                "/abs/OtherScripts".to_string(),
            ],
        )
        .expect("saving script roots should succeed");

        assert_eq!(
            load_script_roots(dir.path()).expect("should succeed"),
            vec![
                "../SharedScripts".to_string(),
                "/abs/OtherScripts".to_string()
            ]
        );
        assert_eq!(load_config(dir.path()).expect("should succeed"), config);
    }

    #[test]
    fn save_script_roots_drops_blank_entries() {
        let dir = tempfile::tempdir().expect("failed to create temp dir");

        save_script_roots(
            dir.path(),
            &[
                "  ".to_string(),
                "../SharedScripts".to_string(),
                String::new(),
            ],
        )
        .expect("saving script roots should succeed");

        assert_eq!(
            load_script_roots(dir.path()).expect("should succeed"),
            vec!["../SharedScripts".to_string()]
        );
    }

    #[test]
    fn load_script_roots_trims_entries_from_yaml() {
        let dir = tempfile::tempdir().expect("failed to create temp dir");
        write_config(
            dir.path(),
            "papyrus-lint.yaml",
            "additional_script_roots:\n  - '  ../SharedScripts  '\n  - '   '\n",
        );

        assert_eq!(
            load_script_roots(dir.path()).expect("should succeed"),
            vec!["../SharedScripts".to_string()]
        );
    }

    #[test]
    fn save_script_roots_empty_clears_existing_roots() {
        let dir = tempfile::tempdir().expect("failed to create temp dir");
        save_script_roots(dir.path(), &["../SharedScripts".to_string()])
            .expect("saving script roots should succeed");

        save_script_roots(dir.path(), &[]).expect("clearing script roots should succeed");

        assert_eq!(
            load_script_roots(dir.path()).expect("should succeed"),
            Vec::<String>::new()
        );
    }

    #[test]
    fn save_config_preserves_existing_script_roots() {
        let dir = tempfile::tempdir().expect("failed to create temp dir");
        save_script_roots(dir.path(), &["../SharedScripts".to_string()])
            .expect("saving script roots should succeed");

        let config = papyrus_lints::Config {
            semicolon: true,
            ..papyrus_lints::Config::default()
        };
        save_config(dir.path(), &config).expect("saving lint config should succeed");

        assert_eq!(
            load_script_roots(dir.path()).expect("should succeed"),
            vec!["../SharedScripts".to_string()]
        );
    }

    #[test]
    fn load_lookup_script_roots_returns_empty_when_unset() {
        let dir = tempfile::tempdir().expect("failed to create temp dir");

        assert_eq!(
            load_lookup_script_roots(dir.path()).expect("should succeed"),
            Vec::<String>::new()
        );
    }

    #[test]
    fn save_and_load_lookup_script_roots_round_trips_without_disturbing_other_settings() {
        let dir = tempfile::tempdir().expect("failed to create temp dir");
        save_script_roots(dir.path(), &["../SharedScripts".to_string()])
            .expect("saving script roots should succeed");

        save_lookup_script_roots(
            dir.path(),
            &[
                "  C:/Skyrim/Data/Scripts/Source  ".to_string(),
                "  ".to_string(),
                "C:/Skyrim/Data/Source/Scripts".to_string(),
            ],
        )
        .expect("saving lookup roots should succeed");

        assert_eq!(
            load_lookup_script_roots(dir.path()).expect("should succeed"),
            vec![
                "C:/Skyrim/Data/Scripts/Source".to_string(),
                "C:/Skyrim/Data/Source/Scripts".to_string()
            ]
        );
        assert_eq!(
            load_script_roots(dir.path()).expect("should succeed"),
            vec!["../SharedScripts".to_string()]
        );
    }

    #[test]
    fn save_lookup_script_roots_empty_is_kept_explicit_and_not_refilled() {
        let dir = tempfile::tempdir().expect("failed to create temp dir");
        save_lookup_script_roots(dir.path(), &["C:/Skyrim/Data/Scripts/Source".to_string()])
            .expect("saving lookup roots should succeed");

        save_lookup_script_roots(dir.path(), &[]).expect("clearing lookup roots should succeed");

        assert_eq!(
            load_lookup_script_roots(dir.path()).expect("should succeed"),
            Vec::<String>::new()
        );
        let contents = fs::read_to_string(dir.path().join("papyrus-lint.yaml"))
            .expect("failed to read saved config");
        assert!(contents.contains("lookup_script_roots:"));
    }

    #[test]
    fn load_lookup_script_roots_trims_entries_from_yaml() {
        let dir = tempfile::tempdir().expect("failed to create temp dir");
        write_config(
            dir.path(),
            "papyrus-lint.yaml",
            "lookup_script_roots:\n  - '  C:/Skyrim/Data/Scripts/Source  '\n  - '   '\n",
        );

        assert_eq!(
            load_lookup_script_roots(dir.path()).expect("should succeed"),
            vec!["C:/Skyrim/Data/Scripts/Source".to_string()]
        );
    }

    #[test]
    fn save_config_preserves_existing_lookup_script_roots() {
        let dir = tempfile::tempdir().expect("failed to create temp dir");
        save_lookup_script_roots(dir.path(), &["C:/Skyrim/Data/Scripts/Source".to_string()])
            .expect("saving lookup roots should succeed");

        let config = papyrus_lints::Config {
            semicolon: true,
            ..papyrus_lints::Config::default()
        };
        save_config(dir.path(), &config).expect("saving lint config should succeed");

        assert_eq!(
            load_lookup_script_roots(dir.path()).expect("should succeed"),
            vec!["C:/Skyrim/Data/Scripts/Source".to_string()]
        );
    }

    #[test]
    fn seed_lookup_script_roots_fills_only_when_the_key_was_missing() {
        let mut unset = ProjectFile::default();
        merge_lookup_roots(
            &mut unset.lookup_script_roots,
            &["C:/Skyrim/Data/Scripts/Source".to_string()],
        );
        assert_eq!(
            unset.lookup_script_roots,
            vec!["C:/Skyrim/Data/Scripts/Source".to_string()]
        );

        let mut explicit = ProjectFile {
            lookup_script_roots_explicit: true,
            ..ProjectFile::default()
        };
        seed_lookup_script_roots(&mut explicit);
        assert!(explicit.lookup_script_roots.is_empty());
    }

    #[test]
    fn updating_a_config_without_lookup_script_roots_writes_the_key() {
        let dir = tempfile::tempdir().expect("failed to create temp dir");
        write_config(dir.path(), "papyrus-lint.yaml", "semicolon: true\n");

        save_script_roots(dir.path(), &["../SharedScripts".to_string()])
            .expect("saving script roots should succeed");

        let contents = fs::read_to_string(dir.path().join("papyrus-lint.yaml"))
            .expect("failed to read saved config");
        assert!(contents.contains("lookup_script_roots:"));
        assert!(contents.contains("semicolon: true"));
    }
}
