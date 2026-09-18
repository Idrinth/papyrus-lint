#!/usr/bin/env python3
"""Verify PapyrusLinterCLI Sigstore bundles, then bake their SHA-256 digests.

The editor plugins check SHA-256 at download time. Those digests must come
from binaries the release workflow actually signed, so this script runs
`cosign verify-blob` against each CLI asset and its `.sigstore.json` bundle
before calling write_cli_hashes.
"""

from __future__ import annotations

import argparse
import os
import subprocess
import sys
from collections.abc import Callable, Sequence
from pathlib import Path

from write_cli_hashes import ASSETS, PY_PATH, TS_PATH, collect_hashes, write_hashes

DEFAULT_OIDC_ISSUER = "https://token.actions.githubusercontent.com"

RunCommand = Callable[..., subprocess.CompletedProcess[str]]


class VerificationError(RuntimeError):
    """A CLI asset failed Sigstore verification."""


def bundle_name(asset: str) -> str:
    return f"{asset}.sigstore.json"


def require_signed_assets(asset_dir: Path) -> list[tuple[Path, Path]]:
    missing: list[str] = []
    pairs: list[tuple[Path, Path]] = []
    for asset in ASSETS:
        binary = asset_dir / asset
        bundle = asset_dir / bundle_name(asset)
        if not binary.is_file():
            missing.append(asset)
        if not bundle.is_file():
            missing.append(bundle_name(asset))
        if binary.is_file() and bundle.is_file():
            pairs.append((binary, bundle))
    if missing:
        raise FileNotFoundError("missing signed CLI assets: " + ", ".join(missing))
    return pairs


def verify_blob(
    binary: Path,
    bundle: Path,
    *,
    identity: str,
    oidc_issuer: str,
    cosign: str = "cosign",
    runner: RunCommand | None = None,
) -> None:
    command: Sequence[str] = [
        cosign,
        "verify-blob",
        "--bundle",
        str(bundle),
        "--certificate-identity",
        identity,
        "--certificate-oidc-issuer",
        oidc_issuer,
        str(binary),
    ]
    if runner is None:
        runner = subprocess.run
    try:
        runner(command, check=True, capture_output=True, text=True)
    except subprocess.CalledProcessError as err:
        detail = (err.stderr or err.stdout or "").strip() or str(err)
        raise VerificationError(f"cosign could not verify {binary.name}: {detail}") from err
    except OSError as err:
        raise VerificationError(f"failed to run {cosign}: {err}") from err


def verify_signed_cli_assets(
    asset_dir: Path,
    *,
    identity: str,
    oidc_issuer: str = DEFAULT_OIDC_ISSUER,
    cosign: str = "cosign",
    runner: RunCommand | None = None,
) -> None:
    if runner is None:
        runner = subprocess.run
    for binary, bundle in require_signed_assets(asset_dir):
        verify_blob(
            binary,
            bundle,
            identity=identity,
            oidc_issuer=oidc_issuer,
            cosign=cosign,
            runner=runner,
        )


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "asset_dir",
        type=Path,
        help="directory containing each CLI asset and its .sigstore.json bundle",
    )
    parser.add_argument(
        "--certificate-identity",
        default=os.environ.get("CERTIFICATE_IDENTITY", ""),
        help="Sigstore certificate identity; defaults to $CERTIFICATE_IDENTITY",
    )
    parser.add_argument(
        "--certificate-oidc-issuer",
        default=os.environ.get("CERTIFICATE_OIDC_ISSUER", DEFAULT_OIDC_ISSUER),
        help="Sigstore OIDC issuer; defaults to $CERTIFICATE_OIDC_ISSUER",
    )
    parser.add_argument("--cosign", default=os.environ.get("COSIGN_BIN", "cosign"))
    parser.add_argument("--ts-path", type=Path, default=TS_PATH)
    parser.add_argument("--py-path", type=Path, default=PY_PATH)
    args = parser.parse_args()
    if not args.certificate_identity:
        print("CERTIFICATE_IDENTITY is required (flag or environment).", file=sys.stderr)
        return 2
    try:
        verify_signed_cli_assets(
            args.asset_dir,
            identity=args.certificate_identity,
            oidc_issuer=args.certificate_oidc_issuer,
            cosign=args.cosign,
        )
    except (FileNotFoundError, VerificationError) as err:
        print(err, file=sys.stderr)
        return 1
    hashes = collect_hashes(args.asset_dir)
    write_hashes(hashes, args.ts_path, args.py_path)
    print(f"Verified signatures and wrote CLI SHA-256 digests to {args.ts_path} and {args.py_path}.")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
