"""Download and cache the PapyrusLinterCLI matching this plugin release."""

import hashlib
import json
import os
import platform
import shutil
import tempfile
import threading
from contextlib import suppress
from pathlib import Path
from urllib.request import urlopen

import sublime

try:
    from .cli_hashes import CLI_SHA256
except ImportError:
    from cli_hashes import CLI_SHA256

RELEASE_BASE = 'https://github.com/Idrinth/papyrus-lint/releases/download'

_download_lock = threading.Lock()


def _asset_name(system=None):
    system = system or platform.system()
    assets = {
        'Windows': 'PapyrusLinterCLI-windows.exe',
        'Darwin': 'PapyrusLinterCLI-macos',
        'Linux': 'PapyrusLinterCLI-linux',
    }
    if system not in assets:
        raise OSError(f'Papyrus Lint does not publish a CLI for {system}')
    return assets[system]


def _package_dir():
    return Path(__file__).resolve().parent


def _version_from_metadata_text(text):
    try:
        return json.loads(text)['version'].removeprefix('v')
    except (KeyError, TypeError, ValueError):
        return None


def release_version():
    # Prefer files on disk: Package Control extracts an update before
    # Sublime's resource cache necessarily reflects it, so load_resource
    # can keep returning the previous package-metadata.json and we'd
    # reuse that older CLI instead of downloading the new one.
    try:
        version = _version_from_metadata_text(
            (_package_dir() / 'package-metadata.json').read_text(encoding='utf-8')
        )
        if version:
            return version
    except OSError:
        pass
    try:
        version = _version_from_metadata_text(
            sublime.load_resource(
                'Packages/SublimeLinter-contrib-papyrus-lint/package-metadata.json'
            )
        )
        if version:
            return version
    except (FileNotFoundError, OSError):
        pass
    # Release archives carry VERSION; Package Control installs additionally
    # expose their tag-derived version through package-metadata.json.
    return (_package_dir() / 'VERSION').read_text(encoding='utf-8').strip().removeprefix('v')


def expected_sha256(asset):
    expected = CLI_SHA256.get(asset)
    if not expected:
        raise OSError(f'no baked SHA-256 for {asset}')
    return expected


def _sha256_file(path):
    digest = hashlib.sha256()
    with open(path, 'rb') as handle:
        while True:
            chunk = handle.read(1024 * 1024)
            if not chunk:
                break
            digest.update(chunk)
    return digest.hexdigest()


def _assert_expected_sha256(path, asset):
    actual = _sha256_file(path)
    expected = expected_sha256(asset)
    if actual != expected:
        raise OSError(
            f'{asset} SHA-256 mismatch (expected {expected}, got {actual}); '
            'refusing to use a manipulated file'
        )


def _is_usable(executable, asset):
    if not executable.is_file() or (os.name != 'nt' and not os.access(str(executable), os.X_OK)):
        return False
    try:
        _assert_expected_sha256(str(executable), asset)
    except OSError:
        return False
    return True


def _prune_other_versions(cache_root, version):
    """Drop CLIs cached for other plugin versions after this one is in place."""
    parent = Path(cache_root) / 'PapyrusLint'
    keep = 'v' + version
    try:
        entries = list(parent.iterdir())
    except OSError:
        return
    for entry in entries:
        if entry.is_dir() and entry.name.startswith('v') and entry.name != keep:
            shutil.rmtree(entry, ignore_errors=True)


def ensure_release_cli(cache_root, version=None, system=None):
    """Return the cached CLI path, downloading this release when necessary."""
    version = version or release_version()
    asset = _asset_name(system)
    directory = Path(cache_root) / 'PapyrusLint' / ('v' + version)
    executable = directory / asset
    with _download_lock:
        if _is_usable(executable, asset):
            _prune_other_versions(cache_root, version)
            return str(executable)

        directory.mkdir(parents=True, exist_ok=True)
        url = f'{RELEASE_BASE}/v{version}/{asset}'
        descriptor, temporary = tempfile.mkstemp(prefix=asset + '.', dir=str(directory))
        try:
            with os.fdopen(descriptor, 'wb') as output, urlopen(url, timeout=30) as response:
                while True:
                    chunk = response.read(1024 * 1024)
                    if not chunk:
                        break
                    output.write(chunk)
            _assert_expected_sha256(temporary, asset)
            os.chmod(temporary, 0o700)
            os.replace(temporary, str(executable))
        finally:
            if os.path.exists(temporary):
                os.unlink(temporary)
        _prune_other_versions(cache_root, version)
        return str(executable)


def prefetch_release_cli(cache_root=None):
    """Best-effort download of this release's CLI; first lint/fix retries on failure."""
    with suppress(Exception):
        ensure_release_cli(cache_root if cache_root is not None else sublime.cache_path())


def plugin_loaded():
    """Download this release's CLI as soon as the package is loaded (install or update)."""
    threading.Thread(target=prefetch_release_cli, daemon=True).start()
