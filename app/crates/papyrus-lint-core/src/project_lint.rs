//! Per-file project lint shared by the CLI and the desktop app.
//!
//! Report formatting, tag filters, and progress stay in those callers. This
//! module only runs the sequence both of them were assembling by hand:
//! optional AST-cache prime, project diagnostics, the engine lint (so a
//! `@disable` naming a project diagnostic counts as used), an optional
//! compiler check, then `.papyrus-lint-ignore`.

use std::path::{Path, PathBuf};

use papyrus_lints::{Diagnostic, ExternalSignatures};

use crate::ignore_file::IgnoreFile;
use crate::script_locator::ScriptIndex;
use crate::{ast_cache, collision_cache, compile_diagnostics, compiler};

/// Where same-named copies are taken from for `conflicting_script_versions`.
///
/// The CLI's non-strict scan and the desktop app pass a [`ScriptIndex`].
/// Strict achlist scope passes only the listed same-named paths, which are
/// not search roots.
pub enum ConflictScope<'a> {
    Index {
        index: &'a ScriptIndex,
        short_paths: bool,
    },
    Known {
        candidates: &'a [PathBuf],
        short_paths: bool,
    },
}

/// Inputs for [`lint_script`] besides the one script and its
/// [`ExternalSignatures`] resolver.
pub struct ProjectLint<'a> {
    pub config: &'a papyrus_lints::Config,
    pub project_root: &'a Path,
    pub additional_roots: &'a [String],
    pub conflicts: ConflictScope<'a>,
    pub compile_check: bool,
    pub compiler_path: &'a str,
    pub ignores: Option<&'a IgnoreFile>,
    /// Skip [`ast_cache::ensure_primed_for_game`]. Batch callers set this
    /// after priming `papyrus_parser`'s in-memory memo for this exact
    /// `source`, so the lint does not take the disk cache's lock again.
    pub already_primed: bool,
    /// Write the collision cache before returning. Desktop per-file lint
    /// does; a batch run flushes once itself and leaves this false.
    pub flush_collision_cache: bool,
}

/// Lints `source` for `path` against the rest of the project.
///
/// Project diagnostics are merged through
/// [`papyrus_lints::lint_with_external_arguments_and_extra_diagnostics`], not
/// appended afterward, so `@disable` / `@disable-file` honor them and
/// `unused-disable` counts them as used. A compiler that cannot be started
/// is left out; the engine diagnostics are still returned.
pub fn lint_script<E: ExternalSignatures>(
    path: &Path,
    source: &str,
    external: &mut E,
    lint: &ProjectLint<'_>,
) -> Vec<Diagnostic> {
    if !lint.already_primed {
        ast_cache::ensure_primed_for_game(lint.config.game, path, source);
    }

    let mut project_diagnostics = Vec::new();
    if lint.config.rules.conflicting_script_versions {
        collision_cache::remember_source(lint.config.game, path, source);
        project_diagnostics.extend(conflicts_for(path, lint));
        if lint.flush_collision_cache {
            collision_cache::flush();
        }
    }
    if lint.config.rules.stale_compiled_output {
        project_diagnostics.extend(crate::stale_pex::check(path));
    }
    if lint.config.rules.script_filename_mismatch {
        if let Ok(tokens) = papyrus_parser::tokenize(source) {
            let relative = crate::script_locator::relative_path_in_script_roots(
                path,
                lint.project_root,
                lint.additional_roots,
            )
            .or_else(|| path.file_name().map(PathBuf::from));
            if let Some(relative) = relative {
                project_diagnostics.extend(papyrus_lints::script_filename_mismatch::check(
                    &relative, &tokens,
                ));
            }
        }
    }

    let _peer_scope = crate::function_table::enter_peer_scope(path, source);

    let mut diagnostics = papyrus_lints::lint_with_external_arguments_and_extra_diagnostics(
        source,
        lint.config,
        external,
        project_diagnostics,
    );

    let compiler_path = lint.compiler_path.trim();
    if lint.compile_check && !compiler_path.is_empty() {
        if let Ok(outcome) = compiler::check_psc_file(
            lint.config.game,
            Path::new(compiler_path),
            path,
            lint.additional_roots,
        ) {
            if !outcome.success {
                diagnostics.extend(compile_diagnostics::parse_compile_errors(&outcome));
            }
        }
    }

    if let Some(ignores) = lint.ignores {
        ignores.retain_diagnostics(path, &mut diagnostics);
    }

    diagnostics
}

fn conflicts_for(path: &Path, lint: &ProjectLint<'_>) -> Vec<Diagnostic> {
    match lint.conflicts {
        ConflictScope::Index { index, short_paths } => {
            crate::script_locator::conflicting_script_versions_in_index(
                path,
                index,
                lint.project_root,
                short_paths,
                lint.config.game,
            )
        }
        ConflictScope::Known {
            candidates,
            short_paths,
        } => {
            let qualified_candidates: Vec<_> = candidates
                .iter()
                .filter(|candidate| {
                    crate::script_locator::same_script_identity(
                        path,
                        candidate,
                        lint.project_root,
                        lint.additional_roots,
                    )
                })
                .cloned()
                .collect();
            crate::script_locator::conflicting_script_versions_among(
                path,
                &qualified_candidates,
                lint.project_root,
                short_paths,
                lint.config.game,
            )
        }
    }
}

#[cfg(test)]
#[path = "project_lint_tests.rs"]
mod tests;
