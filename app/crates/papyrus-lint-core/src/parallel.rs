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
mod tests {
    use super::*;
    use std::collections::HashSet;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::{Barrier, Condvar, Mutex};

    #[test]
    fn default_thread_count_is_never_zero() {
        assert!(default_thread_count() >= 1);
    }

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
        let calls = AtomicUsize::new(0);
        let results: Vec<i32> = map_in_parallel(Vec::new(), 4, |n: i32| {
            calls.fetch_add(1, Ordering::SeqCst);
            n
        });

        assert!(results.is_empty());
        assert_eq!(calls.load(Ordering::SeqCst), 0);
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

    #[test]
    fn a_single_item_runs_on_the_calling_thread_even_with_many_threads_requested() {
        let calling_thread = std::thread::current().id();

        let worker_threads = map_in_parallel(vec!["only"], 8, |_| std::thread::current().id());

        assert_eq!(worker_threads, vec![calling_thread]);
    }

    #[test]
    fn workers_run_items_concurrently() {
        let barrier = Barrier::new(4);
        let calling_thread = std::thread::current().id();

        let worker_threads = map_in_parallel(vec![(); 4], 4, |_| {
            barrier.wait();
            std::thread::current().id()
        });

        let worker_threads = worker_threads.into_iter().collect::<HashSet<_>>();
        assert_eq!(worker_threads.len(), 4);
        assert!(!worker_threads.contains(&calling_thread));
    }

    #[test]
    fn restores_input_order_after_items_finish_in_reverse_order() {
        let all_started = Barrier::new(4);
        let turn = (Mutex::new(3), Condvar::new());
        let completion_order = Mutex::new(Vec::new());

        let results = map_in_parallel(vec![0, 1, 2, 3], 4, |n| {
            all_started.wait();

            let (turn_lock, turn_changed) = &turn;
            let mut current_turn = turn_lock.lock().unwrap();
            while *current_turn != n {
                current_turn = turn_changed.wait(current_turn).unwrap();
            }
            completion_order.lock().unwrap().push(n);
            if n > 0 {
                *current_turn -= 1;
            }
            turn_changed.notify_all();

            format!("result-{n}")
        });

        assert_eq!(*completion_order.lock().unwrap(), vec![3, 2, 1, 0]);
        assert_eq!(results, ["result-0", "result-1", "result-2", "result-3"]);
    }

    #[test]
    fn supports_borrowed_items_results_and_closure_state() {
        let prefix = String::from("script");
        let suffixes = [String::from("one"), String::from("two")];
        let items = suffixes.iter().collect();

        let results = map_in_parallel(items, 2, |suffix| format!("{prefix}-{suffix}"));

        assert_eq!(results, ["script-one", "script-two"]);
    }

    #[test]
    fn more_workers_than_items_still_processes_each_item_once() {
        let counter = AtomicUsize::new(0);

        let results = map_in_parallel(vec![2, 3], 64, |n| {
            counter.fetch_add(1, Ordering::SeqCst);
            n * n
        });

        assert_eq!(results, vec![4, 9]);
        assert_eq!(counter.load(Ordering::SeqCst), 2);
    }

    #[test]
    #[should_panic]
    fn propagates_worker_panics_to_the_caller() {
        map_in_parallel(vec![1, 2], 2, |n| {
            assert_ne!(n, 2, "worker failed");
        });
    }
}
