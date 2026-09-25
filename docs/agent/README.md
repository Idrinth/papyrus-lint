# Agent documentation (on demand)

`AGENTS.md` at the repo root is the always-loaded index. Files in this
directory are **not** loaded unless the current task needs them — see the
routing table in `AGENTS.md`. The project layout is shared with human
contributors in [`../project-structure.md`](../project-structure.md).

| File | When to read |
| --- | --- |
| [development.md](development.md) | Tauri app, editor plugins, or coverage — not the desktop UI or per-crate commands |
| [current-state.md](current-state.md) | A change that crosses crates, the GUI, or an editor plugin |

Crate-local notes live in that crate's `README.md`. Desktop UI notes live
in [`../../app/src/README.md`](../../app/src/README.md).
