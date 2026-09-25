# papyrus-lint-lsp

Stdio language server (`PapyrusLinterLsp`). A separate process: do not
route it through Tauri or the CLI.

It lints the in-memory document snapshot, not a re-read from disk, through
`papyrus-lint-live`. Diagnostics use the project `papyrus-lint.yaml` when
one is found by walking up from the file URI. Repairs are LSP edits.

- `textDocument/codeAction` returns a workspace edit for that diagnostic's
  automatic fix, or for a line, file, or project ignore.
- `workspace/executeCommand` `papyrusLint.fixFile` applies every automatic
  fix through `workspace/applyEdit`.

Start at `src/server.rs`, `src/documents.rs`, `src/diagnostics.rs`,
`src/code_actions.rs`, and `src/commands.rs`.

```sh
cargo test --manifest-path app/crates/papyrus-lint-lsp/Cargo.toml
cargo run --manifest-path app/crates/papyrus-lint-lsp/Cargo.toml
```
