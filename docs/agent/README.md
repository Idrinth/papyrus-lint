# Agent documentation (on demand)

`AGENTS.md` at the repo root is the always-loaded index. Files in this
directory are **not** loaded unless the current task needs them — see the
routing table in `AGENTS.md`. The project layout is shared with human
contributors in [`../project-structure.md`](../project-structure.md).

| File | When to read |
| --- | --- |
| [development.md](development.md) | Running tests, coverage, or the desktop app |
| [ci.md](ci.md) | Changing `.github/workflows/ci.yml` or CI scripts |
| [pages.md](pages.md) | Changing `pages/` or the GitHub Pages workflow |
| [releases.md](releases.md) | Changing `.github/workflows/release.yml` |
| [current-state.md](current-state.md) | Changing parser, lints, CLI, GUI, or editor plugins |
