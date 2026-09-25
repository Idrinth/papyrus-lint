# papyrus-lint-cli

`PapyrusLinterCLI` argument parsing and lint / fix / init / doctor / blob
orchestration. `src/main.rs` is only the binary adapter.

- `--blob` is in-memory only, via `papyrus-lint-live`: no project root, no
  `FunctionTable`, no fix.
- `.ppj` handling lives in `run_scan.rs`, `doctor/checks.rs`, and
  `init.rs`. `<Import>` entries are `additional_script_roots` for that
  run, never `lookup_script_roots`.

```sh
cargo test --manifest-path app/crates/papyrus-lint-cli/Cargo.toml
cargo run --manifest-path app/crates/papyrus-lint-cli/Cargo.toml -- <path>
cargo build --release --manifest-path app/crates/papyrus-lint-cli/Cargo.toml
```

The release binary is
`app/crates/papyrus-lint-cli/target/release/PapyrusLinterCLI`.
