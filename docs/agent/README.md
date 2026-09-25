# Agent documentation (on demand)

`AGENTS.md` at the repo root is the always-loaded index. Files in this
directory are **not** loaded unless the current task needs them — see the
routing table in `AGENTS.md`. The project layout is shared with human
contributors in [`../project-structure.md`](../project-structure.md).

| File | When to read |
| --- | --- |
| [development.md](development.md) | Frontend, desktop app, editor plugins, or coverage — not per-crate commands |
| [current-state.md](current-state.md) | A change that crosses crates, the GUI, or an editor plugin |

Crate-local invariants and `cargo` commands live in that crate's `README.md`,
not in this directory.
