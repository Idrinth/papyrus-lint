# papyrus-lints

Diagnostics, suppression, repair, visitors, and the individual rules.
One rule is `src/<rule>.rs` plus `shared/rules/<id>.json`.

- `check` / `check_with` do not parse or tokenize.
  `registry::collect_diagnostics` does that once and passes
  `Option<&[Token]>` / `Option<&Script>`. `repair()` re-parses after each
  fix. Exception: standalone `check_argument_types` still parses.
- Visitor rules (`visitor()` → `LintVisitor::Ast` or `Tokens`) walk once
  per pass. `none` rules run through `check` only.
- Reuse `const_eval` and `type_flow`. Do not copy them into a new rule.
- Cross-script semantics go through `ExternalSignatures`
  (`external_signatures.rs`). Do not re-derive side-effect flags;
  `papyrus-lint-core` computes them.
- `conflicting-script-versions` owns its diagnostic policy here. Callers
  pass a `ProjectFile` snapshot of the same-named copies, not every script
  in the project. It is not dispatched from `collect_diagnostics`.
- `script-filename-mismatch` owns its diagnostic policy here. Callers pass
  the `.psc` file stem and the lexer tokens (`ScriptName` plus its name
  segments). It is not dispatched from `collect_diagnostics`.
- `lint` / `repair` / `repair_filtered*` / `repaired_line` have no resolver.
  `unused-import` and `argument-naming` are no-ops there. Project callers,
  including desktop preview, use the `*_with_external_arguments` siblings.
  Per-line fix and per-line preview reject line-count-shifting fixes
  (`unused-import`, `property-sorting`).

```sh
cargo test --manifest-path app/crates/papyrus-lints/Cargo.toml
```
