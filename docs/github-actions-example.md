# Using PapyrusLinterCLI in a GitHub Action

The preferred way to lint a project in CI is the [Papyrus Lint GitHub
Action](https://github.com/marketplace/actions/papyrus-lint)
(`idrinth/papyrus-lint-action`): it downloads `PapyrusLinterCLI` for you,
handles AST caching between runs itself, and — when run on a pull request —
posts findings as inline review comments on the changed lines. A minimal
workflow using it looks like:

```yaml
name: Papyrus Lint

on: [push, pull_request]

jobs:
  lint:
    runs-on: ubuntu-latest
    permissions:
      contents: read
      pull-requests: write
    steps:
      - uses: actions/checkout@v4
      - uses: idrinth/papyrus-lint-action@v1
        with:
          path: path/to/project.achlist
```

See the action's [Marketplace
listing](https://github.com/marketplace/actions/papyrus-lint) for its full
set of inputs (e.g. `version`, `config`, `script-root`, `fail-on-problems`)
and outputs.

## Downloading the CLI binary manually instead

If you'd rather not depend on the action, download the standalone
`PapyrusLinterCLI` binary attached to the [latest
release](https://github.com/Idrinth/papyrus-lint/releases/latest) and run it
against your project's `.achlist` directly. A minimal workflow that lints on
every push and pull request looks like:

```yaml
name: Papyrus Lint

on: [push, pull_request]

jobs:
  lint:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4

      - name: Download PapyrusLinterCLI
        run: |
          curl -L -o PapyrusLinterCLI \
            https://github.com/Idrinth/papyrus-lint/releases/latest/download/PapyrusLinterCLI-linux
          chmod +x PapyrusLinterCLI

      - name: Cache parsed script ASTs
        uses: actions/cache@v4
        with:
          path: ast-cache
          key: papyrus-lint-ast-cache-${{ github.run_id }}
          restore-keys: |
            papyrus-lint-ast-cache-

      - name: Lint scripts
        run: ./PapyrusLinterCLI path/to/project.achlist
```

The job fails whenever the lint run's exit code is non-zero (see the
[Command-line interface](../README.md#command-line-interface) section of the
README for what each exit code means), so a project's
`papyrus-lint.yaml`/`.yml` `fail_on_warning`/`fail_on_info` settings decide
whether warnings/info-level findings fail the build the same way they do
locally.

`PapyrusLinterCLI` caches each script's parsed AST on disk in an `ast-cache`
directory next to its own binary, keyed by each file's content and
modification time — so a cached entry from an unchanged file is always safe
to reuse and re-parsing it can be skipped. Since every run above downloads a
fresh binary into a fresh runner, that cache starts out empty each time
unless it's persisted between runs, which is what the `actions/cache` step
does: it restores the most recently saved `ast-cache` directory (via
`restore-keys`, since the `key` itself — suffixed with the unique
`github.run_id` — never matches an existing cache) before linting, then
saves the updated directory under that run's key afterwards. Drop this step
if you'd rather not spend cache storage on it; linting still works
correctly, just without the speedup on unchanged scripts across runs.

To also enforce automatic fixes are applied and commit the results, replace
the last step's `PapyrusLinterCLI` invocation with `PapyrusLinterCLI fix`, or
add a separate `PapyrusLinterCLI --json --output report.json` step to save a
machine-readable report (see the [JSON
Schema](papyrus-lint-report.schema.json)) as a build artifact instead.
