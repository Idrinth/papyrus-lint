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
