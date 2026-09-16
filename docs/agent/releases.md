<!-- Extracted from AGENTS.md so the always-on agent index stays small. -->
# Releases
 (`.github/workflows/release.yml`)

Pushing a tag matching `v*.*.*` triggers a release job that syncs the
tag's version into `app/src-tauri/tauri.conf.json`, `app/package.json`,
`app/src-tauri/Cargo.toml`, and all four reusable crates' `Cargo.toml` files, then
builds the Tauri desktop app (binary name `PapyrusLinter`) on Linux,
macOS, and Windows (via `tauri-apps/tauri-action`) and the
`PapyrusLinterCLI` CLI binary (via `cargo build --release --manifest-path
app/crates/papyrus-lint-cli/Cargo.toml`) on each platform, attaching each
platform's desktop bundle and CLI binary
(`PapyrusLinterCLI-linux`/`PapyrusLinterCLI-macos`/`PapyrusLinterCLI-windows.exe`)
to a GitHub release for that tag, creating the release if it doesn't
already exist. The `ubuntu-latest` leg also copies the checked-in
`docs/papyrus-lint.default.yaml` (see Configuration above) to
`papyrus-lint.yaml` and attaches it to the release alongside the CLI
binary, rather than generating it by running the freshly built CLI's
`init` subcommand. A separate `editor-plugins` job runs independently,
packages the VS Code extension into a `.vsix` (via `@vscode/vsce`)
and the `SublimeLinter-contrib-papyrus-lint` directory into a `.zip`, and
attaches both to the same release. A final `release-notes` job (after
both `release` and `editor-plugins` succeed) overwrites the release's
title and body — replacing the generic body `tauri-apps/tauri-action`
set on the `release` job — with the tag name as the title; a changelist
of the merged pull requests between the previous and current tag,
resolved per commit via the "list pull requests associated with a
commit" GitHub API and linked with the PR title as text, prefixed with
that pull request's `type: *` label(s) (e.g. `[Feature]`, or
`[Feature, Tests]` when a pull request carries more than one) when it
carries any — a pull request with none is listed with no such prefix;
the current
code coverage (aggregated the same way as CI's coverage-comment job,
via `.github/scripts/coverage_summary.py`, from the lcov artifacts of
the most recent successful `ci.yml` run for the tagged commit); and a
link to the full changelist (`.../compare/<previous-tag>...<tag>`). The
changelist itself is grouped into a section per `component: *` label
(see Pull request labels below) in a fixed order — Sublime Text Plugin,
VS Code Extension, Frontend, Linting, CI, Parsing, Pages, GUI, then CLI —
with a pull request carrying more than one of those labels listed under
every matching component in that order; a pull request whose only
matching label is `component: documentation` is left out of the release
notes entirely, since documentation changes are tracked elsewhere, while
one labeled `component: documentation` alongside other component labels
is still listed under every one of those other components. A pull request
matching none of
the labels above falls into a trailing "Other" section instead of
failing the job. It also builds a plain-text version of the same PR
changelist (titles only, no PR numbers or links) and uploads it as the
`nexus-changelog` artifact for the `nexus-upload` job below; a pull
request is left out of this version if it carries `component:
documentation`, `component: pages`, `component: ci`, `type: tests`,
`type: documentation`, or `type: dependency` — regardless of what else
it's labeled, since none of those describe anything a Nexus downloader
would notice, unlike the
GitHub release notes above where a `component: ci`/`component: pages`
pull request still gets its own section.

A final `nexus-upload` job (after `release`, `editor-plugins`, and
`release-notes` all succeed) publishes the release to the project's
[Nexus Mods page](https://www.nexusmods.com/skyrimspecialedition/mods/189862),
authenticating with the `NEXUSMODS_API_KEY` repo secret, via the
[`Nexus-Mods/upload-action`](https://github.com/Nexus-Mods/upload-action).
It downloads the already-built assets straight off the GitHub release
(rather than rebuilding anything) — the Windows installer (`*setup.exe`),
`PapyrusLinterCLI-windows.exe`, the `docs/papyrus-lint.default.yaml` copy
uploaded as `papyrus-lint.yaml` (zipped locally, since Nexus expects it
as an archive), the SublimeLinter plugin `.zip`, and the VS Code
extension `.vsix` (also zipped, for the same reason) — and uploads each
as a new version of its corresponding Nexus mod file: the two
executables as `main` files, the editor plugins as `optional`, and the
zipped config as `miscellaneous`. The setup.exe upload also sets
`primary_mod_manager_download` and `update_mod_version`, since it's the
mod's primary download and drives the mod-level version shown on the
page; it additionally passes `mod_id` and the downloaded
`nexus-changelog` artifact's content as `changelog`, so that same
upload also posts the version's changelog entry to the mod page (the
action requires `mod_id` whenever `changelog` is set). The Nexus API has
no endpoint to update a mod's page description, so
`docs/nexuspage.bbcode` is not synced by this job and still needs to be
pasted onto the mod page by hand.

A `virustotal-scan` job (after `release`, `sublime-plugin`, `vscode-plugin`,
and `release-notes` all succeed) submits every executable and archive
attached to the release — the three `PapyrusLinterCLI-*` binaries, each
platform's desktop installer/package (`*setup.exe`, `.msi`, `.rpm`,
`.deb`, `.AppImage`, `.dmg`, `.app.tar.gz`), the VS Code `.vsix`, and the
SublimeLinter plugin `.zip` — to VirusTotal for scanning, via
[`cssnr/virustotal-action`](https://github.com/cssnr/virustotal-action),
authenticating with the `VIRUSTOTAL_API_KEY` repo secret. It downloads
just those named assets off the release (the same set
`sign-release-assets` signs) rather than scanning every attached file, so
non-binary assets (the Nexus page, the default config, the
`sign-release-assets` job's own `.sigstore.json` signatures) are left
out; it runs independently of `sign-release-assets` itself, since
scanning and signing don't touch the same release data. It resolves the
tag's release id itself (`gh api repos/.../releases/tags/<tag>`) rather
than relying on a `release` event's own context, since this workflow
triggers on a tag push instead. `update_release: true` has the action
append a "🛡️ VirusTotal Results" section, linking each scanned file to
its VirusTotal report, directly onto the release notes — which is why
this job `needs` `release-notes` and runs after it rather than
alongside it: `release-notes` overwrites the release body wholesale, so
scanning any earlier would have its appended results immediately
discarded.

An `update-pages` job (after `release`) triggers `pages.yml` (see GitHub
Pages above) via `gh workflow run pages.yml --ref the-one -f version=...`,
passing the tag (`github.ref_name`) as its `version` input; this is the
only thing that ever deploys the site, since `pages.yml` has no `push`
trigger of its own (see GitHub Pages above for why). It dispatches a
separate run pinned to `the-one` rather than invoking
`pages.yml` in-line as a `workflow_call` (which would otherwise seem the
more obvious choice, and once ran that way): a reusable `workflow_call`
runs on the caller's own ref, which for this tag-triggered workflow is
the tag itself rather than a branch, and the `deploy` job's
`github-pages` environment has a deployment branch policy that only
allows `the-one` — so that run always failed with "Branch/tag not
allowed to deploy to github-pages due to environment protection rules"
regardless of the tag's actual content. Dispatching `pages.yml` to run
on `the-one` keeps the ref a branch the environment allows, at the cost
of the dispatched run no longer being nested under this workflow run (it
shows up as its own `GitHub Pages` run) and needing its own `actions:
write` permission to fire the dispatch.

