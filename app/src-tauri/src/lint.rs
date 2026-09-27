//! Project-aware linting, compile-check, and compile commands.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::str::FromStr;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex, OnceLock, RwLock};

use papyrus_lint_core::ignore_file::IgnoreFile;
use papyrus_lint_core::project_lint::{lint_script, ConflictScope, ProjectLint};
use papyrus_lint_core::source_encoding::read_psc_source;
use papyrus_lint_core::{ast_cache, collision_cache, compiler, function_table, script_locator};
use serde::{Deserialize, Serialize};
use tauri::ipc::{Channel, CommandArg, CommandItem, InvokeError, JavaScriptChannelId};

/// Identity of one desktop-app [`function_table::FunctionTable`]: project
/// root plus the two configured search-root lists. Concurrent Tauri
/// commands for the same project reuse one `RwLock`-guarded table through
/// [`function_table::SharedFunctionTable`], matching the CLI's `--threads`
/// workers rather than building an empty table per file. The exclusive lock
/// is taken only when a lookup still has to parse a script the parse-phase
/// type closure did not already preload.
#[derive(Clone, PartialEq, Eq, Hash)]
struct SharedTableKey {
    game: papyrus_lints::Game,
    root: PathBuf,
    additional_roots: Vec<String>,
    lookup_roots: Vec<String>,
    known_scripts: Option<Vec<PathBuf>>,
}

fn shared_tables(
) -> &'static Mutex<HashMap<SharedTableKey, Arc<RwLock<function_table::FunctionTable>>>> {
    static TABLES: OnceLock<
        Mutex<HashMap<SharedTableKey, Arc<RwLock<function_table::FunctionTable>>>>,
    > = OnceLock::new();
    TABLES.get_or_init(|| Mutex::new(HashMap::new()))
}

pub(crate) fn project_function_table(
    root: String,
    additional_roots: Vec<String>,
    lookup_roots: Vec<String>,
) -> Arc<RwLock<function_table::FunctionTable>> {
    project_function_table_for_game(
        papyrus_lints::Game::default(),
        root,
        additional_roots,
        lookup_roots,
    )
}

pub(crate) fn project_function_table_for_game(
    game: papyrus_lints::Game,
    root: String,
    additional_roots: Vec<String>,
    lookup_roots: Vec<String>,
) -> Arc<RwLock<function_table::FunctionTable>> {
    project_function_table_for_game_and_scripts(game, root, additional_roots, lookup_roots, None)
}

fn project_function_table_for_game_and_scripts(
    game: papyrus_lints::Game,
    root: String,
    additional_roots: Vec<String>,
    lookup_roots: Vec<String>,
    known_scripts: Option<Vec<PathBuf>>,
) -> Arc<RwLock<function_table::FunctionTable>> {
    let key = SharedTableKey {
        game,
        root: PathBuf::from(&root),
        additional_roots: additional_roots.clone(),
        lookup_roots: lookup_roots.clone(),
        known_scripts: known_scripts.clone(),
    };
    let mut cache = shared_tables()
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    cache
        .entry(key)
        .or_insert_with(|| {
            let mut table = function_table::FunctionTable::new_with_additional_roots(
                PathBuf::from(root),
                additional_roots,
            )
            .with_game(game)
            .with_lookup_roots(lookup_roots);
            if let Some(paths) = known_scripts {
                table = table.with_known_scripts(&paths);
            }
            Arc::new(RwLock::new(table))
        })
        .clone()
}

/// Project-level inputs shared by [`lint_psc_file`] and the mutating repair
/// commands: the project root, lint configuration, extra script roots,
/// analysis-only lookup roots, strict-scope inputs, compiler path, and whether
/// to merge PapyrusCompiler.exe errors into the result. Grouped so adding a
/// project-level option only changes this type (and the frontend helper that
/// builds it) rather than every command contract independently.
#[derive(Debug, Clone, Default, Deserialize, Serialize)]
pub(crate) struct ProjectLintContext {
    /// The project root (conventionally the directory containing the
    /// `.achlist` file); it lets the "Argument type check" lint resolve
    /// calls to functions declared on other scripts under `root`, and lets
    /// the "Return type check" lint accept a returned value whose script
    /// under `root` extends the declared return type.
    pub(crate) root: String,
    pub(crate) config: papyrus_lints::Config,
    /// The project's configured additional script roots (see
    /// [`load_script_roots`]), searched the same way alongside `root`'s
    /// conventional source directories.
    pub(crate) additional_roots: Vec<String>,
    /// Analysis-only fallback directories (see [`load_lookup_script_roots`]),
    /// searched only after those, never linted, and ignored by
    /// `conflicting_script_versions`.
    pub(crate) lookup_roots: Vec<String>,
    /// See [`load_compiler_path`].
    pub(crate) compiler_path: String,
    /// See [`load_compile_check`]. Together with [`Self::compiler_path`],
    /// controls whether PapyrusCompiler.exe's own errors are merged in too
    /// — see [`lint_with_compile_check`].
    pub(crate) compile_check: bool,
    /// Restricts project resolution and conflict detection to scripts
    /// explicitly listed in [`Self::known_scripts`].
    pub(crate) strict_achlist_scope: bool,
    /// Scripts listed by the current achlist, PPJ, directory, or direct file.
    pub(crate) known_scripts: Vec<String>,
}

impl ProjectLintContext {
    pub(crate) fn function_table(&self) -> Arc<RwLock<function_table::FunctionTable>> {
        let known_scripts = self
            .strict_achlist_scope
            .then(|| self.known_scripts.iter().map(PathBuf::from).collect());
        project_function_table_for_game_and_scripts(
            self.config.game,
            self.root.clone(),
            self.additional_roots.clone(),
            self.lookup_roots.clone(),
            known_scripts,
        )
    }
}

/// Compiles the `.psc` file at `path` using the compiler executable at
/// `compiler_path` (see [`load_compiler_path`]/[`resolve_compiler_path`]
/// for how the frontend obtains that path), selecting compiler flags for
/// `game`. `additional_roots` are the
/// project's configured additional script roots (see
/// [`load_script_roots`]), included in the compiler's `-i` argument
/// alongside the two conventional source directories. Returns an error if
/// `compiler_path` is blank/unconfigured or the compiler process itself
/// couldn't be run; a script that fails to compile is still reported as
/// `Ok`, with [`compiler::CompileOutcome::success`] false and the
/// compiler's stdout/stderr carrying the reported errors.
#[tauri::command(async)]
pub(crate) fn compile_psc_file(
    path: String,
    game: papyrus_lints::Game,
    compiler_path: String,
    additional_roots: Vec<String>,
) -> Result<compiler::CompileOutcome, String> {
    let compiler_path = compiler_path.trim();
    if compiler_path.is_empty() {
        return Err(
            "No PapyrusCompiler.exe path is configured. Set one in the Settings tab.".to_string(),
        );
    }

    compiler::compile_psc_file(
        game,
        Path::new(compiler_path),
        &PathBuf::from(path),
        &additional_roots,
    )
}

/// Lints `source` for `path` the same way `PapyrusLinterCLI` does: project
/// diagnostics, the engine lint, an optional compiler check, then
/// `ignores`. See [`papyrus_lint_core::project_lint::lint_script`].
///
/// The desktop app primes the AST cache itself (or, for a batch, the parser
/// memo) before this call, so the shared pass does not touch the disk cache
/// again. A same-named conflict is looked up in either the strict list of
/// known scripts or the cached directory index, matching the CLI's selected
/// scope, and the collision cache is flushed before returning.
pub(crate) fn lint_with_compile_check<E: papyrus_lints::ExternalSignatures>(
    path: &Path,
    source: &str,
    context: &ProjectLintContext,
    function_table: &mut E,
    ignores: Option<&IgnoreFile>,
) -> Vec<papyrus_lints::Diagnostic> {
    let cached_index = (!context.strict_achlist_scope
        && context.config.rules.conflicting_script_versions)
        .then(|| {
            script_locator::cached_script_index(Path::new(&context.root), &context.additional_roots)
        });
    let strict_candidates = context.strict_achlist_scope.then(|| {
        let file_name = path.file_name().and_then(|name| name.to_str());
        context
            .known_scripts
            .iter()
            .map(PathBuf::from)
            .filter(|candidate| {
                candidate
                    .file_name()
                    .and_then(|name| name.to_str())
                    .zip(file_name)
                    .is_some_and(|(candidate, current)| candidate.eq_ignore_ascii_case(current))
            })
            .collect::<Vec<_>>()
    });
    let conflicts = match (strict_candidates.as_ref(), cached_index.as_ref()) {
        (Some(candidates), _) => ConflictScope::Known {
            candidates,
            short_paths: false,
        },
        (None, Some(index)) => ConflictScope::Index {
            index: index.as_ref(),
            short_paths: false,
        },
        (None, None) => ConflictScope::Index {
            index: empty_script_index(),
            short_paths: false,
        },
    };
    lint_script(
        path,
        source,
        function_table,
        &ProjectLint {
            config: &context.config,
            project_root: Path::new(&context.root),
            additional_roots: &context.additional_roots,
            conflicts,
            compile_check: context.compile_check,
            compiler_path: &context.compiler_path,
            ignores,
            already_primed: true,
            flush_collision_cache: true,
        },
    )
}

fn empty_script_index() -> &'static script_locator::ScriptIndex {
    static EMPTY: OnceLock<script_locator::ScriptIndex> = OnceLock::new();
    EMPTY.get_or_init(script_locator::ScriptIndex::new)
}

/// Reads the `.psc` file at `path` and runs every lint rule against it,
/// honoring the semicolon style `context.config` selects. See
/// [`ProjectLintContext`] for `root`/`additional_roots`/`lookup_roots`/
/// `strict_achlist_scope`/`known_scripts`/`compiler_path`/`compile_check`.
#[tauri::command(async)]
pub(crate) fn lint_psc_file(
    path: String,
    context: ProjectLintContext,
) -> Result<Vec<papyrus_lints::Diagnostic>, String> {
    let path = Path::new(&path);
    let source = read_psc_source(path).map_err(|err| err.to_string())?;
    ast_cache::ensure_primed_for_game(context.config.game, path, &source);
    // Loaded before the pass so a valid file is filtered inside it, but a
    // bad file still fails only after that pass — same as applying the
    // ignore list once the diagnostics already existed.
    let ignores = IgnoreFile::load_optional(Path::new(&context.root));
    let function_table = context.function_table();
    let mut shared = function_table::SharedFunctionTable(function_table.as_ref());
    let diagnostics = lint_with_compile_check(
        path,
        &source,
        &context,
        &mut shared,
        ignores.as_ref().ok().and_then(Option::as_ref),
    );
    ignores?;
    Ok(diagnostics)
}

/// One update for the desktop progress bar while [`preload_project_scripts`]
/// closes over referenced scripts. `total` starts at the lint-target count
/// and grows as on-disk dependencies are enqueued, matching the CLI's
/// "Parsing: n/total" line. `total == 0` is indeterminate: the closure has
/// finished and the function table is being indexed, which has no fraction
/// of its own.
#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct PreloadProgress {
    pub(crate) phase: String,
    pub(crate) completed: usize,
    pub(crate) total: usize,
}

fn emit_preload_progress(channel: Option<&Channel<PreloadProgress>>, progress: PreloadProgress) {
    if let Some(channel) = channel {
        let _ = channel.send(progress);
    }
}

/// Optional progress channel for [`preload_project_scripts`].
///
/// `Channel<T>` is a [`CommandArg`] by itself, but `Option<Channel<T>>` is
/// not: the blanket impl then requires `Channel<T>: Deserialize`, which it
/// does not implement. A missing or null `onProgress` means "no channel",
/// which is what the frontend sends when it cannot construct one.
pub(crate) struct OptionalPreloadChannel(Option<Channel<PreloadProgress>>);

impl From<Option<Channel<PreloadProgress>>> for OptionalPreloadChannel {
    fn from(channel: Option<Channel<PreloadProgress>>) -> Self {
        Self(channel)
    }
}

impl<'de, R: tauri::Runtime> CommandArg<'de, R> for OptionalPreloadChannel {
    fn from_command(command: CommandItem<'de, R>) -> Result<Self, InvokeError> {
        let name = command.name;
        let key = command.key;
        let webview = command.message.webview();
        let value: Option<String> = Deserialize::deserialize(command).map_err(|error| {
            InvokeError::from_error(tauri::Error::InvalidArgs(name, key, error))
        })?;
        let Some(value) = value else {
            return Ok(Self(None));
        };
        let id = JavaScriptChannelId::from_str(&value).map_err(|_| {
            InvokeError::from(format!(
                "invalid channel value `{value}`, expected a string in the `__CHANNEL__:ID` format"
            ))
        })?;
        Ok(Self(Some(id.channel_on(webview))))
    }
}

/// Parses every one of `paths` up front and closes over the type names in
/// those ASTs (mirroring `PapyrusLinterCLI`'s parse phase — see
/// [`function_table::FunctionTable::parse_type_closure`]), then preloads
/// `context`'s shared function table from the result. Referenced scripts
/// that are not in `paths` — project files, lookup-root files, and bundled
/// vanilla/extender scripts — are cached for analysis only and are not
/// linted. Resolving one of them from a later per-file [`lint_psc_file`]
/// is a cache hit rather than a write-locked on-demand parse. A path that
/// fails to read or parse is simply left out of the seed preload (the
/// frontend's own per-file call still reports that error); a referenced
/// name that cannot be resolved is cached unresolved so lint does not
/// retry it. This command never fails outright.
///
/// `on_progress`, when the frontend supplies it, is notified after each
/// on-disk parse in that closure and once more before the function table
/// is indexed. Absent, the command behaves exactly as before.
#[tauri::command(async)]
pub(crate) fn preload_project_scripts(
    paths: Vec<String>,
    context: ProjectLintContext,
    on_progress: OptionalPreloadChannel,
) {
    let on_progress = on_progress.0;
    let function_table = context.function_table();
    let script_paths: Vec<PathBuf> = paths.into_iter().map(PathBuf::from).collect();
    let report = on_progress.is_some();
    close_project_scripts(
        &function_table,
        &script_paths,
        context.config.game,
        report,
        |progress| {
            emit_preload_progress(on_progress.as_ref(), progress);
        },
    );
}

/// One file from [`lint_project_scripts`], shaped like the frontend's
/// `PscParseOutcome`. A script that fails to read or parse is `ok: false`
/// with no findings — the same outcome the per-file `parse_psc_file` +
/// `lint_psc_file` pair produced, where a parse error never reached lint.
#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ProjectFileOutcome {
    pub(crate) path: String,
    pub(crate) ok: bool,
    pub(crate) detail: String,
    pub(crate) findings: Vec<papyrus_lints::Diagnostic>,
}

/// Streamed to the desktop progress bar and results list while
/// [`lint_project_scripts`] runs. `Progress` with `total == 0` is the
/// indeterminate indexing step, matching [`PreloadProgress`].
#[derive(Clone, Serialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub(crate) enum ProjectLintEvent {
    Progress {
        phase: String,
        completed: usize,
        total: usize,
    },
    Result {
        path: String,
        ok: bool,
        detail: String,
        findings: Vec<papyrus_lints::Diagnostic>,
    },
}

fn emit_project_event(channel: Option<&Channel<ProjectLintEvent>>, event: ProjectLintEvent) {
    if let Some(channel) = channel {
        let _ = channel.send(event);
    }
}

/// Optional event channel for [`lint_project_scripts`]. Same reason as
/// [`OptionalPreloadChannel`]: `Option<Channel<T>>` is not a [`CommandArg`].
pub(crate) struct OptionalProjectLintChannel(Option<Channel<ProjectLintEvent>>);

impl From<Option<Channel<ProjectLintEvent>>> for OptionalProjectLintChannel {
    fn from(channel: Option<Channel<ProjectLintEvent>>) -> Self {
        Self(channel)
    }
}

impl<'de, R: tauri::Runtime> CommandArg<'de, R> for OptionalProjectLintChannel {
    fn from_command(command: CommandItem<'de, R>) -> Result<Self, InvokeError> {
        let name = command.name;
        let key = command.key;
        let webview = command.message.webview();
        let value: Option<String> = Deserialize::deserialize(command).map_err(|error| {
            InvokeError::from_error(tauri::Error::InvalidArgs(name, key, error))
        })?;
        let Some(value) = value else {
            return Ok(Self(None));
        };
        let id = JavaScriptChannelId::from_str(&value).map_err(|_| {
            InvokeError::from(format!(
                "invalid channel value `{value}`, expected a string in the `__CHANNEL__:ID` format"
            ))
        })?;
        Ok(Self(Some(id.channel_on(webview))))
    }
}

/// Parses, indexes, and lints every path in `paths` in this process.
///
/// This is the CLI's lint pipeline (`parse_type_closure`, then
/// [`function_table::FunctionTable::preload`], then
/// [`papyrus_lint_core::parallel::map_in_parallel`]) behind one Tauri
/// command. The desktop drop path used to follow that preload with a
/// second `parse_psc_file` and `lint_psc_file` invoke per file; those
/// round-trips dominated large batches such as the Skyrim base scripts.
///
/// `on_event` receives parsing progress (a growing total, same as
/// [`preload_project_scripts`]), one indeterminate indexing update, then
/// one [`ProjectLintEvent::Result`] and one linting-progress update per
/// finished file, in completion order. The command itself returns nothing:
/// the findings already went out on the channel, and repeating them in the
/// invoke response would ship the whole batch across IPC a second time.
/// Absent `on_event`, the work still runs. This command never fails
/// outright; a file that cannot be read or parsed is a result with
/// `ok: false`.
#[tauri::command(async)]
pub(crate) fn lint_project_scripts(
    paths: Vec<String>,
    context: ProjectLintContext,
    on_event: OptionalProjectLintChannel,
) {
    let on_event = on_event.0;
    let function_table = context.function_table();
    let script_paths: Vec<PathBuf> = paths.iter().map(PathBuf::from).collect();
    let report = on_event.is_some();
    let closed = close_project_scripts(
        &function_table,
        &script_paths,
        context.config.game,
        report,
        |progress| {
            emit_project_event(
                on_event.as_ref(),
                ProjectLintEvent::Progress {
                    phase: progress.phase,
                    completed: progress.completed,
                    total: progress.total,
                },
            );
        },
    );

    let ignores =
        match papyrus_lint_core::ignore_file::IgnoreFile::load_optional(Path::new(&context.root)) {
            Ok(ignores) => ignores,
            Err(err) => {
                for (index, path) in paths.iter().enumerate() {
                    emit_project_event(
                        on_event.as_ref(),
                        ProjectLintEvent::Result {
                            path: path.clone(),
                            ok: false,
                            detail: err.clone(),
                            findings: Vec::new(),
                        },
                    );
                    emit_project_event(
                        on_event.as_ref(),
                        ProjectLintEvent::Progress {
                            phase: "Linting".to_string(),
                            completed: index + 1,
                            total: paths.len(),
                        },
                    );
                }
                return;
            }
        };

    let completed = AtomicUsize::new(0);
    let total = paths.len();
    papyrus_lint_core::parallel::map_in_parallel(
        (0..total).collect(),
        papyrus_lint_core::parallel::default_thread_count(),
        |index| {
            let outcome = lint_preloaded_script(
                &paths[index],
                &script_paths[index],
                &closed.seeds[index],
                &context,
                &function_table,
                ignores.as_ref(),
            );
            let done = completed.fetch_add(1, Ordering::SeqCst) + 1;
            emit_project_event(
                on_event.as_ref(),
                ProjectLintEvent::Result {
                    path: outcome.path,
                    ok: outcome.ok,
                    detail: outcome.detail,
                    findings: outcome.findings,
                },
            );
            emit_project_event(
                on_event.as_ref(),
                ProjectLintEvent::Progress {
                    phase: "Linting".to_string(),
                    completed: done,
                    total,
                },
            );
        },
    );
    collision_cache::flush();
}

/// Shared parse + function-table preload used by [`preload_project_scripts`]
/// and [`lint_project_scripts`]. `on_progress` runs after each on-disk parse
/// and once more, with `total == 0`, before the table is indexed.
fn close_project_scripts(
    function_table: &Arc<RwLock<function_table::FunctionTable>>,
    script_paths: &[PathBuf],
    game: papyrus_lints::Game,
    report_progress: bool,
    on_progress: impl Fn(PreloadProgress) + Sync,
) -> function_table::ClosedScripts<ProjectScriptParse> {
    collision_cache::preload(game, script_paths);
    let total_files = AtomicUsize::new(script_paths.len());
    let completed = AtomicUsize::new(0);
    let closed = {
        let table = function_table
            .read()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        table.parse_type_closure(
            script_paths,
            function_table::TypeClosureOptions {
                threads: papyrus_lint_core::parallel::default_thread_count(),
                total_files: report_progress.then_some(&total_files),
            },
            |path| parse_project_script(game, path),
            |parsed| parsed.ast.as_ref(),
            || {
                let done = completed.fetch_add(1, Ordering::SeqCst) + 1;
                on_progress(PreloadProgress {
                    phase: "Parsing".to_string(),
                    completed: done,
                    total: total_files.load(Ordering::SeqCst),
                });
            },
        )
    };

    on_progress(PreloadProgress {
        phase: "Indexing scripts".to_string(),
        completed: 0,
        total: 0,
    });

    let entries: Vec<function_table::PreloadedScript> = script_paths
        .iter()
        .zip(closed.seeds.iter())
        .filter_map(|(path, parsed)| {
            let source = parsed.source.as_deref()?;
            let name_lower = path.file_stem()?.to_str()?.to_ascii_lowercase();
            Some(function_table::PreloadedScript {
                path,
                name_lower,
                ast: parsed.ast.as_ref(),
                source,
            })
        })
        .collect();

    let mut table = function_table
        .write()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    table.preload(entries);
    table.preload_dependencies(&closed.dependencies);
    table.preload_name_slots(&closed.bundled, &closed.unresolved);
    collision_cache::flush();
    closed
}

fn lint_preloaded_script(
    reported_path: &str,
    path: &Path,
    parsed: &ProjectScriptParse,
    context: &ProjectLintContext,
    function_table: &Arc<RwLock<function_table::FunctionTable>>,
    ignores: Option<&papyrus_lint_core::ignore_file::IgnoreFile>,
) -> ProjectFileOutcome {
    let failed = |detail: String| ProjectFileOutcome {
        path: reported_path.to_string(),
        ok: false,
        detail,
        findings: Vec::new(),
    };
    // A parse or read error used to stop the desktop's per-file pair before
    // `lint_psc_file`. Keep that: an unparseable script is not linted here.
    if parsed.ast.is_none() {
        return failed(
            parsed
                .error
                .clone()
                .unwrap_or_else(|| "failed to parse script".to_string()),
        );
    }
    let Some(source) = parsed.source.as_deref() else {
        return failed(
            parsed
                .error
                .clone()
                .unwrap_or_else(|| "failed to read script".to_string()),
        );
    };
    // Thread-local parser memo, primed on this worker the way the CLI primes
    // before `lint_file`, so the lint pass does not take `ast_cache`'s
    // process-wide lock or re-parse a source this batch already parsed.
    if let Some(ast) = parsed.ast.clone() {
        papyrus_parser::prime_cache(source, ast);
    }
    if let Some(tokens) = parsed.tokens.clone() {
        if !tokens.is_empty() {
            papyrus_parser::prime_tokenize_cache(source, tokens);
        }
    }
    let mut shared = function_table::SharedFunctionTable(function_table.as_ref());
    let diagnostics = lint_with_compile_check(path, source, context, &mut shared, ignores);
    let name = parsed
        .ast
        .as_ref()
        .map(|ast| ast.name.as_str())
        .unwrap_or("");
    ProjectFileOutcome {
        path: reported_path.to_string(),
        ok: true,
        detail: format!("parsed as \"{name}\""),
        findings: diagnostics,
    }
}

struct ProjectScriptParse {
    source: Option<String>,
    ast: Option<papyrus_parser::ast::Script>,
    tokens: Option<Vec<papyrus_parser::token::Token>>,
    /// Set when the file could not be read or parsed. Preload still records
    /// a readable-but-unparseable script as unresolved; lint reports this
    /// string instead of running rules.
    error: Option<String>,
}

#[derive(Debug, PartialEq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct CompletionQuery {
    receiver_type: String,
    prefix: String,
    prefix_start: usize,
}

fn blank_comments(source: &str) -> String {
    let block = regex::Regex::new(r"(?s);/.*?(?:/;|\z)").expect("valid block comment regex");
    let brace = regex::Regex::new(r"(?s)\{.*?(?:\}|\z)").expect("valid brace comment regex");
    let line = regex::Regex::new(r";[^\n]*").expect("valid line comment regex");
    let blank = |captures: &regex::Captures<'_>| {
        captures[0]
            .chars()
            .map(|character| if character == '\n' { '\n' } else { ' ' })
            .collect::<String>()
    };
    let source = block.replace_all(source, &blank);
    let source = brace.replace_all(&source, &blank);
    line.replace_all(&source, blank).into_owned()
}

fn declared_type(source: &str, receiver: &str) -> Option<String> {
    let identifier = r"[A-Za-z_]\w*";
    let clean = blank_comments(source);
    let receiver = receiver.to_ascii_lowercase();

    let script = regex::Regex::new(&format!(
        r"(?im)^\s*ScriptName\s+({identifier})(?:\s+Extends\s+({identifier}))?"
    ))
    .expect("valid script declaration regex");
    if let Some(captures) = script.captures(&clean) {
        if receiver == "self" {
            return Some(captures[1].to_string());
        }
        if receiver == "parent" {
            return captures.get(2).map(|parent| parent.as_str().to_string());
        }
    }

    let declarations = regex::Regex::new(&format!(
        r"(?im)^\s*({identifier})(?:\[\])?\s+(?:Property\s+)?({identifier})\s*(?:=|;|\bAuto\b|$)"
    ))
    .expect("valid variable declaration regex");
    let is_keyword = |word: &str| {
        matches!(
            word.to_ascii_lowercase().as_str(),
            "scriptname"
                | "extends"
                | "hidden"
                | "conditional"
                | "import"
                | "function"
                | "endfunction"
                | "event"
                | "endevent"
                | "property"
                | "endproperty"
                | "auto"
                | "autoreadonly"
                | "global"
                | "native"
                | "return"
                | "if"
                | "elseif"
                | "else"
                | "endif"
                | "while"
                | "endwhile"
                | "state"
                | "endstate"
                | "new"
                | "as"
                | "true"
                | "false"
                | "none"
                | "self"
                | "parent"
                | "length"
                | "debugonly"
                | "betaonly"
        )
    };
    for captures in declarations.captures_iter(&clean) {
        if captures[2].eq_ignore_ascii_case(&receiver)
            && !is_keyword(&captures[1])
            && !is_keyword(&captures[2])
        {
            return Some(captures[1].to_string());
        }
    }

    let headers = regex::Regex::new(&format!(
        r"(?i)\b(?:Function|Event)\s+{identifier}\s*\(([^)]*)\)"
    ))
    .expect("valid function header regex");
    let parameter = regex::Regex::new(&format!(r"^\s*({identifier})(?:\[\])?\s+({identifier})"))
        .expect("valid parameter regex");
    for header in headers.captures_iter(&clean) {
        for value in header[1].split(',') {
            if let Some(captures) = parameter.captures(value) {
                if captures[2].eq_ignore_ascii_case(&receiver)
                    && !is_keyword(&captures[1])
                    && !is_keyword(&captures[2])
                {
                    return Some(captures[1].to_string());
                }
            }
        }
    }
    None
}

/// Resolves the type and member-name prefix immediately before `cursor_index`.
/// This deliberately scans incomplete source instead of requiring a valid AST,
/// because a trailing member-access dot is not valid Papyrus yet.
#[tauri::command]
pub(crate) fn resolve_completion_query(
    source: String,
    cursor_index: usize,
) -> Option<CompletionQuery> {
    // Browser selection offsets count UTF-16 code units; Rust string slices
    // use UTF-8 byte offsets. Translate without letting non-ASCII comments or
    // string literals before the cursor suppress otherwise valid completion.
    let mut utf16_offset = 0;
    let mut byte_offset = None;
    for (index, character) in source.char_indices() {
        if utf16_offset == cursor_index {
            byte_offset = Some(index);
            break;
        }
        utf16_offset += character.len_utf16();
    }
    if utf16_offset == cursor_index {
        byte_offset.get_or_insert(source.len());
    }
    let before = source.get(..byte_offset?)?;
    let receiver = regex::Regex::new(r"([A-Za-z_]\w*)(?:\s*\[[^\[\]]*\])?\s*\.(\w*)$")
        .expect("valid completion receiver regex");
    let captures = receiver.captures(before)?;
    let prefix = captures[2].to_string();
    let receiver_type = declared_type(&source, &captures[1])?;
    Some(CompletionQuery {
        receiver_type,
        prefix_start: cursor_index - prefix.len(),
        prefix,
    })
}

fn parse_project_script(game: papyrus_lints::Game, path: &Path) -> ProjectScriptParse {
    let source = match read_psc_source(path) {
        Ok(source) => source,
        Err(err) => {
            return ProjectScriptParse {
                source: None,
                ast: None,
                tokens: None,
                error: Some(err.to_string()),
            };
        }
    };
    collision_cache::remember_source(game, path, &source);
    if let Some(ast) = ast_cache::get_for_game(game, path, &source) {
        let tokens = ast_cache::get_tokens_for_game(game, path, &source);
        return ProjectScriptParse {
            source: Some(source),
            ast: Some(ast),
            tokens,
            error: None,
        };
    }
    let ast = match papyrus_parser::parse(&source) {
        Ok(ast) => ast,
        Err(err) => {
            return ProjectScriptParse {
                source: Some(source),
                ast: None,
                tokens: None,
                error: Some(err.to_string()),
            };
        }
    };
    ast_cache::put_for_game(game, path, &source, &ast);
    let tokens = match papyrus_parser::tokenize(&source) {
        Ok(tokens) => {
            ast_cache::put_tokens_for_game(game, path, &source, &tokens);
            Some(tokens)
        }
        Err(_) => None,
    };
    ProjectScriptParse {
        source: Some(source),
        ast: Some(ast),
        tokens,
        error: None,
    }
}

/// Lists every function and property available on an object of type
/// `type_name` (including those inherited via `Extends`), for driving the
/// code viewer's editor autocompletion. See [`lint_psc_file`] for
/// `root`/`additional_roots`.
#[tauri::command(async)]
pub(crate) fn list_script_members(
    root: String,
    type_name: String,
    additional_roots: Vec<String>,
    lookup_roots: Vec<String>,
) -> Vec<function_table::Member> {
    let function_table = project_function_table(root, additional_roots, lookup_roots);
    function_table::SharedFunctionTable(function_table.as_ref()).list_members(&type_name)
}

#[cfg(test)]
#[path = "lint_tests.rs"]
mod tests;
