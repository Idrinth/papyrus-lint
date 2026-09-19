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

Regenerate after a deliberate change:

```console
python3 .github/scripts/base_scripts_snapshot.py \
  --cli path/to/PapyrusLinterCLI \
  --preset strict \
  --update
```

`--all` rewrites every preset. On mismatch the raw CLI JSON under
`base-scripts-work/` is uploaded as `base-scripts-work-<preset>`.
