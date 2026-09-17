#!/bin/sh
set -eu

base_scripts=/base-scripts
archive=${PAPYRUS_LINT_BASE_SCRIPTS_ARCHIVE:-/base-scripts/skyrim-scripts.zip}

if [ -f "$archive" ]; then
    base_scripts=/tmp/skyrim-base-scripts
    rm -rf "$base_scripts"
    mkdir -p "$base_scripts"
    unzip -q "$archive" -d "$base_scripts"
fi

if find "$base_scripts" -type f -iname '*.psc' -print -quit | grep -q .; then
    set -- --script-root "$base_scripts" "$@"
fi

exec PapyrusLinterCLI "$@" /project
