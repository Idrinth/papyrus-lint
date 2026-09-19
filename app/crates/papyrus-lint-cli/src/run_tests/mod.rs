//! Integration-style tests for the top-level `run()` pipeline (plain lint,
//! `fix`, `--json`/`--format ai`, `--tag`/`--type`, threading, color, and
//! output-file behavior). Kept separate from the command-shaped
//! `run_scan`/`run_lint`/`run_fix` modules since these tests exercise `run()`
//! end to end rather than any one of them in isolation, and split by
//! scenario across this directory's files since it doesn't map to any one
//! of them either.

mod basic;
mod compile_check;
mod cross_script_resolution;
mod fix;
mod ppj;
mod script_roots_and_config;
mod stale_output_and_filename_checks;
mod tag_filter;
mod threading;
