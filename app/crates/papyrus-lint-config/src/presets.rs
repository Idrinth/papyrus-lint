//! Named baseline configurations `init` (or the desktop app's first-run
//! picker) can generate `papyrus-lint.yaml` from: three built-in presets
//! (see [`Preset`]/[`PRESET_NAMES`]) plus any user-defined ones found under
//! an executable-adjacent `presets` directory (see [`USER_PRESETS_DIR_NAME`]/
//! [`user_presets_dir`]).
//!
//! This module owns [`Preset`] itself (its baseline YAML, name parsing, and
//! the executable-adjacent base-config layering [`initialize_default_config`]/
//! [`preset_lint_config`] apply on top of it), the executable-adjacent
//! directory lookup ([`executable_dir`]) that base config and user presets
//! are found relative to, and the user-preset management functions the
//! CLI's `preset add` and the desktop app's Presets tab call
//! ([`add_user_preset`], [`save_user_preset`], [`rename_user_preset`],
//! [`delete_user_preset`], [`read_user_preset_yaml`],
//! [`list_user_preset_names`]). `papyrus-lint-core`'s own `presets` module
//! layers the desktop app's first-run picker label/description metadata
//! (`PresetInfo`/`all`) on top of this one. Loading/saving a project's own
//! `papyrus-lint.yaml` (not a preset) is [`crate::project_file`]'s job; this
//! module borrows a few of its private helpers (project-file
//! (de)serialization) rather than duplicating them.

use std::borrow::Cow;
use std::fs;
use std::fs::OpenOptions;
use std::io::Write;
use std::path::{Path, PathBuf};

use walkdir::WalkDir;

use crate::comments::with_field_comments;
use crate::project_file::{
    existing_config_path, game_key_first, non_lint_yaml, seed_lookup_script_roots, ProjectFile,
    CONFIG_FILE_NAMES,
};

/// A named baseline `init` can generate `papyrus-lint.yaml` from, selected
/// via the CLI's `--preset <name>` flag (see [`Preset::parse`]). See
/// `configuration/presets/` for each built-in preset's own small overwrite YAML (a
/// header comment plus any non-rule settings it changes, e.g. `careful`'s
/// relaxed cyclomatic complexity thresholds) and the reasoning behind what
/// it turns on/off relative to the others. `build.rs` layers that overwrite
/// file onto `configuration/papyrus-lint.default.yaml`, plus (for `standard`/
/// `careful`) every `rules:` toggle `shared/rules.json` tags `"low"`
/// importance and doesn't mark `kept_in_standard`, into the full YAML this
/// module embeds.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum Preset {
    /// Everything on, including pure style/naming nits. Identical to the
    /// engine's built-in defaults (`configuration/papyrus-lint.default.yaml`), so
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

    /// This preset's baseline YAML content: for a built-in preset, `build.rs`
    /// layers the checked-in `configuration/presets/papyrus-lint.<preset>.yaml`
    /// overwrite file (a header comment plus any non-rule settings the
    /// preset changes) and, for `standard`/`careful`, every `rules:` toggle
    /// `shared/rules.json` says to turn off (see `build.rs`'s
    /// `preset_rule_value`), onto `configuration/papyrus-lint.default.yaml`, and
    /// writes the result to `$OUT_DIR/papyrus-lint.<preset>.yaml`, which is
    /// compiled into the binary here; for [`Self::Custom`], the contents of
    /// the matching `<name>.yaml`/`.yml` file under `base_dir`'s
    /// [`USER_PRESETS_DIR_NAME`] directory. Errors if `base_dir` is
    /// unavailable, has no such directory, or it has no file matching
    /// `name`.
    fn yaml(&self, base_dir: Option<&Path>) -> Result<Cow<'static, str>, String> {
        match self {
            Self::Strict => Ok(Cow::Borrowed(include_str!(concat!(
                env!("OUT_DIR"),
                "/papyrus-lint.strict.yaml"
            )))),
            Self::Standard => Ok(Cow::Borrowed(include_str!(concat!(
                env!("OUT_DIR"),
                "/papyrus-lint.standard.yaml"
            )))),
            Self::Careful => Ok(Cow::Borrowed(include_str!(concat!(
                env!("OUT_DIR"),
                "/papyrus-lint.careful.yaml"
            )))),
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

/// Directory next to the CLI's own running executable, if it can be
/// determined. [`initialize_default_config`] looks here for an optional
/// shared base config, and [`user_presets_dir`] for an optional user
/// presets directory.
fn executable_dir() -> Option<PathBuf> {
    std::env::current_exe()
        .ok()
        .and_then(|path| path.parent().map(Path::to_path_buf))
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

/// Immediate children of `dir`. An unreadable directory yields no entries,
/// matching the previous `fs::read_dir` + `flatten` behavior.
fn dir_children(dir: &Path) -> impl Iterator<Item = PathBuf> {
    WalkDir::new(dir)
        .min_depth(1)
        .max_depth(1)
        .into_iter()
        .filter_map(Result::ok)
        .map(walkdir::DirEntry::into_path)
}

/// Every user preset name available in `dir` (see [`user_presets_dir`]):
/// each `.yaml`/`.yml` file's own file stem (the name it's selected by),
/// sorted case-insensitively so listings (e.g. the desktop app's preset
/// picker) are stable and predictable. Returns an empty `Vec` if `dir`
/// can't be read at all.
pub fn list_user_preset_names(dir: &Path) -> Vec<String> {
    let mut names: Vec<String> = dir_children(dir)
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
    dir_children(dir).find(|path| {
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
/// `lookup_script_roots`/`compile_check`/`strict_achlist_scope`, since those
/// are specific to a project rather than something a reusable preset should
/// hardcode.
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

    let yaml = crate::lint_config_to_yaml(config)?;
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
fn deep_merge(base: serde_norway::Value, over: serde_norway::Value) -> serde_norway::Value {
    match (base, over) {
        (serde_norway::Value::Mapping(mut base_map), serde_norway::Value::Mapping(over_map)) => {
            for (key, value) in over_map {
                let merged = match base_map.remove(&key) {
                    Some(base_value) => deep_merge(base_value, value),
                    None => value,
                };
                base_map.insert(key, merged);
            }
            serde_norway::Value::Mapping(base_map)
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

    let mut base = resolve_preset_project_file(base_dir, &preset)?;
    seed_lookup_script_roots(&mut base);

    let path = dir.join(CONFIG_FILE_NAMES[0]);
    let lint_yaml = crate::lint_config_to_yaml(&base.lint)?;
    let yaml = game_key_first(&format!("{}{lint_yaml}", non_lint_yaml(&base)?));
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&path)
        .map_err(|err| err.to_string())?;
    file.write_all(with_field_comments(&yaml).as_bytes())
        .map_err(|err| err.to_string())?;
    Ok(path)
}

/// Resolves `preset`'s baseline configuration — layered over an
/// executable-adjacent base config at `base_dir`, if one exists, the same
/// way [`initialize_default_config`] does — into a full [`ProjectFile`].
/// Shared by [`initialize_config_with_base`] (which writes the result out
/// as a brand new project config file) and [`preset_lint_config`] (which
/// only needs its lint settings, to reset an existing project's
/// already-edited settings back to a preset in place).
fn resolve_preset_project_file(
    base_dir: Option<&Path>,
    preset: &Preset,
) -> Result<ProjectFile, String> {
    let preset_yaml = preset.yaml(base_dir)?;
    let preset_value: serde_norway::Value =
        serde_norway::from_str(&preset_yaml).map_err(|err| err.to_string())?;

    let merged_value = match base_dir.and_then(existing_config_path) {
        Some(base_path) => {
            let contents = fs::read_to_string(&base_path).map_err(|err| err.to_string())?;
            if contents.trim().is_empty() {
                preset_value
            } else {
                let override_value: serde_norway::Value =
                    serde_norway::from_str(&contents).map_err(|err| err.to_string())?;
                deep_merge(preset_value, override_value)
            }
        }
        None => preset_value,
    };

    serde_norway::from_value(merged_value).map_err(|err| err.to_string())
}

/// Returns `preset`'s lint rule/formatting settings only — not the
/// `compiler_path`/`additional_script_roots`/`lookup_script_roots`/
/// `compile_check`/`strict_achlist_scope` settings [`initialize_default_config`]
/// also seeds a brand new project's file with, since a preset resetting an
/// *existing* project's settings shouldn't touch those — merged with an optional
/// executable-adjacent base config the same way. Used by the desktop app's
/// Settings tab to overwrite its currently edited settings back to a
/// preset in place, without requiring (or touching) a project config file
/// the way [`initialize_default_config`] does.
pub fn preset_lint_config(
    base_dir: Option<&Path>,
    preset: Preset,
) -> Result<papyrus_lints::Config, String> {
    Ok(resolve_preset_project_file(base_dir, &preset)?.lint)
}

/// Same as [`preset_lint_config`], but looks for the optional
/// executable-adjacent base config next to the running executable (see
/// [`executable_dir`]), the same as [`initialize_default_config`] does.
pub fn preset_lint_config_default(preset: Preset) -> Result<papyrus_lints::Config, String> {
    preset_lint_config(executable_dir().as_deref(), preset)
}

#[cfg(test)]
#[path = "presets_tests/mod.rs"]
mod tests;
