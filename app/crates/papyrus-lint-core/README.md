# papyrus-lint-core

Project resolution shared by the desktop app and the CLI: achlist / `.ppj`,
script index, `FunctionTable`, compile, and stale `.pex`. This crate does
not depend on Tauri.

- A script name that appears only once is not content-hashed.
  `script_locator` reads a digest from `papyrus-collision-cache` when the
  stored mtime still matches.
- `.papyrus-lint-ignore` loads from the resolved project root. Exact
  file / line / rule matches are suppressed after lint, project, and
  compiler diagnostics. Relative paths start at that root. Repairs are
  unaffected.
- A `.ppj`'s `<Import>` entries (`ppj::PpjProject::imports`) feed
  `additional_script_roots` for that run or `init` — never
  `lookup_script_roots`. `ppj::parse_ppj` normalizes `\` to `/` and does
  not decompose an already-absolute Windows or UNC path. This crate only
  parses the file.
- Side-effect flags are computed here (`script_functions`). Lints read
  them through `ExternalSignatures`.
- Vanilla engine types with no on-disk `.psc` resolve from the bundled AST
  cache by `ScriptName` (`FunctionTable::ensure_loaded` /
  `script_exists`). A project or lookup-root file of the same name wins.
- `FunctionTable::parse_type_closure` runs before linting. Referenced
  `.psc` files are preloaded for analysis only, not as lint targets.
  Bundled names come from the blob. Names that resolve nowhere stay cached
  unresolved. `SharedFunctionTable` write-locks only a name the closure
  never saw.
- `has_event` answers from event names on a fully resolved `Extends`
  chain (own events plus ancestors, including state-only events). A
  same-named function does not hide an event. An incomplete chain is not
  answered from the index. The index drops when a chained script's mtime
  changes, or when a live search directory's mtime changes because a
  script appeared or disappeared.

```sh
cargo test --manifest-path app/crates/papyrus-lint-core/Cargo.toml
```
