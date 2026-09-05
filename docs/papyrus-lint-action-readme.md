# Papyrus Lint Action

A GitHub Action that downloads the standalone [`PapyrusLinterCLI`](https://github.com/Idrinth/papyrus-lint)
binary, lints your project, and — when run on a pull request — posts the
findings as inline review comments on the changed lines.

## Usage

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
          path: Data/Source.achlist
```

On a `pull_request` event, any diagnostic that lands on a changed line is
posted as an inline review comment; diagnostics outside the diff are listed
in the review's summary instead. The job fails when `PapyrusLinterCLI` exits
non-zero (i.e. when a diagnostic meets your project's `fail_on_warning`/
`fail_on_info` thresholds in `papyrus-lint.yaml`), unless `fail-on-problems`
is set to `false`.

## Inputs

| Name | Required | Default | Description |
| --- | --- | --- | --- |
| `path` | yes | | Path to the `.achlist` (or a single `.psc` file) to lint. |
| `version` | no | `latest` | papyrus-lint release tag to download `PapyrusLinterCLI` from. |
| `config` | no | | Path to a `papyrus-lint.yaml`/`.yml` config file, passed as `--config`. |
| `script-root` | no | | Newline-separated extra script root directories, each passed as `--script-root`. |
| `github-token` | no | `${{ github.token }}` | Token used to fetch the PR diff and post the review. |
| `create-review` | no | `true` | Whether to post a pull request review on `pull_request` events. |
| `review-event` | no | `AUTO` | `AUTO` (`REQUEST_CHANGES` on failure, otherwise `COMMENT`), `COMMENT`, or `REQUEST_CHANGES`. |
| `fail-on-problems` | no | `true` | Whether to fail the step when the lint run exits non-zero. |
| `cache-ast` | no | `true` | Whether to cache `PapyrusLinterCLI`'s on-disk AST cache between workflow runs, so unchanged scripts skip re-parsing. |

## Outputs

| Name | Description |
| --- | --- |
| `success` | Whether the run succeeded under the configured failure thresholds. |
| `total-diagnostics` | Total number of diagnostics reported. |
| `report-path` | Path to the JSON report written by `PapyrusLinterCLI`. |

## How review comments are kept in sync

Each run deletes review comments it previously posted (identified by a
hidden marker) before posting fresh ones, so re-running the workflow after
pushing a fix doesn't pile up stale comments.

## Speeding up repeated runs with the AST cache

`PapyrusLinterCLI` keeps an on-disk cache of parsed script ASTs (an
`ast-cache` directory next to the binary), reused whenever a script's content
and modification time still match a cached entry. Since `actions/checkout`
resets every file's modification time to the moment of checkout, that cache
would never hit across separate workflow runs on its own. When `cache-ast` is
`true` (the default), this action:

1. Restores each tracked `.psc` file's modification time to the timestamp of
   the commit that last changed it, so the CLI's cache can recognize unchanged
   scripts across runs.
2. Persists the `ast-cache` directory between workflow runs with
   [`actions/cache`](https://github.com/actions/cache), scoped by runner OS
   and the `version` input.

Because step 1 only sees the commit history available in your checkout, use
`actions/checkout` with `fetch-depth: 0` (or another sufficiently large depth)
to get the full benefit across commits that only touch a few files; with the
default shallow checkout, caching still works but mostly helps when the same
commit is linted more than once (e.g. re-run workflows or multiple jobs on
the same commit). Set `cache-ast` to `false` to disable both steps entirely.

## Supported runners

`ubuntu-latest`, `macos-latest`, and `windows-latest` are all supported —
the action picks the matching `PapyrusLinterCLI` release asset for the
runner's OS.

## Development

The pull request review logic in `scripts/post-review.js` has unit tests
under `tests/`, run with:

```sh
npm install
npm test
```

## License

MIT
