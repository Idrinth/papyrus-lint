"""Download and cache the PapyrusLinterCLI matching this plugin release."""

import os
from pathlib import Path
import platform
import tempfile
from urllib.request import urlopen

import json
import sublime


RELEASE_BASE = 'https://github.com/Idrinth/papyrus-lint/releases/download'


def _asset_name(system=None):
    system = system or platform.system()
    assets = {
        'Windows': 'PapyrusLinterCLI-windows.exe',
        'Darwin': 'PapyrusLinterCLI-macos',
        'Linux': 'PapyrusLinterCLI-linux',
    }
    if system not in assets:
        raise OSError('Papyrus Lint does not publish a CLI for {}'.format(system))
    return assets[system]


def release_version():
    try:
        metadata = json.loads(
            sublime.load_resource(
                'Packages/SublimeLinter-contrib-papyrus-lint/package-metadata.json'
            )
        )
        return metadata['version'].removeprefix('v')
    except (FileNotFoundError, KeyError, OSError, ValueError):
        # Release archives carry VERSION; Package Control installs additionally
        # expose their tag-derived version through package-metadata.json.
        pass
    return (Path(__file__).with_name('VERSION')).read_text(encoding='utf-8').strip()


def ensure_release_cli(cache_root, version=None, system=None):
    """Return the cached CLI path, downloading this release when necessary."""
    version = version or release_version()
    asset = _asset_name(system)
    directory = Path(cache_root) / 'PapyrusLint' / ('v' + version)
    executable = directory / asset
    if executable.is_file() and (os.name == 'nt' or os.access(str(executable), os.X_OK)):
        return str(executable)

    directory.mkdir(parents=True, exist_ok=True)
    url = '{}/v{}/{}'.format(RELEASE_BASE, version, asset)
    descriptor, temporary = tempfile.mkstemp(prefix=asset + '.', dir=str(directory))
    try:
        with os.fdopen(descriptor, 'wb') as output, urlopen(url, timeout=30) as response:
            while True:
                chunk = response.read(1024 * 1024)
                if not chunk:
                    break
                output.write(chunk)
        os.chmod(temporary, 0o700)
        os.replace(temporary, str(executable))
    finally:
        if os.path.exists(temporary):
            os.unlink(temporary)
    return str(executable)
