# Desktop shell

Tauri commands and the `PapyrusLinter` process. Reusable linting stays in
`app/crates/`.

- Filesystem commands that parse, lint, repair, or compile are
  `#[tauri::command(async)]`. Only instant in-memory commands stay sync.
- A dropped batch is `lint_project_scripts`: one parse of the type
  closure, one function-table preload, then in-process parallel lint.
  `lint_psc_file` stays for a single file.
- Per-file commands reuse `cached_script_index` while the source
  directories' mtimes are unchanged.
- `build.rs` generates `icons/` from `shared/images/logo.png`. Those
  platform icons are not checked in, except `icons/icon.png`.

Commands live in `src/files.rs`, `src/lint.rs`, `src/repair.rs`,
`src/export.rs`, and `src/lint_config.rs`.

```sh
cargo test --manifest-path app/src-tauri/Cargo.toml
```
