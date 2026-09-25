# papyrus-lint-config

`papyrus-lint.yaml` discovery, presets, and compiler / game-install
detection.

`configuration/papyrus-lint.default.yaml` must match `init` output. Drift
is a CI failure in this crate.

```sh
cargo test --manifest-path app/crates/papyrus-lint-config/Cargo.toml
```
