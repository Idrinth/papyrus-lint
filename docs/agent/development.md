<!-- Extracted from AGENTS.md so the always-on agent index stays small. -->
# Development

Crate-local test and run commands live in that crate's `README.md`
(`app/crates/*/README.md`, `app/src-tauri/README.md` for the desktop
shell, and `app/src/README.md` for the UI). Do not copy them here.

- Rule metadata: `shared/rules.json` (read by the Rust crates' build
  scripts, `pages/build.py`, and its own tests) is generated, not
  checked in — run `python3 .github/scripts/build_rules_json.py` after
  cloning and again whenever a `shared/rules/*.json` file changes, before
  any of the commands below that touch it.
- Full desktop app: `npm run tauri dev` / `npm run tauri build` (from `app/`).
  The desktop shell is built with [Tauri](https://tauri.app/), so building
  it requires Tauri's platform prerequisites (a Rust toolchain, plus the
  usual webview dependencies for your OS — see the
  [Tauri prerequisites guide](https://v2.tauri.app/start/prerequisites/)).
  Rust-only checks for that shell are in `app/src-tauri/README.md`.
- VS Code extension (`vscode-extension/`): `npm install`, then `npm run
  watch` (or `npm run compile` for a one-off build) and F5 in VS Code to
  launch an Extension Development Host. Not part of the app's npm
  project — it has its own `package.json`/`tsconfig.json`/`eslint.config.js`.
- Rust coverage for a linting crate: `cargo llvm-cov --manifest-path
  <crate>/Cargo.toml` (requires the
  [`cargo-llvm-cov`](https://github.com/taiki-e/cargo-llvm-cov) subcommand
  and the `llvm-tools-preview` rustup component).
