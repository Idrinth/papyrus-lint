# Base-scripts snapshots

Checked-in CLI reports for `shared/scripts/skyrim-scripts.zip` under each
built-in preset. CI's `base-scripts-snapshot` job rebuilds
`PapyrusLinterCLI` from the current commit, unpacks that zip, and fails
when the normalized report no longer matches the summary snapshot here.

Vanilla scripts are *supposed* to produce findings. Do not "fix" a
mismatch by silencing rules; update the snapshot only after reviewing the
delta and deciding the new output is correct.

```console
python3 .github/scripts/base_scripts_snapshot.py \
  --cli path/to/PapyrusLinterCLI \
  --preset strict \
  --update
```

`--all` regenerates every preset at once.
