# Running Papyrus Lint in Docker

Each release also publishes an Alpine Linux CLI image to GitHub Container
Registry. It always scans `/project` recursively, stores the reusable AST
cache in `/cache`, and uses Papyrus base scripts mounted under
`/base-scripts` for cross-script lookups. The base scripts can be an extracted
directory or a zip named `skyrim-scripts.zip`; set
`PAPYRUS_LINT_BASE_SCRIPTS_ARCHIVE` when the mounted archive has another name.
Vanilla scripts whose content still matches the zip shipped in
`shared/scripts/skyrim-scripts.zip` are also served from a pre-compiled AST/token
blob baked into `PapyrusLinterCLI`, so the first analysis does not re-parse
`Actor`/`Form`/… even when the archive was just unpacked into a new path.

```bash
docker run --rm \
  -v "$PWD:/project:ro" \
  -v papyrus-lint-cache:/cache \
  -v "$HOME/skyrim-scripts.zip:/base-scripts/skyrim-scripts.zip:ro" \
  ghcr.io/idrinth/papyrus-lint:latest
```

Additional CLI flags go after the image name — see the
[command-line interface reference](cli.md) for what's available. For
example, append `--json` for JSON output. Omit `:ro` from the project mount and pass `fix` if the
container should apply automatic fixes. A release-specific image tag such as
`v1.2.3` can be used instead of `latest`.

## Verifying image signatures

Container images are signed keylessly by the release workflow. Verify a release tag with `cosign` (replace the tag as
needed):

```bash
cosign verify \
  --certificate-identity \
    https://github.com/Idrinth/papyrus-lint/.github/workflows/release.yml@refs/tags/v1.2.3 \
  --certificate-oidc-issuer https://token.actions.githubusercontent.com \
  ghcr.io/idrinth/papyrus-lint:v1.2.3
```

Every file attached to a release has a matching `.sigstore.json` bundle.
The release workflow signs these bundles keylessly with [Sigstore](https://www.sigstore.dev/),
using GitHub Actions' short-lived OpenID Connect identity, and records the
signature in Sigstore's transparency log. After installing `cosign`, verify a
download by keeping it next to its bundle and running (replace the file name
and tag as needed):

```bash
cosign verify-blob \
  --bundle PapyrusLinterCLI-linux.sigstore.json \
  --certificate-identity \
    https://github.com/Idrinth/papyrus-lint/.github/workflows/release.yml@refs/tags/v1.2.3 \
  --certificate-oidc-issuer https://token.actions.githubusercontent.com \
  PapyrusLinterCLI-linux
```

This verifies both that the downloaded bytes have not changed and that they
were signed by this repository's tag-triggered release workflow. The signing
key is ephemeral, so there is no long-lived release-signing secret to rotate
or expose.
