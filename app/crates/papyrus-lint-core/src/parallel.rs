//! A thin [`rayon`] wrapper for running the same per-item work (reading,
//! optionally fixing, and linting one `.psc` script) across many scripts at
//! once, instead of one at a time.
//!
//! [`map_in_parallel`] builds a scoped `rayon` thread pool sized to the
//! caller's requested thread count (e.g. the CLI's `--threads` flag) and
//! runs `work` over `items` with it, returning the results in the same
//! order the items went in regardless of which order the pool's workers
//! actually finish them in.
//!
//! Used by `papyrus-lint-cli`'s per-script lint loop (see its `--threads`
//! flag), sharing this module (rather than each having its own copy) with
//! the desktop app so both draw on the same tested implementation. The
//! app's per-file Tauri commands (`lint_psc_file`/`repair_psc_file`) don't
//! call this wrapper directly -- the frontend already bounds concurrency
//! itself -- but they do share one `RwLock`-guarded [`crate::function_table::FunctionTable`]
//! per project through [`crate::function_table::SharedFunctionTable`], the
//! same adapter the CLI workers use. See [`crate::ast_cache`]'s module
//! docs for the concurrency-safety work this shares with the app's own
//! already-concurrent command dispatch.

use rayon::prelude::*;
use rayon::ThreadPoolBuilder;

/// The number of worker threads [`map_in_parallel`] should default to when
/// a caller has no more specific preference (e.g. a CLI flag) -- the
/// machine's available parallelism, or `1` if that can't be determined.
pub fn default_thread_count() -> usize {
    std::thread::available_parallelism()
        .map(std::num::NonZeroUsize::get)
        .unwrap_or(1)
}

/// Runs `work` once per item in `items`, using up to `threads` worker
/// threads, and returns the results in the same order as `items` --
/// regardless of which order the threads actually finish their items in.
///
/// `threads <= 1` (or fewer than two items) runs `work` directly on the
/// calling thread instead of spawning any workers at all, so a caller can
/// use this unconditionally (e.g. from a `--threads 1` flag meant to force
/// fully sequential, easier-to-reason-about behavior) without a separate
/// non-parallel code path of its own.
///
/// `work` must be safe to call from multiple threads at once (`Sync`); it
/// closes over whatever shared, thread-safe state each call actually needs
/// (e.g. a `RwLock`-guarded [`crate::function_table::FunctionTable`] via
/// [`crate::function_table::SharedFunctionTable`]) rather than this
/// function knowing anything about lint-specific state itself.
pub fn map_in_parallel<T, R>(
    items: Vec<T>,
    threads: usize,
    work: impl Fn(T) -> R + Sync + Send,
) -> Vec<R>
where
    T: Send,
    R: Send,
{
    if threads <= 1 || items.len() <= 1 {
        return items.into_iter().map(work).collect();
    }

    let pool = ThreadPoolBuilder::new()
        .num_threads(threads)
        .build()
        .expect("failed to build rayon thread pool");

    pool.install(|| items.into_par_iter().map(work).collect())
}

#[cfg(test)]
#[path = "parallel_tests.rs"]
mod tests;
