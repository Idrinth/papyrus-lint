# Using PapyrusLinterCLI in a GitHub Action

There's no dedicated GitHub Action for Papyrus Lint (yet) — instead, download
the standalone `PapyrusLinterCLI` binary attached to the [latest
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

      - name: Lint scripts
        run: ./PapyrusLinterCLI path/to/project.achlist
```

The job fails whenever the lint run's exit code is non-zero (see the
[Command-line interface](../README.md#command-line-interface) section of the
README for what each exit code means), so a project's
`papyrus-lint.yaml`/`.yml` `fail_on_warning`/`fail_on_info` settings decide
whether warnings/info-level findings fail the build the same way they do
locally.

To also enforce automatic fixes are applied and commit the results, replace
the last step's `PapyrusLinterCLI` invocation with `PapyrusLinterCLI fix`, or
add a separate `PapyrusLinterCLI --json --output report.json` step to save a
machine-readable report (see the [JSON
Schema](papyrus-lint-report.schema.json)) as a build artifact instead.
