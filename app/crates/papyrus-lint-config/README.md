# papyrus-lint-config

`papyrus-lint.yaml` discovery, presets, compiler detection, and script-root
configuration.

`build.rs` renders the default YAML from `shared/rules/*.json` and the
lint-settings JSON. `init` output must match that render. Drift is a test
failure in this crate.

```sh
cargo test --manifest-path app/crates/papyrus-lint-config/Cargo.toml
```
