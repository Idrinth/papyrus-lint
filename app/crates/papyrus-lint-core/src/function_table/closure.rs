//! Parse-phase closure over the type names a script mentions.
//!
//! Lint should only ask what is already known about a type. Walking
//! `Extends` and other references from [`super::FunctionTable::ensure_loaded`]
//! during lint is resolver work: the scripts are named in the AST the
//! parse phase already owns. [`FunctionTable::parse_type_closure`] starts
//! from the lint targets, parses each one, and enqueues every referenced
//! type that resolves to a `.psc` the table would load. Bundled
//! vanilla/extender scripts are not read from disk; their names are
//! recorded and filled from the blob in [`FunctionTable::preload_name_slots`].
//! A name that cannot be resolved is recorded the same way, so lint does
//! not retry it.
//!
//! Names already queued or resolved are skipped, which is the same cycle
//! break ancestry walks use (`visited`). A seed counts as resolved only
//! when this table would load that same path for its file stem, so a
//! same-stem file that loses to a higher-priority root does not hide the
//! winner. A missing parent stops that branch instead of being retried. Scripts reached only through this closure are
//! analysis-only: callers preload them and must not add them to the lint
//! target list. [`FunctionTable::preload`] still checks
//! [`FunctionTable::resolved_path_and_mtime`], so a same-stem file in a
//! higher-priority root is not overwritten.

use std::collections::{HashSet, VecDeque};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Condvar, Mutex};
use std::thread;

use papyrus_parser::ast::{Expr, FunctionDecl, Script, Stmt};

use super::FunctionTable;
use crate::source_encoding::read_psc_source;

/// How [`FunctionTable::parse_type_closure`] should run.
pub struct TypeClosureOptions<'a> {
    /// Worker count. `0` or `1` parses on the calling thread.
    pub threads: usize,
    /// When set, incremented once per extra on-disk script enqueued after
    /// the seeds. The caller initializes this to the seed count so a
    /// "Parsing: n/total" line can grow as parents appear. Bundled names
    /// are not counted: they are not disk parses.
    pub total_files: Option<&'a AtomicUsize>,
}

/// One `.psc` parsed because a type name resolved to it. Not a lint target
/// unless the caller also passed the path as a seed.
pub struct ParsedDependency {
    pub path: PathBuf,
    /// Lowercased type name the closure resolved, which is the
    /// [`FunctionTable`] cache key (the file stem in the usual case).
    pub name_lower: String,
    pub source: String,
    /// `None` when the file was unreadable or failed to parse. Preloading
    /// that still records a cached-unresolved slot, matching
    /// [`FunctionTable::ensure_loaded`].
    pub ast: Option<Script>,
}

/// Seeds in input order, plus every referenced script the closure parsed
/// or classified without a disk parse.
pub struct ClosedScripts<T> {
    pub seeds: Vec<T>,
    pub dependencies: Vec<ParsedDependency>,
    /// Lowercased names that resolve only through the bundled blob.
    pub bundled: Vec<String>,
    /// Lowercased names that resolve nowhere.
    pub unresolved: Vec<String>,
}

struct QueueInner {
    jobs: VecDeque<Job>,
    /// Jobs waiting in `jobs` or currently running.
    inflight: usize,
}

struct QueueState {
    inner: Mutex<QueueInner>,
    cv: Condvar,
}

enum Job {
    Seed { index: usize, path: PathBuf },
    Dependency { name_lower: String, path: PathBuf },
    Bundled { name_lower: String },
}

impl Job {
    fn counts_as_parsed_file(&self) -> bool {
        !matches!(self, Job::Bundled { .. })
    }
}

struct Shared<T> {
    seen: Mutex<HashSet<String>>,
    seed_slots: Mutex<Vec<Option<T>>>,
    dependencies: Mutex<Vec<ParsedDependency>>,
    bundled: Mutex<Vec<String>>,
    unresolved: Mutex<Vec<String>>,
}

struct Discovery {
    jobs: Vec<Job>,
    disk_jobs: usize,
}

/// Decrements the in-flight count on drop so a panic cannot leave workers
/// blocked on an empty queue.
struct FinishGuard<'a> {
    state: &'a QueueState,
    total_files: Option<&'a AtomicUsize>,
    finished: bool,
}

impl<'a> FinishGuard<'a> {
    fn new(state: &'a QueueState, total_files: Option<&'a AtomicUsize>) -> Self {
        Self {
            state,
            total_files,
            finished: false,
        }
    }

    fn finish(mut self, discovered: Discovery) {
        self.complete(discovered);
        self.finished = true;
    }

    fn complete(&self, discovered: Discovery) {
        if discovered.disk_jobs > 0 {
            if let Some(total) = self.total_files {
                total.fetch_add(discovered.disk_jobs, Ordering::SeqCst);
            }
        }
        let mut inner = self
            .state
            .inner
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        inner.inflight += discovered.jobs.len();
        for job in discovered.jobs {
            inner.jobs.push_back(job);
        }
        inner.inflight = inner.inflight.saturating_sub(1);
        self.state.cv.notify_all();
    }
}

impl Drop for FinishGuard<'_> {
    fn drop(&mut self) {
        if !self.finished {
            self.complete(Discovery {
                jobs: Vec::new(),
                disk_jobs: 0,
            });
        }
    }
}

impl FunctionTable {
    /// Parses every seed, then every script a type name in those ASTs
    /// (and in the scripts they pull in) resolves to.
    ///
    /// `parse_seed` runs only for `seed_paths`. Its results come back in
    /// the same order. Referenced `.psc` files are parsed internally and
    /// returned in [`ClosedScripts::dependencies`] — they are not lint
    /// targets. `on_disk_job_finished` runs after each seed and each
    /// dependency parse, after [`TypeClosureOptions::total_files`] has
    /// already been increased for any disk jobs that parse just enqueued.
    pub fn parse_type_closure<T: Send>(
        &self,
        seed_paths: &[PathBuf],
        options: TypeClosureOptions<'_>,
        parse_seed: impl Fn(&Path) -> T + Sync,
        seed_ast: impl Fn(&T) -> Option<&Script> + Sync,
        on_disk_job_finished: impl Fn() + Sync,
    ) -> ClosedScripts<T> {
        if seed_paths.is_empty() {
            return ClosedScripts {
                seeds: Vec::new(),
                dependencies: Vec::new(),
                bundled: Vec::new(),
                unresolved: Vec::new(),
            };
        }

        let mut seen = HashSet::new();
        for path in seed_paths {
            if let Some(name) = stem_lower(path) {
                // A seed occupies its stem only when this table would load
                // that same path. Otherwise a higher-priority root of the
                // same name must still be parsed for the cache.
                if self
                    .resolve_script_path_kind(&name)
                    .is_some_and(|(resolved, _)| resolved.as_path() == path)
                {
                    seen.insert(name);
                }
            }
        }

        let mut jobs = VecDeque::new();
        for (index, path) in seed_paths.iter().enumerate() {
            jobs.push_back(Job::Seed {
                index,
                path: path.clone(),
            });
        }
        let state = QueueState {
            inner: Mutex::new(QueueInner {
                inflight: seed_paths.len(),
                jobs,
            }),
            cv: Condvar::new(),
        };
        let shared = Shared {
            seen: Mutex::new(seen),
            dependencies: Mutex::new(Vec::new()),
            bundled: Mutex::new(Vec::new()),
            unresolved: Mutex::new(Vec::new()),
            seed_slots: Mutex::new(
                (0..seed_paths.len())
                    .map(|_| None)
                    .collect::<Vec<Option<T>>>(),
            ),
        };

        let threads = options.threads.max(1);
        let drive = || {
            while let Some(job) = pop_job(&state) {
                let guard = FinishGuard::new(&state, options.total_files);
                let counts_as_file = job.counts_as_parsed_file();
                let discovered = self.run_job(job, &parse_seed, &seed_ast, &shared);
                guard.finish(discovered);
                if counts_as_file {
                    on_disk_job_finished();
                }
            }
        };

        if threads == 1 {
            drive();
        } else {
            thread::scope(|scope| {
                for _ in 0..threads {
                    // `&drive` is the closure clippy wants here, but passing
                    // that reference trips `needless_borrows_for_generic_args`.
                    #[allow(clippy::redundant_closure)]
                    scope.spawn(|| drive());
                }
            });
        }

        let seeds = shared
            .seed_slots
            .into_inner()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .into_iter()
            .map(|slot| slot.expect("seed parse job stored its result"))
            .collect();
        ClosedScripts {
            seeds,
            dependencies: shared
                .dependencies
                .into_inner()
                .unwrap_or_else(|poisoned| poisoned.into_inner()),
            bundled: shared
                .bundled
                .into_inner()
                .unwrap_or_else(|poisoned| poisoned.into_inner()),
            unresolved: shared
                .unresolved
                .into_inner()
                .unwrap_or_else(|poisoned| poisoned.into_inner()),
        }
    }

    /// Merges dependency parses into the cache via [`Self::preload`].
    pub fn preload_dependencies(&mut self, dependencies: &[ParsedDependency]) {
        let entries = dependencies
            .iter()
            .map(|dependency| super::PreloadedScript {
                path: &dependency.path,
                name_lower: dependency.name_lower.clone(),
                ast: dependency.ast.as_ref(),
                source: &dependency.source,
            })
            .collect();
        self.preload(entries);
    }

    fn run_job<T>(
        &self,
        job: Job,
        parse_seed: &impl Fn(&Path) -> T,
        seed_ast: &impl Fn(&T) -> Option<&Script>,
        shared: &Shared<T>,
    ) -> Discovery {
        match job {
            Job::Seed { index, path } => {
                let parsed = parse_seed(&path);
                let discovered = match seed_ast(&parsed) {
                    Some(script) => self.discover(referenced_type_names(script), shared),
                    None => Discovery {
                        jobs: Vec::new(),
                        disk_jobs: 0,
                    },
                };
                shared
                    .seed_slots
                    .lock()
                    .unwrap_or_else(|poisoned| poisoned.into_inner())[index] = Some(parsed);
                discovered
            }
            Job::Dependency { name_lower, path } => {
                let parsed = self.parse_dependency(path, name_lower);
                let discovered = match parsed.ast.as_ref() {
                    Some(script) => self.discover(referenced_type_names(script), shared),
                    None => Discovery {
                        jobs: Vec::new(),
                        disk_jobs: 0,
                    },
                };
                shared
                    .dependencies
                    .lock()
                    .unwrap_or_else(|poisoned| poisoned.into_inner())
                    .push(parsed);
                discovered
            }
            Job::Bundled { name_lower } => {
                let ast = crate::ast_cache::ast_for_script_name(self.game, &name_lower);
                let discovered = match ast.as_ref() {
                    Some(script) => self.discover(referenced_type_names(script), shared),
                    None => Discovery {
                        jobs: Vec::new(),
                        disk_jobs: 0,
                    },
                };
                shared
                    .bundled
                    .lock()
                    .unwrap_or_else(|poisoned| poisoned.into_inner())
                    .push(name_lower);
                discovered
            }
        }
    }

    fn discover<T>(&self, names: Vec<String>, shared: &Shared<T>) -> Discovery {
        let mut jobs = Vec::new();
        let mut disk_jobs = 0;
        for name in names {
            let name_lower = name.to_ascii_lowercase();
            if name_lower.is_empty() {
                continue;
            }
            {
                let mut seen = shared
                    .seen
                    .lock()
                    .unwrap_or_else(|poisoned| poisoned.into_inner());
                if !seen.insert(name_lower.clone()) {
                    continue;
                }
            }
            if let Some((path, _)) = self.resolve_script_path_kind(&name_lower) {
                disk_jobs += 1;
                jobs.push(Job::Dependency { name_lower, path });
                continue;
            }
            if crate::ast_cache::contains_script_name(self.game, &name_lower) {
                jobs.push(Job::Bundled { name_lower });
                continue;
            }
            shared
                .unresolved
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner())
                .push(name_lower);
        }
        Discovery { jobs, disk_jobs }
    }

    fn parse_dependency(&self, path: PathBuf, name_lower: String) -> ParsedDependency {
        let (source, ast) = match read_psc_source(&path) {
            Ok(source) => {
                let ast = cached_or_parse(self.game, &path, &source);
                (source, ast)
            }
            Err(_) => (String::new(), None),
        };
        ParsedDependency {
            path,
            name_lower,
            source,
            ast,
        }
    }
}

fn pop_job(state: &QueueState) -> Option<Job> {
    let mut inner = state
        .inner
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    loop {
        if let Some(job) = inner.jobs.pop_front() {
            return Some(job);
        }
        if inner.inflight == 0 {
            return None;
        }
        inner = state
            .cv
            .wait(inner)
            .unwrap_or_else(|poisoned| poisoned.into_inner());
    }
}

fn stem_lower(path: &Path) -> Option<String> {
    path.file_stem()
        .and_then(|stem| stem.to_str())
        .map(|stem| stem.to_ascii_lowercase())
}

fn cached_or_parse(game: papyrus_lint_globals::Game, path: &Path, source: &str) -> Option<Script> {
    if let Some(cached) = crate::ast_cache::get_for_game(game, path, source) {
        return Some(cached);
    }
    let parsed = papyrus_parser::parse(source).ok()?;
    crate::ast_cache::put_for_game(game, path, source, &parsed);
    if let Ok(tokens) = papyrus_parser::tokenize(source) {
        crate::ast_cache::put_tokens_for_game(game, path, source, &tokens);
    }
    Some(parsed)
}

/// Every script type `script` names: its parent, imports, declared types,
/// `new` / `as` / `is`, and the receiver of a `T.Member` expression (a
/// static call or static property). Collected once per parsed AST so lint
/// does not walk bodies to decide what to load.
fn referenced_type_names(script: &Script) -> Vec<String> {
    let mut names = Vec::new();
    let mut seen = HashSet::new();
    let mut push = |name: &str| {
        if name.is_empty() {
            return;
        }
        if seen.insert(name.to_ascii_lowercase()) {
            names.push(name.to_string());
        }
    };

    if let Some(parent) = &script.extends {
        push(parent);
    }
    for import in &script.imports {
        push(&import.name);
    }
    for property in script
        .properties
        .iter()
        .chain(script.groups.iter().flat_map(|group| &group.properties))
    {
        push(&property.type_name.name);
        if let Some(value) = &property.value {
            collect_expr(value, &mut push);
        }
    }
    for variable in &script.variables {
        push(&variable.type_name.name);
        if let Some(value) = &variable.value {
            collect_expr(value, &mut push);
        }
    }
    for member in script
        .structs
        .iter()
        .flat_map(|struct_decl| &struct_decl.members)
    {
        push(&member.type_name.name);
        if let Some(value) = &member.value {
            collect_expr(value, &mut push);
        }
    }
    for function in script
        .functions
        .iter()
        .chain(script.states.iter().flat_map(|state| &state.functions))
    {
        collect_function(function, &mut push);
    }
    names
}

fn collect_function(function: &FunctionDecl, push: &mut impl FnMut(&str)) {
    if let Some(return_type) = &function.return_type {
        push(&return_type.name);
    }
    for param in &function.params {
        push(&param.type_name.name);
        if let Some(default) = &param.default {
            collect_expr(default, push);
        }
    }
    for statement in &function.body {
        collect_stmt(statement, push);
    }
}

fn collect_stmt(statement: &Stmt, push: &mut impl FnMut(&str)) {
    match statement {
        Stmt::VarDecl(variable) => {
            push(&variable.type_name.name);
            if let Some(value) = &variable.value {
                collect_expr(value, push);
            }
        }
        Stmt::Assign { target, value, .. } => {
            collect_expr(target, push);
            collect_expr(value, push);
        }
        Stmt::Expr { value, .. } => collect_expr(value, push),
        Stmt::Return { value, .. } => {
            if let Some(value) = value {
                collect_expr(value, push);
            }
        }
        Stmt::If {
            branches,
            else_body,
            ..
        } => {
            for branch in branches {
                collect_expr(&branch.condition, push);
                for statement in &branch.body {
                    collect_stmt(statement, push);
                }
            }
            for statement in else_body {
                collect_stmt(statement, push);
            }
        }
        Stmt::While {
            condition, body, ..
        } => {
            collect_expr(condition, push);
            for statement in body {
                collect_stmt(statement, push);
            }
        }
        Stmt::LockGuard {
            body, else_body, ..
        } => {
            for statement in body.iter().chain(else_body) {
                collect_stmt(statement, push);
            }
        }
    }
}

fn collect_expr(expr: &Expr, push: &mut impl FnMut(&str)) {
    match expr {
        Expr::Literal(_) | Expr::Identifier(_) | Expr::Self_ | Expr::Parent => {}
        Expr::Binary { left, right, .. } => {
            collect_expr(left, push);
            collect_expr(right, push);
        }
        Expr::Unary { operand, .. } => collect_expr(operand, push),
        Expr::Call { callee, args, .. } => {
            collect_expr(callee, push);
            for arg in args {
                collect_expr(arg, push);
            }
        }
        Expr::NamedArg { value, .. } => collect_expr(value, push),
        Expr::Member { object, .. } => {
            if let Expr::Identifier(name) = object.as_ref() {
                push(name);
            }
            collect_expr(object, push);
        }
        Expr::Index { object, index } => {
            collect_expr(object, push);
            collect_expr(index, push);
        }
        Expr::Cast { value, type_name } | Expr::Is { value, type_name } => {
            push(type_name);
            collect_expr(value, push);
        }
        Expr::NewArray { type_name, size } => {
            push(&type_name.name);
            collect_expr(size, push);
        }
        Expr::NewStruct { type_name } => push(type_name),
    }
}

#[cfg(test)]
#[path = "closure_tests.rs"]
mod tests;
