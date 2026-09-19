<!-- Extracted so docs/agent/ci.md stays focused on ci.yml. -->
# Base scripts snapshot
(`.github/workflows/base-scripts.yml`)

Builds release `PapyrusLinterCLI` once, unpacks
`shared/scripts/skyrim-scripts.zip`, and runs the CLI with each built-in
preset (`strict`, `standard`, `careful`) against that corpus using
`configuration/presets/papyrus-lint.<preset>.yaml`.

The normalized `--json` report is compared to
`testdata/base-scripts/<preset>.summary.txt` (SHA-256 of the full listing
plus per-rule counts). Findings on vanilla scripts are expected — the job
fails only when the report drifts from the snapshot or the CLI exits 2+
(usage, I/O, crash).

After all three quick summary checks finish, the workflow builds the CLI from
the pull request's base revision (or the preceding revision on a push) and
runs both that binary and the current binary with plain-text output. This runs
even when a summary check found drift, so a unified line-by-line diff can show
the exact change for every preset: `-` lines only occurred in the base run,
while `+` lines only occur in the current run. Failed reports are uploaded in
the `base-scripts-text-reports` artifact.

Regenerate after a deliberate change:

```console
python3 .github/scripts/base_scripts_snapshot.py \
  --cli path/to/PapyrusLinterCLI \
  --preset strict \
  --update
```

`--all` rewrites every preset. On mismatch the raw CLI JSON under
`base-scripts-work/` is uploaded as `base-scripts-work-<preset>`.
