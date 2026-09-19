//! Unit tests for [`crate`]'s public lint/repair pipeline (`lint`, `repair`,
//! and the filter/restrict/`@disable` helpers). Split across this directory
//! by the production entry points they cover, matching how `papyrus-lint-cli`'s
//! `run_tests/` is split for `run()`. Per-rule diagnostics stay in each
//! rule's sibling `*_tests.rs`.

mod support;

mod diagnostic;
mod disable;
mod lint;
mod repair;
mod repair_external;
mod restrict_to_line;
mod rule_flags;
mod rule_flags_external;
