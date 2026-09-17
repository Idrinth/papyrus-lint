#!/bin/sh
set -eu

base_scripts=/base-scripts
archive=${PAPYRUS_LINT_BASE_SCRIPTS_ARCHIVE:-/base-scripts/skyrim-scripts.zip}
default_config=/usr/local/share/papyrus-lint/papyrus-lint.yaml

if [ -f "$archive" ]; then
    base_scripts=/tmp/skyrim-base-scripts
    rm -rf "$base_scripts"
    mkdir -p "$base_scripts"
    unzip -q "$archive" -d "$base_scripts"
fi

if find "$base_scripts" -type f -iname '*.psc' -print -quit | grep -q .; then
    set -- --script-root "$base_scripts" "$@"
fi

has_config_flag=0
for arg in "$@"; do
    case "$arg" in
        --config|--config=*)
            has_config_flag=1
            break
            ;;
    esac
done

if [ "$has_config_flag" -eq 0 ] \
    && [ ! -f /project/papyrus-lint.yaml ] \
    && [ ! -f /project/papyrus-lint.yml ] \
    && [ -f "$default_config" ]; then
    set -- --config "$default_config" "$@"
fi

exec PapyrusLinterCLI "$@" /project
