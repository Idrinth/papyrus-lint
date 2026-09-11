//! Locates and loads a project's papyrus-lint YAML configuration file,
//! producing the [`papyrus_lints::Config`] passed to every check/fix job,
//! and the app-level settings (currently just the PapyrusCompiler.exe path)
//! that live in the same file alongside it.

use std::borrow::Cow;
use std::fs;
use std::fs::OpenOptions;
use std::io::Write;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

/// Candidate config file names, checked in order, inside a project's
/// directory (conventionally the directory containing its `.achlist`
/// file).
const CONFIG_FILE_NAMES: [&str; 2] = ["papyrus-lint.yaml", "papyrus-lint.yml"];

/// The name of the compiler executable looked for during auto-detection.
const COMPILER_EXECUTABLE_NAME: &str = "PapyrusCompiler.exe";

/// The directory (relative to a project's `.achlist` directory) that
/// auto-detection looks for the compiler executable under.
const COMPILER_AUTO_DETECT_DIR_NAME: &str = "Papyrus Compiler";

/// The full contents of a project's papyrus-lint YAML config file: the
/// lint/fix settings (flattened at the top level, unchanged from before)
/// plus app-level settings that aren't lint-related, currently just an
/// optional explicit PapyrusCompiler.exe path override.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
struct ProjectFile {
    #[serde(skip_serializing_if = "Option::is_none")]
    compiler_path: Option<String>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    additional_script_roots: Vec<String>,
    /// Whether the desktop app also runs PapyrusCompiler.exe (at
    /// `compiler_path`, above) against a dropped `.psc` as part of linting
    /// it, surfacing any errors it reports as additional `[error]`
    /// diagnostics alongside the lint engine's own findings. `false` by
    /// default: it's opt-in since it requires a configured compiler path
    /// and is slower than the lint engine's own, dependency-free checks.
    /// Compiles to a throwaway temporary directory rather than the
    /// project's real output directory, so enabling it never touches (or
    /// requires write access to) the project's actual compiled `.pex`
    /// output.
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
    lint: papyrus_lints::Config,
}

/// Finds a project's existing config file in `dir`, if any.
fn existing_config_path(dir: &Path) -> Option<PathBuf> {
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

    let contents = fs::read_to_string(&path).map_err(|err| err.to_string())?;
    if contents.trim().is_empty() {
        return Ok(ProjectFile::default());
    }
    serde_yaml::from_str(&contents).map_err(|err| err.to_string())
}

/// The explanatory comment shown above each top-level key in the README's
/// [configuration reference](../../../../README.md#configuration), in the
/// same order `ProjectFile`/`papyrus_lints::Config` declare their fields.
/// Kept in sync with that table so a saved config file documents itself
/// the same way.
const FIELD_COMMENTS: &[(&str, &str)] = &[
    (
        "compiler_path",
        "# Path to PapyrusCompiler.exe, or null to auto-detect it",
    ),
    (
        "additional_script_roots",
        "# Extra directories (relative to the project root, or absolute) to search\n\
         # for .psc files, besides scripts/source and source/scripts",
    ),
    (
        "compile_check",
        "# true, false; also runs PapyrusCompiler.exe (into a throwaway temporary\n\
         # directory) as part of linting a dropped .psc, reporting its errors\n\
         # alongside the lint engine's own. Requires compiler_path to be set/\n\
         # auto-detected",
    ),
    (
        "strict_achlist_scope",
        "# true enables strict cross-script resolution and conflicting-script-versions\n\
         # checks for only the achlist's listed entries. false (the default) keeps\n\
         # parent-directory search roots. In strict mode, every .psc dependency must\n\
         # be listed in the achlist",
    ),
    ("semicolon", "# true, false"),
    ("indentation", "# tab, space"),
    (
        "indentation_width",
        "# Non-negative integer; used only when indentation is space",
    ),
    (
        "identifier_casing",
        "# camelCase, PascalCase, snake_case, CONSTANT_CASE",
    ),
    ("cyclomatic_complexity_warning", "# Non-negative integer"),
    ("cyclomatic_complexity_error", "# Non-negative integer"),
    (
        "type_casing",
        "# PascalCase, camelCase, lowercase, UPPERCASE",
    ),
    ("named_arguments", "# always, instead_of_defaults, never"),
    ("min_wait_interval", "# Non-negative number"),
    ("magic_numbers", "# loose, strict"),
    ("fail_on_warning", "# true, false"),
    ("fail_on_info", "# true, false"),
    ("bool_like_int", "# true, false"),
    (
        "assume_auto_properties_filled",
        "# true, false; true treats an Auto/AutoReadOnly property as already\n\
         # filled in by the time a function runs instead of possibly None",
    ),
    ("rules", "# Each rule accepts true or false"),
];

/// Inserts [`FIELD_COMMENTS`] above their matching top-level key in `yaml`.
/// Only unindented `key:` lines are matched, so the `rules:` block's own
/// nested keys are left alone, matching the single comment the README
/// shows above `rules:` itself rather than one per rule.
fn with_field_comments(yaml: &str) -> String {
    let mut out = String::with_capacity(yaml.len() + FIELD_COMMENTS.len() * 32);
    for line in yaml.lines() {
        if !line.starts_with(' ') {
            if let Some(key) = line.split(':').next() {
                if let Some((_, comment)) = FIELD_COMMENTS.iter().find(|(name, _)| *name == key) {
                    out.push_str(comment);
                    out.push('\n');
                }
            }
        }
        out.push_str(line);
        out.push('\n');
    }
    out
}

/// Writes `project` to `dir`'s papyrus-lint YAML config file. Overwrites
/// whichever candidate name (`papyrus-lint.yaml`/`.yml`) already exists in
/// `dir`, or creates `papyrus-lint.yaml` if `dir` has neither yet. Each
/// top-level key is preceded by the same explanatory comment the README
/// shows for it (see [`FIELD_COMMENTS`]).
fn save_project_file(dir: &Path, project: &ProjectFile) -> Result<(), String> {
    let path = existing_config_path(dir).unwrap_or_else(|| dir.join(CONFIG_FILE_NAMES[0]));
    save_project_file_at(&path, project)
}

/// Writes `project` to the exact file at `path`, creating it if it doesn't
/// exist yet. Shared by [`save_project_file`] (which first resolves `path`
/// from a project directory) and [`save_config_at_path`] (which targets an
/// explicit file directly).
fn save_project_file_at(path: &Path, project: &ProjectFile) -> Result<(), String> {
    let yaml = serde_yaml::to_string(project).map_err(|err| err.to_string())?;
    fs::write(path, with_field_comments(&yaml)).map_err(|err| err.to_string())
}

/// Directory next to the CLI's own running executable, if it can be
/// determined. [`initialize_default_config`] looks here for an optional
/// shared base config, and [`user_presets_dir`] for an optional user
/// presets directory.
fn executable_dir() -> Option<PathBuf> {
    std::env::current_exe()
        .ok()
        .and_then(|path| path.parent().map(Path::to_path_buf))
}

/// Serializes the app-only (non-lint) settings of `project`, always
/// including every field regardless of [`ProjectFile`]'s `skip_serializing_if`
/// attributes, so an initialized file serves as a complete, discoverable
/// template rather than omitting whichever settings happen to match their
/// default.
fn non_lint_yaml(project: &ProjectFile) -> Result<String, String> {
    #[derive(Serialize)]
    struct NonLintFields<'a> {
        compiler_path: &'a Option<String>,
        additional_script_roots: &'a [String],
        compile_check: bool,
        strict_achlist_scope: bool,
    }

    serde_yaml::to_string(&NonLintFields {
        compiler_path: &project.compiler_path,
        additional_script_roots: &project.additional_script_roots,
        compile_check: project.compile_check,
        strict_achlist_scope: project.strict_achlist_scope,
    })
    .map_err(|err| err.to_string())
}

/// A named baseline `init` can generate `papyrus-lint.yaml` from, selected
/// via the CLI's `--preset <name>` flag (see [`Preset::parse`]). See
/// `docs/presets/` for each built-in preset's own annotated YAML and the
/// reasoning behind what it turns on/off relative to the others.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum Preset {
    /// Everything on, including pure style/naming nits. Identical to the
    /// engine's built-in defaults (`docs/papyrus-lint.default.yaml`), so
    /// plain `init` (no `--preset`) behaves exactly as it did before
    /// presets existed.
    #[default]
    Strict,
    /// Keeps every rule with a real correctness/performance stake, plus
    /// the cheap, auto-fixable formatting rules; turns off naming/style and
    /// purely informational/advisory rules.
    Standard,
    /// Only rules tagged `medium`/`high` importance stay on, and cyclomatic
    /// complexity thresholds are relaxed — meant for a quiet first pass
    /// over an unfamiliar or legacy codebase.
    Careful,
    /// A user-defined preset, named after a `<name>.yaml`/`.yml` file found
    /// under a `presets` directory next to the running executable (see
    /// [`user_presets_dir`]). [`Preset::parse`] accepts any name that isn't
    /// one of the three built-ins above as this variant without checking
    /// the filesystem yet; [`Preset::yaml`] is where a name that doesn't
    /// actually match a file there is finally rejected, since only there is
    /// the executable-adjacent base directory available.
    Custom(String),
}

/// The names [`Preset::parse`] accepts, in the order shown in `--help`/
/// error text.
pub const PRESET_NAMES: [&str; 3] = ["strict", "standard", "careful"];

/// Name of the directory, next to the running executable, that holds
/// optional user-defined preset YAML files. A file named `<name>.yaml` (or
/// `.yml`) there is selectable as `--preset <name>` (CLI) or from the
/// desktop app's first-run preset picker, exactly as if it were a fourth
/// built-in preset.
pub const USER_PRESETS_DIR_NAME: &str = "presets";

impl Preset {
    /// Matches `name` against [`PRESET_NAMES`] case-insensitively. Anything
    /// else non-empty (after trimming) is accepted as [`Self::Custom`],
    /// naming a user preset whose actual existence is only checked once its
    /// YAML is needed (see [`Preset::yaml`]). Returns `None` only for an
    /// empty (or all-whitespace) name, which the CLI's `--preset` parsing
    /// treats as a usage error.
    pub fn parse(name: &str) -> Option<Self> {
        let trimmed = name.trim();
        if trimmed.is_empty() {
            return None;
        }
        match trimmed.to_ascii_lowercase().as_str() {
            "strict" => Some(Self::Strict),
            "standard" => Some(Self::Standard),
            "careful" => Some(Self::Careful),
            _ => Some(Self::Custom(trimmed.to_string())),
        }
    }

    /// This preset's baseline YAML content: for a built-in preset, the
    /// checked-in `docs/presets/papyrus-lint.<preset>.yaml` contents
    /// compiled into the binary; for [`Self::Custom`], the contents of the
    /// matching `<name>.yaml`/`.yml` file under `base_dir`'s
    /// [`USER_PRESETS_DIR_NAME`] directory. Errors if `base_dir` is
    /// unavailable, has no such directory, or it has no file matching
    /// `name`.
    fn yaml(&self, base_dir: Option<&Path>) -> Result<Cow<'static, str>, String> {
        match self {
            Self::Strict => Ok(Cow::Borrowed(include_str!(
                "../../../../docs/presets/papyrus-lint.strict.yaml"
            ))),
            Self::Standard => Ok(Cow::Borrowed(include_str!(
                "../../../../docs/presets/papyrus-lint.standard.yaml"
            ))),
            Self::Careful => Ok(Cow::Borrowed(include_str!(
                "../../../../docs/presets/papyrus-lint.careful.yaml"
            ))),
            Self::Custom(name) => {
                let path = user_presets_dir_under(base_dir)
                    .and_then(|dir| find_user_preset_file(&dir, name));
                match path {
                    Some(path) => fs::read_to_string(&path).map(Cow::Owned).map_err(|err| err.to_string()),
                    None => Err(format!(
                        "unknown preset '{name}' (expected one of: {}, or a matching <name>.yaml/.yml \
                         file in a '{USER_PRESETS_DIR_NAME}' directory next to the executable)",
                        PRESET_NAMES.join(", ")
                    )),
                }
            }
        }
    }
}

/// The user presets directory next to the running executable (see
/// [`USER_PRESETS_DIR_NAME`]), if the executable's location can be
/// determined and it actually has such a directory.
pub fn user_presets_dir() -> Option<PathBuf> {
    user_presets_dir_under(executable_dir().as_deref())
}

/// Same as [`user_presets_dir`], but takes the executable-adjacent
/// directory explicitly rather than assuming it's [`executable_dir`] — the
/// same split used elsewhere in this module (see
/// [`initialize_config_with_base`]) so tests can supply a controlled
/// directory instead of depending on the test binary's own
/// `current_exe()`.
fn user_presets_dir_under(base_dir: Option<&Path>) -> Option<PathBuf> {
    let dir = base_dir?.join(USER_PRESETS_DIR_NAME);
    dir.is_dir().then_some(dir)
}

/// Why [`add_user_preset`] refused to add a user preset.
#[derive(Debug, PartialEq, Eq)]
pub enum AddPresetError {
    /// `name` is blank, or matches a built-in preset name (see
    /// [`PRESET_NAMES`]) case-insensitively — such a name would never
    /// actually be selectable via `--preset <name>`, since [`Preset::parse`]
    /// always resolves a built-in name first.
    InvalidName(String),
    /// A user preset named `name` already exists at this path, and
    /// `overwrite` was `false`.
    AlreadyExists(PathBuf),
    /// The running executable's own directory couldn't be determined, so
    /// there's nowhere to create (or look for) the `presets` directory.
    BaseDirUnavailable,
    /// Failed to read `source_path`, create the `presets` directory, or
    /// write the destination file.
    Io(String),
}

impl std::fmt::Display for AddPresetError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::InvalidName(name) => write!(
                f,
                "'{name}' can't be used as a preset name (it's blank, or matches a built-in \
                 preset: {})",
                PRESET_NAMES.join(", ")
            ),
            Self::AlreadyExists(path) => {
                write!(f, "a preset already exists at {}", path.display())
            }
            Self::BaseDirUnavailable => {
                write!(f, "could not determine the running executable's directory")
            }
            Self::Io(err) => write!(f, "{err}"),
        }
    }
}

/// Adds (or overwrites) a user preset named `name`, copying the contents of
/// `source_path` (an existing `papyrus-lint.yaml`/`.yml`) into the
/// executable-adjacent [`USER_PRESETS_DIR_NAME`] directory (see
/// [`user_presets_dir`]), creating that directory first if it doesn't exist
/// yet. The saved preset then becomes selectable the same way a built-in
/// preset is, via `--preset <name>` (CLI `init`) or the desktop app's preset
/// picker.
///
/// Refuses `name` (see [`AddPresetError::InvalidName`]) if it's blank or
/// matches a built-in preset name case-insensitively.
///
/// If a preset named `name` already exists (matched case-insensitively, as
/// either `.yaml` or `.yml`), this refuses to overwrite it unless
/// `overwrite` is `true`, returning [`AddPresetError::AlreadyExists`] with
/// the existing file's path instead — giving a caller (the CLI's `preset
/// add` command) the chance to make the user confirm before retrying with
/// `overwrite: true`. Overwriting reuses the existing file's
/// own path (and therefore its `.yaml`/`.yml` extension) rather than
/// creating a second file alongside it; a brand new preset is always
/// written as `<name>.yaml`.
pub fn add_user_preset(
    name: &str,
    source_path: &Path,
    overwrite: bool,
) -> Result<PathBuf, AddPresetError> {
    add_user_preset_under(executable_dir().as_deref(), name, source_path, overwrite)
}

/// Same as [`add_user_preset`], but takes the executable-adjacent directory
/// explicitly rather than assuming it's [`executable_dir`] — the same split
/// used elsewhere in this module (see [`initialize_config_with_base`]) so
/// tests can supply a controlled directory instead of depending on the test
/// binary's own `current_exe()`.
fn add_user_preset_under(
    base_dir: Option<&Path>,
    name: &str,
    source_path: &Path,
    overwrite: bool,
) -> Result<PathBuf, AddPresetError> {
    let trimmed = name.trim();
    if trimmed.is_empty()
        || PRESET_NAMES
            .iter()
            .any(|builtin| builtin.eq_ignore_ascii_case(trimmed))
    {
        return Err(AddPresetError::InvalidName(name.to_string()));
    }

    let base_dir = base_dir.ok_or(AddPresetError::BaseDirUnavailable)?;
    let presets_dir = base_dir.join(USER_PRESETS_DIR_NAME);
    fs::create_dir_all(&presets_dir).map_err(|err| AddPresetError::Io(err.to_string()))?;

    let existing = find_user_preset_file(&presets_dir, trimmed);
    if let Some(existing_path) = &existing {
        if !overwrite {
            return Err(AddPresetError::AlreadyExists(existing_path.clone()));
        }
    }
    let target_path = existing.unwrap_or_else(|| presets_dir.join(format!("{trimmed}.yaml")));

    let contents =
        fs::read_to_string(source_path).map_err(|err| AddPresetError::Io(err.to_string()))?;
    fs::write(&target_path, contents).map_err(|err| AddPresetError::Io(err.to_string()))?;

    Ok(target_path)
}

/// Whether `path`'s extension is `yaml`/`yml`, matched case-insensitively —
/// the same two extensions a project's own `papyrus-lint.yaml`/`.yml`
/// supports (see [`CONFIG_FILE_NAMES`]).
fn has_yaml_extension(path: &Path) -> bool {
    path.extension()
        .and_then(|ext| ext.to_str())
        .is_some_and(|ext| ext.eq_ignore_ascii_case("yaml") || ext.eq_ignore_ascii_case("yml"))
}

/// Every user preset name available in `dir` (see [`user_presets_dir`]):
/// each `.yaml`/`.yml` file's own file stem (the name it's selected by),
/// sorted case-insensitively so listings (e.g. the desktop app's preset
/// picker) are stable and predictable. Returns an empty `Vec` if `dir`
/// can't be read at all.
pub fn list_user_preset_names(dir: &Path) -> Vec<String> {
    let Ok(entries) = fs::read_dir(dir) else {
        return Vec::new();
    };
    let mut names: Vec<String> = entries
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .filter(|path| path.is_file() && has_yaml_extension(path))
        .filter_map(|path| {
            path.file_stem()
                .and_then(|stem| stem.to_str())
                .map(str::to_string)
        })
        .collect();
    names.sort_by_key(|name| name.to_ascii_lowercase());
    names
}

/// Finds the `.yaml`/`.yml` file in `dir` whose file stem matches `name`
/// case-insensitively, e.g. `find_user_preset_file(dir, "ABC")` matching a
/// file named `abc.yaml`. Returns `None` if `dir` can't be read or has no
/// such file.
fn find_user_preset_file(dir: &Path, name: &str) -> Option<PathBuf> {
    fs::read_dir(dir)
        .into_iter()
        .flatten()
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .find(|path| {
            path.is_file()
                && has_yaml_extension(path)
                && path
                    .file_stem()
                    .and_then(|stem| stem.to_str())
                    .is_some_and(|stem| stem.eq_ignore_ascii_case(name))
        })
}

/// Saves `config` as a new user preset named `name`, in a `presets`
/// directory next to the running executable (created if it doesn't exist
/// yet), so it becomes selectable afterward exactly like a built-in preset
/// (`--preset <name>`, or the desktop app's "Save current settings as
/// preset" button/first-run picker). Only the lint settings themselves are
/// written, not a project's own `compiler_path`/`additional_script_roots`/
/// `compile_check`/`strict_achlist_scope`, since those are specific to a
/// project rather than something a reusable preset should hardcode.
///
/// Refuses a blank name, and refuses a name matching one of
/// [`PRESET_NAMES`] (case-insensitively), since [`Preset::parse`] always
/// resolves those to a built-in preset first — a same-named file here would
/// be written but never actually selectable. An existing same-named preset
/// (matched case-insensitively, the same way [`find_user_preset_file`]
/// resolves one) is only replaced if `overwrite` is true; otherwise this
/// errors without touching it.
pub fn save_user_preset(
    name: &str,
    config: &papyrus_lints::Config,
    overwrite: bool,
) -> Result<PathBuf, String> {
    save_user_preset_under(executable_dir().as_deref(), name, config, overwrite)
}

/// Same as [`save_user_preset`], but takes the executable-adjacent
/// directory explicitly rather than assuming it's next to the running
/// executable, so tests can exercise it without depending on the test
/// binary's own `current_exe()`.
fn save_user_preset_under(
    base_dir: Option<&Path>,
    name: &str,
    config: &papyrus_lints::Config,
    overwrite: bool,
) -> Result<PathBuf, String> {
    let trimmed = name.trim();
    if trimmed.is_empty() {
        return Err("preset name must not be blank".to_string());
    }
    if PRESET_NAMES
        .iter()
        .any(|built_in| built_in.eq_ignore_ascii_case(trimmed))
    {
        return Err(format!(
            "'{trimmed}' is a built-in preset name and can't be used for a custom preset"
        ));
    }
    let base_dir = base_dir
        .ok_or_else(|| "could not determine the running executable's directory".to_string())?;
    let dir = base_dir.join(USER_PRESETS_DIR_NAME);
    fs::create_dir_all(&dir).map_err(|err| err.to_string())?;

    let path =
        find_user_preset_file(&dir, trimmed).unwrap_or_else(|| dir.join(format!("{trimmed}.yaml")));
    if path.is_file() && !overwrite {
        return Err(format!("a preset named '{trimmed}' already exists"));
    }

    let yaml = papyrus_lints::config::to_yaml(config).map_err(|err| err.to_string())?;
    fs::write(&path, with_field_comments(&yaml)).map_err(|err| err.to_string())?;
    Ok(path)
}

/// Deletes the user preset named `name` from the executable-adjacent
/// [`USER_PRESETS_DIR_NAME`] directory (see [`user_presets_dir`]), for the
/// desktop app's preset management tab. Errors if the executable's
/// directory can't be determined, or no preset named `name` exists there.
pub fn delete_user_preset(name: &str) -> Result<(), String> {
    delete_user_preset_under(executable_dir().as_deref(), name)
}

/// Same as [`delete_user_preset`], but takes the executable-adjacent
/// directory explicitly rather than assuming it's [`executable_dir`], so
/// tests can supply a controlled directory instead of depending on the
/// test binary's own `current_exe()`.
fn delete_user_preset_under(base_dir: Option<&Path>, name: &str) -> Result<(), String> {
    let trimmed = name.trim();
    let base_dir = base_dir
        .ok_or_else(|| "could not determine the running executable's directory".to_string())?;
    let dir = base_dir.join(USER_PRESETS_DIR_NAME);
    let path = find_user_preset_file(&dir, trimmed)
        .ok_or_else(|| format!("no preset named '{trimmed}' exists"))?;
    fs::remove_file(&path).map_err(|err| err.to_string())
}

/// Renames the user preset named `old_name` to `new_name`, in the same
/// executable-adjacent [`USER_PRESETS_DIR_NAME`] directory (see
/// [`user_presets_dir`]) [`save_user_preset`]/[`delete_user_preset`] use,
/// for the desktop app's preset management tab. Refuses `new_name` (the
/// same way [`save_user_preset`] does) if it's blank or matches a built-in
/// preset name case-insensitively, since such a name would never actually
/// be selectable via `--preset <new_name>`. Errors if no preset named
/// `old_name` exists.
///
/// If a preset already exists under `new_name` (matched case-insensitively,
/// as either `.yaml` or `.yml`), this refuses to overwrite it unless
/// `overwrite` is `true` — the same guard [`save_user_preset`] applies,
/// giving a caller the chance to confirm before retrying with
/// `overwrite: true`. Renaming a preset to a name that only differs from
/// its current one by case is a no-op, returning the file's existing path
/// unchanged.
pub fn rename_user_preset(
    old_name: &str,
    new_name: &str,
    overwrite: bool,
) -> Result<PathBuf, String> {
    rename_user_preset_under(executable_dir().as_deref(), old_name, new_name, overwrite)
}

/// Same as [`rename_user_preset`], but takes the executable-adjacent
/// directory explicitly rather than assuming it's [`executable_dir`], so
/// tests can supply a controlled directory instead of depending on the
/// test binary's own `current_exe()`.
fn rename_user_preset_under(
    base_dir: Option<&Path>,
    old_name: &str,
    new_name: &str,
    overwrite: bool,
) -> Result<PathBuf, String> {
    let old_trimmed = old_name.trim();
    let new_trimmed = new_name.trim();
    if new_trimmed.is_empty() {
        return Err("preset name must not be blank".to_string());
    }
    if PRESET_NAMES
        .iter()
        .any(|built_in| built_in.eq_ignore_ascii_case(new_trimmed))
    {
        return Err(format!(
            "'{new_trimmed}' is a built-in preset name and can't be used for a custom preset"
        ));
    }

    let base_dir = base_dir
        .ok_or_else(|| "could not determine the running executable's directory".to_string())?;
    let dir = base_dir.join(USER_PRESETS_DIR_NAME);
    let old_path = find_user_preset_file(&dir, old_trimmed)
        .ok_or_else(|| format!("no preset named '{old_trimmed}' exists"))?;

    if old_trimmed.eq_ignore_ascii_case(new_trimmed) {
        return Ok(old_path);
    }

    let extension = old_path
        .extension()
        .and_then(|ext| ext.to_str())
        .unwrap_or("yaml");
    let target_path = find_user_preset_file(&dir, new_trimmed)
        .unwrap_or_else(|| dir.join(format!("{new_trimmed}.{extension}")));
    if target_path.is_file() && !overwrite {
        return Err(format!("a preset named '{new_trimmed}' already exists"));
    }

    fs::rename(&old_path, &target_path).map_err(|err| err.to_string())?;
    Ok(target_path)
}

/// Reads a user preset's own YAML content verbatim, for exporting it (e.g.
/// the desktop app's preset management tab) without merging it against a
/// project's own settings the way [`initialize_default_config`] does.
/// Errors if no preset named `name` exists under the executable-adjacent
/// [`USER_PRESETS_DIR_NAME`] directory.
pub fn read_user_preset_yaml(name: &str) -> Result<String, String> {
    read_user_preset_yaml_under(executable_dir().as_deref(), name)
}

/// Same as [`read_user_preset_yaml`], but takes the executable-adjacent
/// directory explicitly rather than assuming it's [`executable_dir`], so
/// tests can supply a controlled directory instead of depending on the
/// test binary's own `current_exe()`.
fn read_user_preset_yaml_under(base_dir: Option<&Path>, name: &str) -> Result<String, String> {
    Preset::Custom(name.trim().to_string())
        .yaml(base_dir)
        .map(Cow::into_owned)
}

/// Deep-merges `over` onto `base`: a `Mapping` present in both merges key by
/// key (recursively, so `rules:`'s own nested keys merge independently
/// rather than one `rules:` block replacing the other outright), and
/// anything else in `over` replaces `base`'s value for that key entirely.
/// Used to layer an executable-adjacent base config over a selected
/// [`Preset`]'s own YAML, the same key-by-key override semantics a project's
/// own `papyrus-lint.yaml` already gets over the engine's built-in defaults.
fn deep_merge(base: serde_yaml::Value, over: serde_yaml::Value) -> serde_yaml::Value {
    match (base, over) {
        (serde_yaml::Value::Mapping(mut base_map), serde_yaml::Value::Mapping(over_map)) => {
            for (key, value) in over_map {
                let merged = match base_map.remove(&key) {
                    Some(base_value) => deep_merge(base_value, value),
                    None => value,
                };
                base_map.insert(key, merged);
            }
            serde_yaml::Value::Mapping(base_map)
        }
        (_, over) => over,
    }
}

/// Creates `papyrus-lint.yaml` in `dir` from `preset`'s baseline
/// configuration. Refuses to replace either supported config filename, so
/// an existing project configuration cannot be lost accidentally. If
/// `preset` is [`Preset::Custom`], its YAML is read from the matching file
/// under the executable-adjacent [`USER_PRESETS_DIR_NAME`] directory (see
/// [`Preset::yaml`]), erroring out if none matches.
///
/// If a `papyrus-lint.yaml`/`.yml` file exists next to the running
/// executable, it's layered on top of `preset` instead of the engine's
/// built-in defaults: any setting it specifies overrides the preset's own,
/// and any setting it leaves out still falls back to the preset. This lets
/// someone define their own baseline settings once, next to wherever they
/// keep the CLI (or desktop app) binary, and reuse it across every project
/// they run `init` in — on top of whichever preset they pick each time —
/// rather than hand-editing each newly generated file the same way
/// afterward.
pub fn initialize_default_config(dir: &Path, preset: Preset) -> Result<PathBuf, String> {
    initialize_config_with_base(dir, executable_dir().as_deref(), preset)
}

/// Same as [`initialize_default_config`], but takes the directory to look
/// for the optional shared base config (and, for a [`Preset::Custom`]
/// preset, the `presets` directory) in explicitly, rather than assuming
/// it's next to the running executable. Split out so tests can exercise the
/// merge behavior without depending on `std::env::current_exe()`.
fn initialize_config_with_base(
    dir: &Path,
    base_dir: Option<&Path>,
    preset: Preset,
) -> Result<PathBuf, String> {
    if let Some(path) = existing_config_path(dir) {
        return Err(format!("config already exists at {}", path.display()));
    }

    let preset_yaml = preset.yaml(base_dir)?;
    let preset_value: serde_yaml::Value =
        serde_yaml::from_str(&preset_yaml).map_err(|err| err.to_string())?;

    let merged_value = match base_dir.and_then(existing_config_path) {
        Some(base_path) => {
            let contents = fs::read_to_string(&base_path).map_err(|err| err.to_string())?;
            if contents.trim().is_empty() {
                preset_value
            } else {
                let override_value: serde_yaml::Value =
                    serde_yaml::from_str(&contents).map_err(|err| err.to_string())?;
                deep_merge(preset_value, override_value)
            }
        }
        None => preset_value,
    };

    let base: ProjectFile = serde_yaml::from_value(merged_value).map_err(|err| err.to_string())?;

    let path = dir.join(CONFIG_FILE_NAMES[0]);
    let lint_yaml = papyrus_lints::config::to_yaml(&base.lint).map_err(|err| err.to_string())?;
    let yaml = format!("{}{lint_yaml}", non_lint_yaml(&base)?);
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&path)
        .map_err(|err| err.to_string())?;
    file.write_all(with_field_comments(&yaml).as_bytes())
        .map_err(|err| err.to_string())?;
    Ok(path)
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
    if contents.trim().is_empty() {
        return Ok(papyrus_lints::Config::default());
    }
    let project: ProjectFile = serde_yaml::from_str(&contents).map_err(|err| err.to_string())?;
    Ok(project.lint)
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
        let contents = fs::read_to_string(path).map_err(|err| err.to_string())?;
        if contents.trim().is_empty() {
            ProjectFile::default()
        } else {
            serde_yaml::from_str(&contents).map_err(|err| err.to_string())?
        }
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
    let project: ProjectFile = serde_yaml::from_str(&contents).map_err(|err| err.to_string())?;
    Ok(project.strict_achlist_scope)
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

/// Looks for `PapyrusCompiler.exe` under a `Papyrus Compiler` directory one
/// level above `dir` (a project's `.achlist` directory) — the layout used
/// by Bethesda's Creation Kit tooling, where a game's `Data` directory
/// (typically where a project's `.achlist` lives) sits alongside a
/// `Papyrus Compiler` directory in the game's install root. Returns `None`
/// if `dir` has no parent or the executable isn't found there.
pub fn auto_detect_compiler_path(dir: &Path) -> Option<PathBuf> {
    let candidate = dir
        .parent()?
        .join(COMPILER_AUTO_DETECT_DIR_NAME)
        .join(COMPILER_EXECUTABLE_NAME);
    candidate.is_file().then_some(candidate)
}

/// Resolves the PapyrusCompiler.exe path to use for `dir`'s project: an
/// explicit override from its papyrus-lint config file, or, absent one,
/// an auto-detected path (see [`auto_detect_compiler_path`]). Returns
/// `None` if neither is available.
pub fn resolve_compiler_path(dir: &Path) -> Result<Option<String>, String> {
    if let Some(path) = load_compiler_path(dir)? {
        return Ok(Some(path));
    }

    Ok(auto_detect_compiler_path(dir).map(|path| path.to_string_lossy().into_owned()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use papyrus_lints::config::Indentation;

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

        assert_eq!(
            generated, docs_copy,
            "docs/papyrus-lint.default.yaml is out of date; regenerate it with `PapyrusLinterCLI init`"
        );
    }

    #[test]
    fn default_preset_is_strict() {
        assert_eq!(Preset::default(), Preset::Strict);
    }

    #[test]
    fn preset_parse_matches_built_ins_case_insensitively_and_rejects_only_blank_names() {
        assert_eq!(Preset::parse("strict"), Some(Preset::Strict));
        assert_eq!(Preset::parse("STANDARD"), Some(Preset::Standard));
        assert_eq!(Preset::parse("Careful"), Some(Preset::Careful));
        assert_eq!(Preset::parse("   "), None);
        assert_eq!(Preset::parse(""), None);
    }

    #[test]
    fn preset_parse_treats_any_other_name_as_a_custom_preset() {
        assert_eq!(
            Preset::parse("lenient"),
            Some(Preset::Custom("lenient".to_string()))
        );
        assert_eq!(
            Preset::parse("  my-preset  "),
            Some(Preset::Custom("my-preset".to_string()))
        );
    }

    #[test]
    fn every_built_in_preset_yaml_parses_into_a_project_file() {
        for preset in [Preset::Strict, Preset::Standard, Preset::Careful] {
            let yaml = preset.yaml(None).expect("built-in preset should resolve");
            serde_yaml::from_str::<ProjectFile>(&yaml)
                .unwrap_or_else(|err| panic!("{preset:?} preset failed to parse: {err}"));
        }
    }

    #[test]
    fn custom_preset_yaml_errors_when_no_presets_dir_exists() {
        let dir = tempfile::tempdir().expect("failed to create temp dir");

        let error = Preset::Custom("abc".to_string())
            .yaml(Some(dir.path()))
            .expect_err("should fail without a presets directory");

        assert!(error.contains("unknown preset 'abc'"));
    }

    #[test]
    fn custom_preset_yaml_errors_when_no_matching_file_exists() {
        let base_dir = tempfile::tempdir().expect("failed to create temp dir");
        fs::create_dir(base_dir.path().join(USER_PRESETS_DIR_NAME))
            .expect("failed to create presets dir");

        let error = Preset::Custom("abc".to_string())
            .yaml(Some(base_dir.path()))
            .expect_err("should fail without a matching preset file");

        assert!(error.contains("unknown preset 'abc'"));
    }

    #[test]
    fn custom_preset_yaml_reads_the_matching_file_case_insensitively() {
        let base_dir = tempfile::tempdir().expect("failed to create temp dir");
        let presets_dir = base_dir.path().join(USER_PRESETS_DIR_NAME);
        fs::create_dir(&presets_dir).expect("failed to create presets dir");
        write_config(&presets_dir, "abc.yaml", "semicolon: true\n");

        let yaml = Preset::Custom("ABC".to_string())
            .yaml(Some(base_dir.path()))
            .expect("should find abc.yaml case-insensitively");

        assert_eq!(yaml.as_ref(), "semicolon: true\n");
    }

    #[test]
    fn custom_preset_yaml_supports_uppercase_yml_extensions() {
        let base_dir = tempfile::tempdir().expect("failed to create temp dir");
        let presets_dir = base_dir.path().join(USER_PRESETS_DIR_NAME);
        fs::create_dir(&presets_dir).expect("failed to create presets dir");
        write_config(&presets_dir, "team-style.YML", "semicolon: true\n");

        let yaml = Preset::Custom("team-style".to_string())
            .yaml(Some(base_dir.path()))
            .expect("uppercase YML preset should resolve");

        assert_eq!(yaml.as_ref(), "semicolon: true\n");
    }

    #[test]
    fn save_user_preset_rejects_a_blank_name() {
        let error = save_user_preset_under(None, "   ", &papyrus_lints::Config::default(), false)
            .expect_err("blank name should be rejected");

        assert!(error.contains("must not be blank"));
    }

    #[test]
    fn save_user_preset_rejects_built_in_preset_names_case_insensitively() {
        let base_dir = tempfile::tempdir().expect("failed to create temp dir");

        let error = save_user_preset_under(
            Some(base_dir.path()),
            "STRICT",
            &papyrus_lints::Config::default(),
            false,
        )
        .expect_err("built-in preset name should be rejected");

        assert!(error.contains("built-in preset name"));
        assert!(!base_dir.path().join(USER_PRESETS_DIR_NAME).exists());
    }

    #[test]
    fn save_user_preset_errors_without_a_resolvable_base_dir() {
        let error =
            save_user_preset_under(None, "my-preset", &papyrus_lints::Config::default(), false)
                .expect_err("should fail without a base dir");

        assert!(error.contains("executable's directory"));
    }

    #[test]
    fn save_user_preset_creates_the_presets_directory_and_writes_the_file() {
        let base_dir = tempfile::tempdir().expect("failed to create temp dir");
        let config = papyrus_lints::Config {
            semicolon: true,
            ..papyrus_lints::Config::default()
        };

        let path = save_user_preset_under(Some(base_dir.path()), "my-preset", &config, false)
            .expect("saving a new preset should succeed");

        assert_eq!(
            path,
            base_dir
                .path()
                .join(USER_PRESETS_DIR_NAME)
                .join("my-preset.yaml")
        );
        let yaml = Preset::Custom("my-preset".to_string())
            .yaml(Some(base_dir.path()))
            .expect("saved preset should resolve");
        let saved: papyrus_lints::Config =
            serde_yaml::from_str(&yaml).expect("saved preset should parse as a lint config");
        assert_eq!(saved, config);
    }

    #[test]
    fn save_user_preset_trims_the_name_used_for_the_file() {
        let base_dir = tempfile::tempdir().expect("failed to create temp dir");

        let path = save_user_preset_under(
            Some(base_dir.path()),
            "  team-style  ",
            &papyrus_lints::Config::default(),
            false,
        )
        .expect("saving a trimmed preset name should succeed");

        assert_eq!(
            path.file_name().and_then(|name| name.to_str()),
            Some("team-style.yaml")
        );
        assert_eq!(
            list_user_preset_names(&base_dir.path().join(USER_PRESETS_DIR_NAME)),
            vec!["team-style".to_string()]
        );
    }

    #[test]
    fn save_user_preset_refuses_to_overwrite_without_the_flag() {
        let base_dir = tempfile::tempdir().expect("failed to create temp dir");
        save_user_preset_under(
            Some(base_dir.path()),
            "my-preset",
            &papyrus_lints::Config::default(),
            false,
        )
        .expect("first save should succeed");

        let error = save_user_preset_under(
            Some(base_dir.path()),
            "my-preset",
            &papyrus_lints::Config::default(),
            false,
        )
        .expect_err("saving over an existing preset should fail without overwrite");

        assert!(error.contains("already exists"));
    }

    #[test]
    fn save_user_preset_overwrites_an_existing_preset_case_insensitively_when_allowed() {
        let base_dir = tempfile::tempdir().expect("failed to create temp dir");
        save_user_preset_under(
            Some(base_dir.path()),
            "My-Preset",
            &papyrus_lints::Config::default(),
            false,
        )
        .expect("first save should succeed");
        let config = papyrus_lints::Config {
            semicolon: true,
            ..papyrus_lints::Config::default()
        };

        let path = save_user_preset_under(Some(base_dir.path()), "my-preset", &config, true)
            .expect("overwrite should succeed");

        // The differently-cased existing file is reused rather than a second
        // one being created alongside it.
        assert_eq!(
            path,
            base_dir
                .path()
                .join(USER_PRESETS_DIR_NAME)
                .join("My-Preset.yaml")
        );
        let entries: Vec<_> = fs::read_dir(base_dir.path().join(USER_PRESETS_DIR_NAME))
            .expect("failed to read presets dir")
            .filter_map(Result::ok)
            .collect();
        assert_eq!(entries.len(), 1);
        let saved: papyrus_lints::Config = serde_yaml::from_str(
            &Preset::Custom("my-preset".to_string())
                .yaml(Some(base_dir.path()))
                .expect("saved preset should resolve"),
        )
        .expect("saved preset should parse as a lint config");
        assert_eq!(saved, config);
    }

    #[test]
    fn delete_user_preset_removes_the_matching_file_case_insensitively() {
        let base_dir = tempfile::tempdir().expect("failed to create temp dir");
        let presets_dir = base_dir.path().join(USER_PRESETS_DIR_NAME);
        fs::create_dir(&presets_dir).expect("failed to create presets dir");
        write_config(&presets_dir, "My-Preset.yaml", "semicolon: true\n");

        delete_user_preset_under(Some(base_dir.path()), "my-preset")
            .expect("deleting an existing preset should succeed");

        assert!(list_user_preset_names(&presets_dir).is_empty());
    }

    #[test]
    fn delete_user_preset_errors_for_an_unknown_preset() {
        let base_dir = tempfile::tempdir().expect("failed to create temp dir");

        let error = delete_user_preset_under(Some(base_dir.path()), "missing")
            .expect_err("deleting a missing preset should fail");

        assert!(error.contains("no preset named 'missing' exists"));
    }

    #[test]
    fn delete_user_preset_errors_without_a_resolvable_base_dir() {
        let error = delete_user_preset_under(None, "my-preset")
            .expect_err("should fail without a base dir");

        assert!(error.contains("executable's directory"));
    }

    #[test]
    fn rename_user_preset_renames_the_matching_file() {
        let base_dir = tempfile::tempdir().expect("failed to create temp dir");
        let presets_dir = base_dir.path().join(USER_PRESETS_DIR_NAME);
        fs::create_dir(&presets_dir).expect("failed to create presets dir");
        write_config(&presets_dir, "old-name.yaml", "semicolon: true\n");

        let path = rename_user_preset_under(Some(base_dir.path()), "old-name", "new-name", false)
            .expect("renaming should succeed");

        assert_eq!(path, presets_dir.join("new-name.yaml"));
        assert_eq!(
            list_user_preset_names(&presets_dir),
            vec!["new-name".to_string()]
        );
        assert_eq!(fs::read_to_string(&path).unwrap(), "semicolon: true\n");
    }

    #[test]
    fn rename_user_preset_preserves_the_original_files_extension() {
        let base_dir = tempfile::tempdir().expect("failed to create temp dir");
        let presets_dir = base_dir.path().join(USER_PRESETS_DIR_NAME);
        fs::create_dir(&presets_dir).expect("failed to create presets dir");
        write_config(&presets_dir, "old-name.yml", "semicolon: true\n");

        let path = rename_user_preset_under(Some(base_dir.path()), "old-name", "new-name", false)
            .expect("renaming should succeed");

        assert_eq!(path, presets_dir.join("new-name.yml"));
    }

    #[test]
    fn rename_user_preset_is_a_no_op_when_only_case_differs() {
        let base_dir = tempfile::tempdir().expect("failed to create temp dir");
        let presets_dir = base_dir.path().join(USER_PRESETS_DIR_NAME);
        fs::create_dir(&presets_dir).expect("failed to create presets dir");
        write_config(&presets_dir, "my-preset.yaml", "semicolon: true\n");

        let path = rename_user_preset_under(Some(base_dir.path()), "my-preset", "My-Preset", false)
            .expect("a case-only rename should succeed");

        assert_eq!(path, presets_dir.join("my-preset.yaml"));
    }

    #[test]
    fn rename_user_preset_errors_for_an_unknown_source_preset() {
        let base_dir = tempfile::tempdir().expect("failed to create temp dir");

        let error = rename_user_preset_under(Some(base_dir.path()), "missing", "new-name", false)
            .expect_err("renaming a missing preset should fail");

        assert!(error.contains("no preset named 'missing' exists"));
    }

    #[test]
    fn rename_user_preset_rejects_a_blank_new_name() {
        let base_dir = tempfile::tempdir().expect("failed to create temp dir");
        let presets_dir = base_dir.path().join(USER_PRESETS_DIR_NAME);
        fs::create_dir(&presets_dir).expect("failed to create presets dir");
        write_config(&presets_dir, "old-name.yaml", "semicolon: true\n");

        let error = rename_user_preset_under(Some(base_dir.path()), "old-name", "   ", false)
            .expect_err("blank new name should be rejected");

        assert!(error.contains("must not be blank"));
    }

    #[test]
    fn rename_user_preset_rejects_a_built_in_preset_name_case_insensitively() {
        let base_dir = tempfile::tempdir().expect("failed to create temp dir");
        let presets_dir = base_dir.path().join(USER_PRESETS_DIR_NAME);
        fs::create_dir(&presets_dir).expect("failed to create presets dir");
        write_config(&presets_dir, "old-name.yaml", "semicolon: true\n");

        let error = rename_user_preset_under(Some(base_dir.path()), "old-name", "STRICT", false)
            .expect_err("built-in preset name should be rejected");

        assert!(error.contains("built-in preset name"));
    }

    #[test]
    fn rename_user_preset_refuses_to_overwrite_without_the_flag() {
        let base_dir = tempfile::tempdir().expect("failed to create temp dir");
        let presets_dir = base_dir.path().join(USER_PRESETS_DIR_NAME);
        fs::create_dir(&presets_dir).expect("failed to create presets dir");
        write_config(&presets_dir, "old-name.yaml", "semicolon: true\n");
        write_config(&presets_dir, "new-name.yaml", "semicolon: false\n");

        let error = rename_user_preset_under(Some(base_dir.path()), "old-name", "new-name", false)
            .expect_err("renaming over an existing preset should fail without overwrite");

        assert!(error.contains("already exists"));
    }

    #[test]
    fn rename_user_preset_overwrites_an_existing_preset_when_allowed() {
        let base_dir = tempfile::tempdir().expect("failed to create temp dir");
        let presets_dir = base_dir.path().join(USER_PRESETS_DIR_NAME);
        fs::create_dir(&presets_dir).expect("failed to create presets dir");
        write_config(&presets_dir, "old-name.yaml", "semicolon: true\n");
        write_config(&presets_dir, "new-name.yaml", "semicolon: false\n");

        let path = rename_user_preset_under(Some(base_dir.path()), "old-name", "new-name", true)
            .expect("overwrite should succeed");

        assert_eq!(fs::read_to_string(&path).unwrap(), "semicolon: true\n");
        assert_eq!(
            list_user_preset_names(&presets_dir),
            vec!["new-name".to_string()]
        );
    }

    #[test]
    fn read_user_preset_yaml_returns_the_files_raw_contents() {
        let base_dir = tempfile::tempdir().expect("failed to create temp dir");
        let presets_dir = base_dir.path().join(USER_PRESETS_DIR_NAME);
        fs::create_dir(&presets_dir).expect("failed to create presets dir");
        write_config(&presets_dir, "my-preset.yaml", "semicolon: true\n");

        let yaml = read_user_preset_yaml_under(Some(base_dir.path()), "my-preset")
            .expect("reading an existing preset should succeed");

        assert_eq!(yaml, "semicolon: true\n");
    }

    #[test]
    fn read_user_preset_yaml_errors_for_an_unknown_preset() {
        let base_dir = tempfile::tempdir().expect("failed to create temp dir");

        let error = read_user_preset_yaml_under(Some(base_dir.path()), "missing")
            .expect_err("reading a missing preset should fail");

        assert!(error.contains("unknown preset 'missing'"));
    }

    #[test]
    fn list_user_preset_names_lists_yaml_and_yml_stems_sorted_case_insensitively() {
        let dir = tempfile::tempdir().expect("failed to create temp dir");
        write_config(dir.path(), "Zebra.yaml", "");
        write_config(dir.path(), "abc.yml", "");
        write_config(dir.path(), "not-a-preset.txt", "");

        assert_eq!(
            list_user_preset_names(dir.path()),
            vec!["abc".to_string(), "Zebra".to_string()]
        );
    }

    #[test]
    fn list_user_preset_names_returns_empty_for_a_missing_directory() {
        let dir = tempfile::tempdir().expect("failed to create temp dir");

        assert_eq!(
            list_user_preset_names(&dir.path().join("does-not-exist")),
            Vec::<String>::new()
        );
    }

    #[test]
    fn add_user_preset_creates_the_presets_dir_and_copies_the_source_file() {
        let base_dir = tempfile::tempdir().expect("failed to create temp dir");
        let source = base_dir.path().join("source.yaml");
        fs::write(&source, "semicolon: true\n").expect("failed to write source file");

        let path = add_user_preset_under(Some(base_dir.path()), "my-team", &source, false)
            .expect("adding a new preset should succeed");

        assert_eq!(
            path,
            base_dir
                .path()
                .join(USER_PRESETS_DIR_NAME)
                .join("my-team.yaml")
        );
        assert_eq!(fs::read_to_string(&path).unwrap(), "semicolon: true\n");
        assert_eq!(
            list_user_preset_names(&base_dir.path().join(USER_PRESETS_DIR_NAME)),
            vec!["my-team".to_string()]
        );
    }

    #[test]
    fn add_user_preset_refuses_to_overwrite_an_existing_preset_without_confirmation() {
        let base_dir = tempfile::tempdir().expect("failed to create temp dir");
        let presets_dir = base_dir.path().join(USER_PRESETS_DIR_NAME);
        fs::create_dir(&presets_dir).expect("failed to create presets dir");
        write_config(&presets_dir, "my-team.yaml", "semicolon: true\n");
        let existing_path = presets_dir.join("my-team.yaml");
        let source = base_dir.path().join("source.yaml");
        fs::write(&source, "semicolon: false\n").expect("failed to write source file");

        let error = add_user_preset_under(Some(base_dir.path()), "my-team", &source, false)
            .expect_err("should refuse to overwrite without confirmation");

        assert_eq!(error, AddPresetError::AlreadyExists(existing_path.clone()));
        assert_eq!(
            fs::read_to_string(&existing_path).unwrap(),
            "semicolon: true\n"
        );
    }

    #[test]
    fn add_user_preset_overwrites_an_existing_preset_when_confirmed() {
        let base_dir = tempfile::tempdir().expect("failed to create temp dir");
        let presets_dir = base_dir.path().join(USER_PRESETS_DIR_NAME);
        fs::create_dir(&presets_dir).expect("failed to create presets dir");
        // The existing file is a `.yml`, so overwriting should reuse that
        // same path/extension rather than also creating a `.yaml` file.
        write_config(&presets_dir, "my-team.yml", "semicolon: true\n");
        let existing_path = presets_dir.join("my-team.yml");
        let source = base_dir.path().join("source.yaml");
        fs::write(&source, "semicolon: false\n").expect("failed to write source file");

        let path = add_user_preset_under(Some(base_dir.path()), "my-team", &source, true)
            .expect("overwriting with confirmation should succeed");

        assert_eq!(path, existing_path);
        assert_eq!(fs::read_to_string(&path).unwrap(), "semicolon: false\n");
        assert_eq!(
            list_user_preset_names(&presets_dir),
            vec!["my-team".to_string()]
        );
    }

    #[test]
    fn add_user_preset_rejects_a_blank_name() {
        let base_dir = tempfile::tempdir().expect("failed to create temp dir");
        let source = base_dir.path().join("source.yaml");
        fs::write(&source, "semicolon: true\n").expect("failed to write source file");

        let error = add_user_preset_under(Some(base_dir.path()), "   ", &source, false)
            .expect_err("a blank name should be rejected");

        assert_eq!(error, AddPresetError::InvalidName("   ".to_string()));
    }

    #[test]
    fn add_user_preset_rejects_a_name_matching_a_built_in_preset() {
        let base_dir = tempfile::tempdir().expect("failed to create temp dir");
        let source = base_dir.path().join("source.yaml");
        fs::write(&source, "semicolon: true\n").expect("failed to write source file");

        let error = add_user_preset_under(Some(base_dir.path()), "STRICT", &source, false)
            .expect_err("a name matching a built-in preset should be rejected");

        assert_eq!(error, AddPresetError::InvalidName("STRICT".to_string()));
    }

    #[test]
    fn add_user_preset_errors_when_the_source_file_does_not_exist() {
        let base_dir = tempfile::tempdir().expect("failed to create temp dir");

        let error = add_user_preset_under(
            Some(base_dir.path()),
            "my-team",
            &base_dir.path().join("does-not-exist.yaml"),
            false,
        )
        .expect_err("a missing source file should be reported as an error");

        assert!(matches!(error, AddPresetError::Io(_)));
    }

    #[test]
    fn add_user_preset_errors_without_a_base_dir() {
        let source = tempfile::tempdir()
            .expect("failed to create temp dir")
            .path()
            .join("source.yaml");

        let error = add_user_preset_under(None, "my-team", &source, false)
            .expect_err("a missing base dir should be reported as an error");

        assert_eq!(error, AddPresetError::BaseDirUnavailable);
    }

    #[test]
    fn init_generates_a_config_from_a_custom_preset_under_the_base_dir() {
        let dir = tempfile::tempdir().expect("failed to create temp dir");
        let base_dir = tempfile::tempdir().expect("failed to create temp dir");
        let presets_dir = base_dir.path().join(USER_PRESETS_DIR_NAME);
        fs::create_dir(&presets_dir).expect("failed to create presets dir");
        write_config(&presets_dir, "abc.yaml", "semicolon: true\n");

        let path = initialize_config_with_base(
            dir.path(),
            Some(base_dir.path()),
            Preset::Custom("abc".to_string()),
        )
        .expect("init should succeed from a custom preset");
        let generated = fs::read_to_string(&path).expect("failed to read generated config");

        assert!(generated.contains("semicolon: true\n"));
    }

    #[test]
    fn init_matches_a_custom_preset_name_case_insensitively() {
        let dir = tempfile::tempdir().expect("failed to create temp dir");
        let base_dir = tempfile::tempdir().expect("failed to create temp dir");
        let presets_dir = base_dir.path().join(USER_PRESETS_DIR_NAME);
        fs::create_dir(&presets_dir).expect("failed to create presets dir");
        write_config(&presets_dir, "Team-Style.yml", "semicolon: true\n");

        let path = initialize_config_with_base(
            dir.path(),
            Some(base_dir.path()),
            Preset::Custom("team-style".to_string()),
        )
        .expect("init should resolve a differently cased custom preset name");

        assert_eq!(
            load_config_from_path(&path).expect("generated config should parse"),
            papyrus_lints::Config {
                semicolon: true,
                ..papyrus_lints::Config::default()
            }
        );
    }

    #[test]
    fn invalid_custom_preset_does_not_leave_a_partial_project_config() {
        let dir = tempfile::tempdir().expect("failed to create temp dir");
        let base_dir = tempfile::tempdir().expect("failed to create temp dir");
        let presets_dir = base_dir.path().join(USER_PRESETS_DIR_NAME);
        fs::create_dir(&presets_dir).expect("failed to create presets dir");
        write_config(&presets_dir, "broken.yaml", "semicolon: [not a bool\n");

        let error = initialize_config_with_base(
            dir.path(),
            Some(base_dir.path()),
            Preset::Custom("broken".to_string()),
        )
        .expect_err("invalid custom preset YAML should be rejected");

        assert!(!error.is_empty());
        assert_eq!(config_file_path(dir.path()), None);
    }

    #[test]
    fn init_reports_an_error_for_an_unknown_custom_preset() {
        let dir = tempfile::tempdir().expect("failed to create temp dir");
        let base_dir = tempfile::tempdir().expect("failed to create temp dir");

        let error = initialize_config_with_base(
            dir.path(),
            Some(base_dir.path()),
            Preset::Custom("does-not-exist".to_string()),
        )
        .expect_err("init should fail for an unresolvable custom preset");

        assert!(error.contains("unknown preset 'does-not-exist'"));
    }

    #[test]
    fn init_with_no_base_config_matches_init_with_a_base_dir_that_has_none() {
        let dir = tempfile::tempdir().expect("failed to create temp dir");
        let base_dir = tempfile::tempdir().expect("failed to create temp dir");

        let path =
            initialize_config_with_base(dir.path(), Some(base_dir.path()), Preset::default())
                .expect("init should succeed");
        let generated = fs::read_to_string(&path).expect("failed to read generated config");

        let default_dir = tempfile::tempdir().expect("failed to create temp dir");
        let default_path = initialize_default_config(default_dir.path(), Preset::default())
            .expect("init should succeed");
        let default_generated =
            fs::read_to_string(&default_path).expect("failed to read generated config");

        assert_eq!(generated, default_generated);
    }

    #[test]
    fn init_merges_an_executable_adjacent_base_config_over_the_defaults() {
        let dir = tempfile::tempdir().expect("failed to create temp dir");
        let base_dir = tempfile::tempdir().expect("failed to create temp dir");
        write_config(
            base_dir.path(),
            "papyrus-lint.yaml",
            "compiler_path: /opt/PapyrusCompiler.exe\nsemicolon: true\n",
        );

        let path =
            initialize_config_with_base(dir.path(), Some(base_dir.path()), Preset::default())
                .expect("init should succeed");
        let generated = fs::read_to_string(&path).expect("failed to read generated config");

        // The base's own settings win...
        assert!(generated.contains("compiler_path: /opt/PapyrusCompiler.exe\n"));
        assert!(generated.contains("semicolon: true\n"));
        // ...while everything the base didn't set still falls back to the
        // built-in default.
        assert!(generated.contains("indentation: tab\n"));
        assert!(generated.contains("strict_achlist_scope: false\n"));
    }

    #[test]
    fn init_ignores_an_empty_executable_adjacent_base_config() {
        let dir = tempfile::tempdir().expect("failed to create temp dir");
        let base_dir = tempfile::tempdir().expect("failed to create temp dir");
        write_config(base_dir.path(), "papyrus-lint.yaml", "");

        let path =
            initialize_config_with_base(dir.path(), Some(base_dir.path()), Preset::default())
                .expect("init should succeed");
        let generated = fs::read_to_string(&path).expect("failed to read generated config");

        let default_dir = tempfile::tempdir().expect("failed to create temp dir");
        let default_path = initialize_default_config(default_dir.path(), Preset::default())
            .expect("init should succeed");
        let default_generated =
            fs::read_to_string(&default_path).expect("failed to read generated config");

        assert_eq!(generated, default_generated);
    }

    #[test]
    fn init_errors_on_an_invalid_executable_adjacent_base_config() {
        let dir = tempfile::tempdir().expect("failed to create temp dir");
        let base_dir = tempfile::tempdir().expect("failed to create temp dir");
        write_config(
            base_dir.path(),
            "papyrus-lint.yaml",
            "semicolon: [not a bool\n",
        );

        assert!(
            initialize_config_with_base(dir.path(), Some(base_dir.path()), Preset::default())
                .is_err()
        );
    }

    #[test]
    fn init_refuses_to_replace_either_supported_config_name() {
        for name in CONFIG_FILE_NAMES {
            let dir = tempfile::tempdir().expect("failed to create temp dir");
            write_config(dir.path(), name, "semicolon: true\n");

            let error = initialize_config_with_base(dir.path(), None, Preset::default())
                .expect_err("init should reject an existing config");

            assert!(error.contains(name));
            assert_eq!(
                fs::read_to_string(dir.path().join(name)).expect("failed to read existing config"),
                "semicolon: true\n"
            );
        }
    }

    #[test]
    fn standard_preset_turns_off_purely_stylistic_rules_but_keeps_formatting() {
        let dir = tempfile::tempdir().expect("failed to create temp dir");

        let path = initialize_config_with_base(dir.path(), None, Preset::Standard)
            .expect("init should succeed");
        let generated = fs::read_to_string(&path).expect("failed to read generated config");

        assert!(generated.contains("  identifier_casing: false\n"));
        assert!(generated.contains("  trailing_whitespace: true\n"));
    }

    #[test]
    fn careful_preset_relaxes_complexity_thresholds_and_disables_formatting() {
        let dir = tempfile::tempdir().expect("failed to create temp dir");

        let path = initialize_config_with_base(dir.path(), None, Preset::Careful)
            .expect("init should succeed");
        let generated = fs::read_to_string(&path).expect("failed to read generated config");

        assert!(generated.contains("cyclomatic_complexity_warning: 20\n"));
        assert!(generated.contains("cyclomatic_complexity_error: 40\n"));
        assert!(generated.contains("  trailing_whitespace: false\n"));
    }

    #[test]
    fn strict_preset_matches_the_built_in_default() {
        let dir = tempfile::tempdir().expect("failed to create temp dir");

        let path = initialize_config_with_base(dir.path(), None, Preset::Strict)
            .expect("init should succeed");
        let generated = fs::read_to_string(&path).expect("failed to read generated config");

        let default_dir = tempfile::tempdir().expect("failed to create temp dir");
        let default_path = initialize_default_config(default_dir.path(), Preset::default())
            .expect("init should succeed");
        let default_generated =
            fs::read_to_string(&default_path).expect("failed to read generated config");

        assert_eq!(generated, default_generated);
    }

    #[test]
    fn executable_adjacent_base_config_overrides_a_non_strict_preset() {
        let dir = tempfile::tempdir().expect("failed to create temp dir");
        let base_dir = tempfile::tempdir().expect("failed to create temp dir");
        write_config(
            base_dir.path(),
            "papyrus-lint.yaml",
            "semicolon: true\nrules:\n  property_sorting: true\n",
        );

        let path = initialize_config_with_base(dir.path(), Some(base_dir.path()), Preset::Careful)
            .expect("init should succeed");
        let generated = fs::read_to_string(&path).expect("failed to read generated config");

        // The base's own settings win, even over the preset's own values...
        assert!(generated.contains("semicolon: true\n"));
        assert!(generated.contains("  property_sorting: true\n"));
        // ...while every other rule/setting still falls back to the
        // selected preset rather than the hardcoded built-in default.
        assert!(generated.contains("cyclomatic_complexity_warning: 20\n"));
        assert!(generated.contains("  trailing_whitespace: false\n"));
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
    fn auto_detect_compiler_path_finds_executable_one_level_up() {
        let root = tempfile::tempdir().expect("failed to create temp dir");
        let compiler_dir = root.path().join("Papyrus Compiler");
        fs::create_dir(&compiler_dir).expect("failed to create compiler dir");
        fs::write(compiler_dir.join("PapyrusCompiler.exe"), b"").expect("failed to write stub exe");
        let data_dir = root.path().join("Data");
        fs::create_dir(&data_dir).expect("failed to create data dir");

        let detected = auto_detect_compiler_path(&data_dir);

        assert_eq!(detected, Some(compiler_dir.join("PapyrusCompiler.exe")));
    }

    #[test]
    fn auto_detect_compiler_path_returns_none_when_absent() {
        let root = tempfile::tempdir().expect("failed to create temp dir");
        let data_dir = root.path().join("Data");
        fs::create_dir(&data_dir).expect("failed to create data dir");

        assert_eq!(auto_detect_compiler_path(&data_dir), None);
    }

    #[test]
    fn resolve_compiler_path_prefers_explicit_override_over_auto_detection() {
        let root = tempfile::tempdir().expect("failed to create temp dir");
        let compiler_dir = root.path().join("Papyrus Compiler");
        fs::create_dir(&compiler_dir).expect("failed to create compiler dir");
        fs::write(compiler_dir.join("PapyrusCompiler.exe"), b"").expect("failed to write stub exe");
        let data_dir = root.path().join("Data");
        fs::create_dir(&data_dir).expect("failed to create data dir");
        save_compiler_path(&data_dir, Some("C:\\Custom\\PapyrusCompiler.exe"))
            .expect("saving compiler path should succeed");

        assert_eq!(
            resolve_compiler_path(&data_dir).expect("should succeed"),
            Some("C:\\Custom\\PapyrusCompiler.exe".to_string())
        );
    }

    #[test]
    fn resolve_compiler_path_falls_back_to_auto_detection() {
        let root = tempfile::tempdir().expect("failed to create temp dir");
        let compiler_dir = root.path().join("Papyrus Compiler");
        fs::create_dir(&compiler_dir).expect("failed to create compiler dir");
        fs::write(compiler_dir.join("PapyrusCompiler.exe"), b"").expect("failed to write stub exe");
        let data_dir = root.path().join("Data");
        fs::create_dir(&data_dir).expect("failed to create data dir");

        assert_eq!(
            resolve_compiler_path(&data_dir).expect("should succeed"),
            Some(
                compiler_dir
                    .join("PapyrusCompiler.exe")
                    .to_string_lossy()
                    .into_owned()
            )
        );
    }

    #[test]
    fn resolve_compiler_path_none_when_neither_available() {
        let dir = tempfile::tempdir().expect("failed to create temp dir");

        assert_eq!(
            resolve_compiler_path(dir.path()).expect("should succeed"),
            None
        );
    }
}
