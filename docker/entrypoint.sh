#!/bin/sh
set -eu

user_archive=${PAPYRUS_LINT_BASE_SCRIPTS_ARCHIVE:-/base-scripts/skyrim-scripts.zip}
default_config=/usr/local/share/papyrus-lint/papyrus-lint.yaml

# script_locator only reads immediate children of a search root, so point
# --script-root at the directory that actually contains the .psc files.
psc_dir_in() {
    found=$(find "$1" -maxdepth 1 -type f -iname '*.psc' -print -quit 2>/dev/null || true)
    if [ -n "$found" ]; then
        printf '%s\n' "$1"
        return 0
    fi
    found=$(find "$1" -type f -iname '*.psc' -print -quit 2>/dev/null || true)
    if [ -n "$found" ]; then
        dirname "$found"
        return 0
    fi
    return 1
}

base_scripts=
if dir=$(psc_dir_in /base-scripts); then
    base_scripts=$dir
else
    archive=
    if [ -f "$user_archive" ]; then
        archive=$user_archive
        unpacked=/tmp/skyrim-base-scripts
        rm -rf "$unpacked"
        mkdir -p "$unpacked"
        unzip -q "$archive" -d "$unpacked"
        if dir=$(psc_dir_in "$unpacked"); then
            base_scripts=$dir
        fi
    fi
fi

if [ -n "$base_scripts" ]; then
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

exec PapyrusLinterCLI lint "$@" /project
