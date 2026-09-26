# SublimeLinter-contrib-papyrus-lint

This package provides an interface to
[Papyrus Lint](https://github.com/Idrinth/papyrus-lint)'s standalone
`PapyrusLinterCLI` binary or the desktop app's `PapyrusLinter` executable for
[SublimeLinter](http://sublimelinter.com).
It runs on saved `.psc` files, and on buffers whose syntax package assigns one
of the common Papyrus scopes (`source.papyrus`, `source.papyrus.skyrim`,
`source.papyrus.fallout4`, or `source.papyrusf4`). A syntax package is still
useful for highlighting, but it is no longer required for the linter to attach.
Each reported diagnostic's message ends with a link to that rule's own
documentation on the [project website](https://papyrus-lint.idrinth.de) when
available.

## Installation

1. Install [SublimeLinter](http://www.sublimelinter.com/en/stable/installation.html).
2. Optionally install a Papyrus syntax package. The linter activates on `.psc`
   files even when that package names the language something other than
   `source.papyrus`, or when no syntax package is installed at all.
3. Install this package, either via by cloning/copying this directory into your
   Sublime Text `Packages` directory.
4. The plugin automatically downloads and caches the platform-specific
   `PapyrusLinterCLI` from the GitHub release matching the plugin version.
   Updating the plugin downloads that new release's CLI (and drops the
   previously cached copy). To use a locally installed CLI or desktop app
   executable instead, configure it as described below.

## Settings

- [SublimeLinter settings](http://www.sublimelinter.com/en/stable/settings.html)
- [Linter settings](http://www.sublimelinter.com/en/stable/linter_settings.html)

By default, the linter downloads its matching release CLI when the package
is loaded (and again when the package is updated to a new version), then
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

The manually configured executable's SHA-256 must match this plugin
release's baked `PapyrusLinterCLI` or `PapyrusLinter` digest, and its
`--version` output must match this plugin's version. A mismatched binary
is rejected before linting, fixing, or initializing a configuration.

Lint configuration is read from `papyrus-lint.yaml`/`.yml` in the project
root inferred by the CLI (the directory above a `Scripts/Source` or
`Source/Scripts` pair, or the nearest ancestor that already has a config
file). See the main project's [configuration
reference](https://github.com/Idrinth/papyrus-lint/blob/the-one/docs/configuration.md)
for the format and exact resolution order.

## Live linting

A saved `.psc` file with no unsaved changes gets the full project-aware lint,
including cross-script checks and the project's own configuration. A view
with unsaved changes is linted from its current contents, so
[SublimeLinter's background linting](http://www.sublimelinter.com/en/stable/lint_modes.html)
(as configured by the standard `lint_mode` setting) shows live feedback on
what's actually in the editor. Live results skip cross-script checks and use
the default configuration unless `config_path` is set. Saving restores the
full project-aware lint.

To use a config file somewhere other than that inferred project root, set
`config_path` (either as a linter setting or per-project) to its path. This
also applies to fixes and overrides automatic configuration discovery:

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
saved `.psc` file with no unsaved changes). It applies every automatic fix
(see the main project's README), reloads the file if anything changed, and
checks it again.

A "PapyrusLint: Fix This Issue" command is available the same way, for
fixing just the diagnostic under (or nearest to) the caret instead of the
whole file. It selects the issue on the caret's line closest to its column
and applies only that fix; every other issue is left untouched. If the
caret's line has no reported issue, or the issue there has no automatic
fix, an error message explains why nothing changed.

## Initializing a project

A "PapyrusLint: Initialize Configuration" command (Command Palette only,
since it's a project-wide action rather than one scoped to the current
file) creates a `papyrus-lint.yaml` in a project without overwriting an
existing one. It picks a target directory from the window's open folder
(prompting when more than one is open, or falling back to the active file's
own directory when no folder is open at all), then prompts for a preset to
start from — the built-in `strict` (the default), `standard`, or `careful`,
or a custom preset name added via `PapyrusLinterCLI preset add` or the desktop
app's "Save current settings as preset…" button.

## Testing

The tests provide lightweight substitutes for the Sublime Text and
SublimeLinter APIs, so they can run with a standard Python installation:

```sh
python -m unittest discover -s tests -v
```

## Contact

<!--CONTACT-LINKS-->
