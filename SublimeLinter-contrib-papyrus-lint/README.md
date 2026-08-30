# SublimeLinter-contrib-papyrus-lint

This package provides an interface to
[Papyrus Lint](https://github.com/Idrinth/papyrus-lint)'s standalone
`PapyrusLinterCLI` binary for [SublimeLinter](http://sublimelinter.com).
It will be used with files that have the `source.papyrus` scope, i.e. a
Papyrus syntax package installed in Sublime Text.

## Installation

1. Install [SublimeLinter](http://www.sublimelinter.com/en/stable/installation.html).
2. Install a Papyrus syntax package, so `.psc` files get the `source.papyrus`
   scope this linter looks for.
3. Install this package, either via
   [Package Control](https://packagecontrol.io) (search for
   `SublimeLinter-contrib-papyrus-lint`) or by cloning/copying this
   directory into your Sublime Text `Packages` directory.
4. Make sure `PapyrusLinterCLI` (or `PapyrusLinterCLI.exe` on Windows) is on
   your `PATH`. You can build it from this repository or download it from
   [the releases page](https://github.com/Idrinth/papyrus-lint/releases) —
   or via `cargo build --release --manifest-path
   app/crates/papyrus-lint-cli/Cargo.toml` in this repository.

## Settings

- [SublimeLinter settings](http://www.sublimelinter.com/en/stable/settings.html)
- [Linter settings](http://www.sublimelinter.com/en/stable/linter_settings.html)

Additionally, this linter supports the standard `executable` setting if
`PapyrusLinterCLI` isn't on your `PATH`:

```json
{
    "linters": {
        "papyrus-lint": {
            "executable": "/path/to/PapyrusLinterCLI"
        }
    }
}
```

Lint configuration is read from `papyrus-lint.yaml`/`.yml` in the project
root inferred by the CLI (two directories above a `.psc` in the conventional
`Scripts/Source` or `Source/Scripts` layout). See the main project's README
for the configuration format. Under the hood, this
linter runs `PapyrusLinterCLI --json` and parses its structured JSON
report rather than scraping plain-text output.

To use a config file somewhere other than that inferred project root, set
`config_path` (either as a linter setting, or per-project) to its path;
this linter (and the fix command below) then passes it to the CLI via
`--config`, overriding the CLI's own discovery:

```json
{
    "linters": {
        "papyrus-lint": {
            "config_path": "/path/to/papyrus-lint.yaml"
        }
    }
}
```

## Fixing files

This package also exposes a "PapyrusLint: Fix Current File" command (via
the Command Palette and the editor's right-click context menu, for a
saved `.psc` file with no unsaved changes) that runs `PapyrusLinterCLI
fix` against the file, applying every automatic fix (see the main
project's README) and rewriting it on disk if anything changed, then
reloads the file and re-lints it.

## Testing

The tests provide lightweight substitutes for the Sublime Text and
SublimeLinter APIs, so they can run with a standard Python installation:

```sh
python -m unittest discover -s tests -v
```
