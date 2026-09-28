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
  pass a `ProjectFile` snapshot of copies with the same qualified identity
  (Fallout 4 / Starfield `A:B:C` is `A/B/C.psc`; see the
  papyrus-lint-core README), not every script that shares a leaf name.
  It is not dispatched from `collect_diagnostics`.
- `script-filename-mismatch` owns its diagnostic policy here. Callers pass
  the `.psc` path relative to its search root and the lexer tokens
  (`ScriptName` plus its name segments). A qualified name must match that
  relative path after dropping leading folders that are not in the
  ScriptName (FO4 `Base/`, a DLC pack folder the name does not include);
  an unqualified name is still the file stem. Same
  mapping as the papyrus-lint-core README. It is not dispatched from
  `collect_diagnostics`.
- `lint` / `repair` / `repair_filtered*` / `repaired_line` have no resolver.
  `unused-import` and `argument-naming` are no-ops there. Project callers,
  including desktop preview, use the `*_with_external_arguments` siblings.
  Per-line fix and per-line preview reject line-count-shifting fixes
  (`unused-import`, `property-sorting`).

```sh
cargo test --manifest-path app/crates/papyrus-lints/Cargo.toml
```
