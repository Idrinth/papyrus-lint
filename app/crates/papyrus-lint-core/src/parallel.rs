//! A small, dependency-free worker pool for running the same per-item work
//! (reading, optionally fixing, and linting one `.psc` script) across many
//! scripts at once, instead of one at a time.
//!
//! This is deliberately minimal rather than pulling in a crate like
//! `rayon`: the only thing needed is "run this closure for every item,
//! spread across a bounded number of threads, and get the results back in
//! the same order the items went in" -- [`map_in_parallel`] is exactly
//! that, built on [`std::thread::scope`] so it never needs `'static` bounds
//! or to hand ownership of `items` to the caller ahead of time.
//!
//! Used by `papyrus-lint-cli`'s per-script lint loop (see its `--threads`
//! flag), sharing this module (rather than each having its own copy) with
//! the desktop app so both draw on the same tested implementation, even
//! though the app's own per-file Tauri commands (`lint_psc_file`/
//! `repair_psc_file`) don't call it directly today -- see
//! [`crate::ast_cache`]'s module docs for the concurrency-safety work this
//! shares with the app's own already-concurrent command dispatch.

use std::collections::VecDeque;
use std::sync::Mutex;

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
/// (e.g. a `Mutex`-guarded [`crate::function_table::FunctionTable`] via
/// [`crate::function_table::SharedFunctionTable`]) rather than this
/// function knowing anything about lint-specific state itself.
pub fn map_in_parallel<T, R>(items: Vec<T>, threads: usize, work: impl Fn(T) -> R + Sync) -> Vec<R>
where
    T: Send,
    R: Send,
{
    if threads <= 1 || items.len() <= 1 {
        return items.into_iter().map(work).collect();
    }

    let queue: Mutex<VecDeque<(usize, T)>> = Mutex::new(items.into_iter().enumerate().collect());
    let results: Mutex<Vec<(usize, R)>> = Mutex::new(Vec::new());

    std::thread::scope(|scope| {
        for _ in 0..threads {
            scope.spawn(|| loop {
                let next = queue
                    .lock()
                    .unwrap_or_else(|poisoned| poisoned.into_inner())
                    .pop_front();
                let Some((index, item)) = next else {
                    break;
                };
                let result = work(item);
                results
                    .lock()
                    .unwrap_or_else(|poisoned| poisoned.into_inner())
                    .push((index, result));
            });
        }
    });

    let mut results = results
        .into_inner()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    results.sort_by_key(|(index, _)| *index);
    results.into_iter().map(|(_, result)| result).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};

    #[test]
    fn preserves_input_order_regardless_of_thread_count() {
        let items: Vec<i32> = (0..50).collect();
        let results = map_in_parallel(items.clone(), 8, |n| n * 2);
        let expected: Vec<i32> = items.iter().map(|n| n * 2).collect();
        assert_eq!(results, expected);
    }

    #[test]
    fn single_thread_runs_sequentially_on_the_calling_thread() {
        let order = Mutex::new(Vec::new());
        let items: Vec<i32> = (0..5).collect();
        let results = map_in_parallel(items, 1, |n| {
            order.lock().unwrap().push(n);
            n
        });
        assert_eq!(results, vec![0, 1, 2, 3, 4]);
        assert_eq!(*order.lock().unwrap(), vec![0, 1, 2, 3, 4]);
    }

    #[test]
    fn zero_threads_is_treated_as_sequential() {
        let results = map_in_parallel(vec![1, 2, 3], 0, |n| n + 1);
        assert_eq!(results, vec![2, 3, 4]);
    }

    #[test]
    fn empty_input_returns_empty_output() {
        let results: Vec<i32> = map_in_parallel(Vec::new(), 4, |n: i32| n);
        assert!(results.is_empty());
    }

    #[test]
    fn every_item_is_processed_exactly_once_under_contention() {
        let counter = AtomicUsize::new(0);
        let items: Vec<i32> = (0..200).collect();
        let results = map_in_parallel(items, 16, |n| {
            counter.fetch_add(1, Ordering::SeqCst);
            n
        });
        assert_eq!(counter.load(Ordering::SeqCst), 200);
        assert_eq!(results, (0..200).collect::<Vec<_>>());
    }
}
