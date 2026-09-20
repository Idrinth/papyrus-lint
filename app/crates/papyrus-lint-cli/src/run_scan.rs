//! Resolves which scripts a run targets — from an `.achlist`, a `.ppj`, a
//! bare `.psc` file, or a directory scanned recursively — and the project
//! state (lint config, cross-script function table, script index) needed to
//! lint or fix them. This is the discovery phase [`crate::run_lint`] and
//! [`crate::run_fix`] share, done once up front rather than per script.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use papyrus_lint_config as config;
use papyrus_lint_core::achlist;
use papyrus_lint_core::function_table::FunctionTable;
use papyrus_lint_core::ppj;
use papyrus_lint_core::script_locator::find_psc_files_recursively;

use crate::project::{is_ppj_path, is_psc_path, resolve_input_project_root};

/// Project state resolved by [`scan_project`]: the scripts a run should
/// process, alongside everything [`crate::run_lint`]/[`crate::run_fix`] need
/// to lint (or fix) each of them against the rest of the project.
pub(crate) struct ScanOutcome {
    pub(crate) script_paths: Vec<PathBuf>,
    pub(crate) lint_config: papyrus_lints::Config,
    pub(crate) function_table: FunctionTable,
    /// Achlist entries grouped by (lowercased) file name, for
    /// `conflicting_script_versions` in `strict_achlist_scope` mode. Empty
    /// (and unused) otherwise, where `script_index` covers this instead.
    pub(crate) scripts_by_name: HashMap<String, Vec<PathBuf>>,
    pub(crate) script_index: Arc<HashMap<String, Vec<PathBuf>>>,
    pub(crate) strict_achlist_scope: bool,
    pub(crate) compile_check: bool,
    pub(crate) compiler_path: String,
}

/// Resolves `input_path` (an achlist, a bare `.psc` file, or a directory to
/// scan recursively) into the scripts a run should process, the project
/// root's lint configuration (or `config_path`'s, if given), and the
/// cross-script function table those scripts are linted against — the same
/// project-root/config discovery [`run`](crate::run)'s doc comment describes
/// in full. `cli_script_roots` are `--script-root` values, merged in on top
/// of the project's own configured `additional_script_roots`.
///
/// On failure, returns the ready-to-print `error: ...` message the caller
/// should write to stderr before exiting with code `2`.
pub(crate) fn scan_project(
    input_path: &Path,
    config_path: Option<&Path>,
    cli_script_roots: Vec<String>,
) -> Result<ScanOutcome, String> {
    let is_psc_file = is_psc_path(input_path);
    let is_directory = !is_psc_file && input_path.is_dir();

    let (script_paths, ppj_imports) = collect_script_paths(input_path, is_psc_file, is_directory)?;

    // A bare .psc file's project root is found by walking up for a
    // `scripts/source`/`source/scripts` directory pair (see
    // `find_psc_project_root`) so it still works when the script is nested
    // deeper still, e.g. under a namespaced subfolder. An .achlist's/.ppj's
    // own entries, or a scanned directory's own recursively-found entries,
    // are tried the same way first, so a project whose .achlist/.ppj/scanned
    // directory doesn't live in the project root still resolves correctly;
    // only if none of the resolved scripts sit under such a pair do we fall
    // back to the achlist's/ppj's own parent directory (the conventional
    // layout) or, for a scanned directory, the directory itself.
    let project_root =
        resolve_input_project_root(input_path, &script_paths, is_psc_file, is_directory);

    let settings = load_scan_settings(
        &project_root,
        config_path,
        cli_script_roots,
        ppj_imports,
        is_psc_file,
        &script_paths,
    )?;
    Ok(assemble_scan_outcome(script_paths, project_root, settings))
}

struct ScanSettings {
    lint_config: papyrus_lints::Config,
    additional_script_roots: Vec<String>,
    strict_achlist_scope: bool,
    compile_check: bool,
    compiler_path: String,
    lookup_script_roots: Vec<String>,
}

fn load_scan_settings(
    project_root: &Path,
    config_path: Option<&Path>,
    cli_script_roots: Vec<String>,
    ppj_imports: Vec<String>,
    is_psc_file: bool,
    script_paths: &[PathBuf],
) -> Result<ScanSettings, String> {
    let lint_config = config_path
        .map_or_else(
            || config::load_config(project_root),
            config::load_config_from_path,
        )
        .map_err(|err| format!("error: failed to load lint config: {err}"))?;

    // `--config` bypasses discovering the project root's own
    // papyrus-lint.yaml/.yml entirely (see USAGE), so its
    // additional_script_roots is skipped too in that case; `--script-root`
    // and a `.ppj` input's own `<Import>` entries still apply on top either
    // way.
    let mut additional_script_roots = if config_path.is_some() {
        Vec::new()
    } else {
        config::load_script_roots(project_root)
            .map_err(|err| format!("error: failed to load lint config: {err}"))?
    };
    additional_script_roots.extend(ppj_imports);
    additional_script_roots.extend(cli_script_roots);

    // `strict_achlist_scope` (off by default) picks between two ways of
    // letting an achlist's entries resolve each other across arbitrary,
    // non-conventional source directories:
    //
    // - Off (the default): every listed entry's parent directory is added
    //   as a generic additional root, exactly as before this option
    //   existed. This is what an achlist-based project may already depend
    //   on — e.g. an unlisted sibling script in the same directory as a
    //   listed one still resolving — so normal usage sees no change at all.
    // - On: each listed script is instead registered directly by name (see
    //   `FunctionTable::with_known_scripts`), without treating its
    //   directory as a root. This never makes an unlisted file that happens
    //   to share a listed one's directory resolvable, and never requires
    //   scanning that directory at all, which matters a great deal on a
    //   large achlist whose entries are spread across many directories (see
    //   #311) — but it does mean a project relying on the off behavior
    //   above would see resolution/diagnostics change.
    //
    // Unlike `additional_script_roots` above, this is read from whichever
    // config file is actually in effect — the project root's own, or the
    // file named by `--config` — rather than being forced off whenever
    // `--config` is used: it isn't a project-root-only setting, so a
    // `--config` file that sets it is honored the same way it is for every
    // other key in that file (see #362).
    let strict_achlist_scope = config_path
        .map_or_else(
            || config::load_strict_achlist_scope(project_root),
            config::load_strict_achlist_scope_from_path,
        )
        .map_err(|err| format!("error: failed to load lint config: {err}"))?;

    if !is_psc_file && !strict_achlist_scope {
        add_script_parent_roots(script_paths, project_root, &mut additional_script_roots);
    }

    let (compile_check, compiler_path) = load_compile_settings(project_root)?;

    // Analysis-only fallback directories: read from whichever config file
    // is actually in effect, the same as `strict_achlist_scope`. They are
    // never mixed into `additional_script_roots`, so they don't get linted
    // and `conflicting_script_versions` never scans them.
    let lookup_script_roots = config_path
        .map_or_else(
            || config::load_lookup_script_roots(project_root),
            config::load_lookup_script_roots_from_path,
        )
        .map_err(|err| format!("error: failed to load lint config: {err}"))?;

    Ok(ScanSettings {
        lint_config,
        additional_script_roots,
        strict_achlist_scope,
        compile_check,
        compiler_path,
        lookup_script_roots,
    })
}

fn load_compile_settings(project_root: &Path) -> Result<(bool, String), String> {
    // Read from the project root's own config the same way `doctor` reports
    // on them (see `run_doctor`), regardless of `--config` — `compile_check`
    // and `compiler_path` aren't part of the lint settings a `--config`
    // override replaces. `compiler_path` is only resolved when `compile_check`
    // is actually enabled, since it's otherwise unused.
    let compile_check = config::load_compile_check(project_root)
        .map_err(|err| format!("error: failed to load lint config: {err}"))?;
    let compiler_path = if compile_check {
        config::resolve_compiler_path(project_root)
            .map_err(|err| format!("error: failed to load lint config: {err}"))?
            .unwrap_or_default()
    } else {
        String::new()
    };
    Ok((compile_check, compiler_path.trim().to_string()))
}

fn assemble_scan_outcome(
    script_paths: Vec<PathBuf>,
    project_root: PathBuf,
    settings: ScanSettings,
) -> ScanOutcome {
    let mut function_table =
        FunctionTable::new_with_additional_roots(project_root, settings.additional_script_roots)
            .with_game(settings.lint_config.game)
            .with_lookup_roots(settings.lookup_script_roots);
    if settings.strict_achlist_scope {
        function_table = function_table.with_known_scripts(&script_paths);
    }

    // Grouped by (lowercased) file name up front so checking for a
    // same-named achlist entry elsewhere in the list, below, stays
    // proportional to how many scripts actually share a name rather than to
    // the achlist's full size. Only needed in strict-scope mode: the
    // off-by-default `script_index` built below already covers this (and
    // more) via the directories just added to `additional_script_roots`.
    let scripts_by_name = if settings.strict_achlist_scope {
        group_scripts_by_name(&script_paths)
    } else {
        HashMap::new()
    };

    // Built once up front, rather than re-scanning `function_table`'s search
    // directories for every conflict check or newly resolved script, since a
    // modlist-sized achlist can list hundreds of scripts across as many
    // directories (see #311). Empty (and unused) in strict-scope mode, where
    // `scripts_by_name` above covers this instead.
    let script_index = Arc::new(if !settings.strict_achlist_scope {
        papyrus_lint_core::script_locator::build_script_index(
            function_table.root(),
            function_table.additional_roots(),
        )
    } else {
        HashMap::new()
    });
    if !settings.strict_achlist_scope {
        function_table = function_table.with_script_index(Arc::clone(&script_index));
    }

    ScanOutcome {
        script_paths,
        lint_config: settings.lint_config,
        function_table,
        scripts_by_name,
        script_index,
        strict_achlist_scope: settings.strict_achlist_scope,
        compile_check: settings.compile_check,
        compiler_path: settings.compiler_path,
    }
}

/// Resolves `input_path` into the scripts a run should process, alongside
/// any `<Import>` search paths it names — populated only for a `.ppj` input,
/// where they're the project's own `additional_script_roots` equivalent
/// (see [`papyrus_lint_core::ppj`]'s module docs), made absolute (relative
/// to the process's current directory) so they can be appended to
/// `additional_script_roots` regardless of what `project_root` resolves to.
fn collect_script_paths(
    input_path: &Path,
    is_psc_file: bool,
    is_directory: bool,
) -> Result<(Vec<PathBuf>, Vec<String>), String> {
    if is_psc_file {
        return Ok((vec![input_path.to_path_buf()], Vec::new()));
    }
    if is_directory {
        return Ok((find_psc_files_recursively(input_path), Vec::new()));
    }
    if is_ppj_path(input_path) {
        let project = ppj::parse_ppj(input_path).map_err(|err| format!("error: {err}"))?;
        let scripts = project
            .scripts
            .into_iter()
            .filter(|path| is_psc_path(path))
            .collect();
        let imports = project
            .imports
            .iter()
            .map(|import| crate::project::absolutize(import))
            .collect();
        return Ok((scripts, imports));
    }
    let entries = achlist::parse_achlist(input_path).map_err(|err| format!("error: {err}"))?;
    Ok((
        entries
            .into_iter()
            .filter(|path| is_psc_path(path))
            .collect(),
        Vec::new(),
    ))
}

fn group_scripts_by_name(script_paths: &[PathBuf]) -> HashMap<String, Vec<PathBuf>> {
    let mut scripts_by_name: HashMap<String, Vec<PathBuf>> = HashMap::new();
    for script_path in script_paths {
        if let Some(name) = script_path.file_name().and_then(|name| name.to_str()) {
            scripts_by_name
                .entry(name.to_ascii_lowercase())
                .or_default()
                .push(script_path.clone());
        }
    }
    scripts_by_name
}

fn add_script_parent_roots(
    script_paths: &[PathBuf],
    project_root: &Path,
    additional_script_roots: &mut Vec<String>,
) {
    for script_path in script_paths {
        let Some(parent) = script_path.parent() else {
            continue;
        };
        let root = parent
            .strip_prefix(project_root)
            .unwrap_or(parent)
            .to_string_lossy()
            .into_owned();
        if !additional_script_roots.contains(&root) {
            additional_script_roots.push(root);
        }
    }
}
