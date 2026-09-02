# SublimeLinter-contrib-papyrus-lint

This package provides an interface to
[Papyrus Lint](https://github.com/Idrinth/papyrus-lint)'s standalone
`PapyrusLinterCLI` binary or the desktop app's `PapyrusLinter` executable for
[SublimeLinter](http://sublimelinter.com).
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
4. The plugin automatically downloads and caches the platform-specific
   `PapyrusLinterCLI` from the GitHub release matching the plugin version. To
   use a locally installed CLI or desktop app executable instead, configure
   it as described below.

## Settings

- [SublimeLinter settings](http://www.sublimelinter.com/en/stable/settings.html)
- [Linter settings](http://www.sublimelinter.com/en/stable/linter_settings.html)

By default, the linter downloads its matching release CLI on first use and
reuses the copy in Sublime Text's cache. Its standard `executable` setting can
instead select either a standalone CLI at another location or the desktop
app's `PapyrusLinter` executable; both lint and fix commands honor this setting:

```json
{
    "linters": {
        "papyrus-lint": {
            "executable": "/path/to/PapyrusLinter"
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

## Contact

- Discord: <https://discord.gg/idrinth>
- NexusMods: <https://www.nexusmods.com/skyrimspecialedition/mods/189862>
- GitHub: <https://github.com/idrinth/papyrus-lint>
