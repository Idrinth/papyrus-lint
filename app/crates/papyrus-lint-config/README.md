# papyrus-lint-config

`papyrus-lint.yaml` discovery, presets, compiler detection, and script-root
configuration.

`configuration/papyrus-lint.default.yaml` must match `init` output. Drift
is a CI failure in this crate.

```sh
cargo test --manifest-path app/crates/papyrus-lint-config/Cargo.toml
```
