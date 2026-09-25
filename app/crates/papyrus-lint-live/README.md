# papyrus-lint-live

In-memory source linting for CLI `--blob`, the LSP document snapshot, and
desktop live edit.

No project root, no `FunctionTable`, and no fix. Unsaved buffers use this
path; saved files use the full CLI.

```sh
cargo test --manifest-path app/crates/papyrus-lint-live/Cargo.toml
```
