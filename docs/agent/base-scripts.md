<!-- Extracted so docs/agent/ci.md stays focused on ci.yml. -->
# Base scripts snapshot
(`.github/workflows/ci.base-scripts.yml`)

Builds release `PapyrusLinterCLI` once, unpacks
`shared/scripts/skyrim-scripts.zip`, and runs the CLI with each built-in
preset (`strict`, `standard`, `careful`) against that corpus using
`configuration/presets/papyrus-lint.<preset>.yaml`.

The CLI runs in plain-text mode and its output is compared to
`fixtures/<preset>.txt`. Line order is ignored, while duplicate occurrences
remain significant. A mismatch prints removed (`-`) and added (`+`) lines and
exits with status 1. Findings on vanilla scripts are expected — the job fails
only when the output differs from its baseline or the CLI exits 2+ (usage,
I/O, crash).

Regenerate after a deliberate change:

```console
python3 .github/scripts/base_scripts_snapshot.py \
  --cli path/to/PapyrusLinterCLI \
  --preset strict \
  --update
```

`--all` rewrites every preset. The baseline files are intentionally not yet
checked in; until they are, comparison reports each missing baseline and exits
with status 1.
