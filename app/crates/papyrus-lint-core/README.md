# papyrus-lint-core

Project resolution shared by the desktop app and the CLI: achlist / `.ppj`,
script index, `FunctionTable`, compile, and stale `.pex`. This crate does
not depend on Tauri.

- `project_lint::lint_script` is the per-file project pass shared by the CLI
  and the desktop app: project diagnostics, the engine lint, an optional
  compiler check, then `.papyrus-lint-ignore`. Reporting stays in the
  caller. `already_primed` skips the disk AST-cache prime. Desktop per-file
  lint sets `flush_collision_cache`; a batch flushes once itself.
- `project_batch` is the batch runner both call: `parse_closure` (the total
  grows as referenced `.psc` files are enqueued), `preload_closure` (seeds,
  dependencies, and name slots), then `lint_in_parallel` (primes the
  in-memory parser memo, then the caller's per-seed lint, which calls
  `lint_script`). Stdout progress, fix-before-lint, and report folding stay
  in the CLI. Tauri channels, and not linting an unparseable script, stay
  in the desktop app.
- A script name that appears only once is not content-hashed.
  `script_locator` reads a digest from `papyrus-collision-cache` when the
  stored mtime still matches. Conflict buckets use the complete path below
  the search root, so equal stems in different namespace folders stay
  distinct while equal qualified names across roots are compared.
- `.papyrus-lint-ignore` loads from the resolved project root. Exact
  file / line / rule matches are suppressed after lint, project, and
  compiler diagnostics. Relative paths start at that root. Repairs are
  unaffected.
- A `.ppj`'s `<Import>` entries (`ppj::PpjProject::imports`) feed
  `additional_script_roots` for that run or `init` — never
  `lookup_script_roots`. `ppj::parse_ppj` normalizes `\\` to `/` and does
  not decompose an already-absolute Windows or UNC path. This crate only
  parses the file.
- Side-effect flags are computed here (`script_functions`). Lints read
  them through `ExternalSignatures`.
- In Fallout 4 / Starfield, a type or `ScriptName` `A:B:C` resolves to
  `A/B/C.psc` under a script root, an import, or a lookup root. Lookup is
  case-insensitive. A namespaced script that exists is not a
  `Script:Struct`. Skyrim names stay a single segment and match only a
  file sitting directly in a search root. `script_locator`, `FunctionTable`,
  and `.ppj` `<Script>` entries share this mapping.
- Vanilla engine types with no on-disk `.psc` resolve from the bundled AST
  cache by `ScriptName` (`FunctionTable::ensure_loaded` /
  `script_exists`), including a qualified name. Outside known-scripts mode, a
  project or lookup-root file of the same name wins. In known-scripts mode,
  only registered project paths are considered; lookup roots are still
  searched before the bundled cache.
- Known-scripts mode registers each listed path under its file stem, a
  path-derived `folder:stem` name when the file sits under a script root
  (`Scripts/Source/User/Foo.psc` → `user:foo`), and the declared
  `ScriptName`. Qualified names stay distinct; an unqualified stem still
  keeps first-listed-wins. Directory lookup uses that same qualified key
  through the namespaced locator; a nested file is not a bare stem.
  Preload caches every key that still resolves to the file, so `Extends`
  and `Import` of `User:Foo` hit `user:foo`. Project files win over lookup
  roots and the bundled blob.
- `type_exists` treats `T[]` as an array of `T` (not a script named `T[]`),
  `CustomEventName` and `ScriptEventName` as compiler typedefs (no script
  file, same as `String`/`Int`), and `Script:Struct` /
  `Namespace:Script:Struct` as a struct declared on that script. A namespaced
  script that itself exists still wins over the struct reading. A colon
  name that is neither that script nor a struct on a located owner is an
  unresolved reference. The declaring script is parsed in the table's game
  dialect so Fallout 4 / Starfield `Struct` blocks are visible.
  Unqualified struct names are not global: `declares_struct` is that
  script's own structs (what `Import` exposes) and
  `declares_struct_in_ancestry` walks `Extends`. `unresolved-script`
  applies those to the script being linted.
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
