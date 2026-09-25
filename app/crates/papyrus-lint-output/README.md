# papyrus-lint-output

Plain-text, JSON, and AI report formatting shared by the CLI and the
desktop app. This crate only formats.

Report links use `papyrus_lints::tags::RuleTags::doc_url`. Do not invent a
second URL scheme. AI export schemas are versioned under `schema/`; old
versions stay frozen.

```sh
cargo test --manifest-path app/crates/papyrus-lint-output/Cargo.toml
```
