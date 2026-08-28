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
   your `PATH`, or a project you build/download it from
   [the releases page](https://github.com/Idrinth/papyrus-lint/releases) —
   or via `cargo build --release --manifest-path
   crates/papyrus-lint-cli/Cargo.toml` in this repository.

## Settings

- SublimeLinter settings: http://www.sublimelinter.com/en/stable/settings.html
- Linter settings: http://www.sublimelinter.com/en/stable/linter_settings.html

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

Diagnostics are read from a `papyrus-lint.yaml`/`.yml` file placed next to
the `.psc` file being linted, the same as the Papyrus Lint desktop app and
CLI — see the main project's README for its format.
