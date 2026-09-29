"""Assembles shared/configuration/lint-settings.yaml from per-setting JSON.

Each project setting lives in shared/configuration/project-settings/<key>.json
and each lint setting in shared/configuration/lint-settings/<key>.json.
Declaration order (the Settings tab order) and the rules-section comment
live in shared/configuration/lint-settings.meta.json.

The combined YAML is git-ignored. Rust build scripts,
app/scripts/generate-config-types.mjs, and the default-config/schema
generators still read that one file, so none of them parse the JSON
directories themselves. Regenerate it with
.github/scripts/build_lint_settings_yaml.py, or by running
generate_default_config.py / generate_config_schema.py, which refresh it
before they read.
"""

from __future__ import annotations

import json
import re
from pathlib import Path

HEADER = """\
# Generated from `shared/configuration/project-settings/*.json`,
# `shared/configuration/lint-settings/*.json`, and
# `shared/configuration/lint-settings.meta.json` by
# `.github/scripts/build_lint_settings_yaml.py`. Do not edit by hand.
"""

_DOUBLE_QUOTED_KEYS = frozenset({"comment", "description", "doc"})
_PLAIN_KEY = re.compile(r"[A-Za-z_][A-Za-z0-9_]*\Z")
_YAML11_BOOL = frozenset(
    {
        "y",
        "Y",
        "yes",
        "Yes",
        "YES",
        "n",
        "N",
        "no",
        "No",
        "NO",
        "true",
        "True",
        "TRUE",
        "false",
        "False",
        "FALSE",
        "on",
        "On",
        "ON",
        "off",
        "Off",
        "OFF",
    }
)
_YAML11_NULL = frozenset({"null", "Null", "NULL", "~"})
_PLAIN_NUMBER = re.compile(
    r"[-+]?(?:0|[1-9][0-9]*)(?:\.[0-9]*)?(?:[eE][-+]?[0-9]+)?\Z"
    r"|[-+]?\.[0-9]+(?:[eE][-+]?[0-9]+)?\Z"
    r"|0x[0-9A-Fa-f]+\Z"
)
_PLAIN_START = set("-?:,[]{}#&*!|>'\"%@`")


def assemble_lint_settings(
    project_settings_dir: Path,
    lint_settings_dir: Path,
    meta_path: Path,
) -> dict:
    """Reads the meta file's declaration order and returns the combined
    document: project_settings, rules_yaml_comment, then settings.

    Each listed key must have a `<key>.json` whose `key` field matches the
    file name. A JSON file that the meta list does not name is an error,
    same as a listed key with no file.
    """
    meta = _load_meta(meta_path)
    project_settings = _load_listed(
        project_settings_dir, meta["project_settings"], meta_path, "project_settings"
    )
    settings = _load_listed(lint_settings_dir, meta["settings"], meta_path, "settings")
    _reject_shared_keys(project_settings, settings)
    return {
        "project_settings": project_settings,
        "rules_yaml_comment": meta["rules_yaml_comment"],
        "settings": settings,
    }


def render_lint_settings_yaml(document: dict, *, header: bool = True) -> str:
    """Renders *document* in the checked-in lint-settings.yaml style.

    Prose fields (`comment`, `description`, `doc`) stay double-quoted with
    escaped newlines. Strings YAML 1.1 would otherwise read as null, bool,
    or a number stay single-quoted. Everything else that is a plain scalar
    stays unquoted, which is what the hand-written file did.
    """
    lines: list[str] = ["project_settings:"]
    _emit_setting_list(lines, document["project_settings"])
    lines.append(
        f"rules_yaml_comment: {_format_scalar(document['rules_yaml_comment'], 'rules_yaml_comment')}"
    )
    lines.append("settings:")
    _emit_setting_list(lines, document["settings"])
    body = "\n".join(lines) + "\n"
    if not header:
        return body
    return HEADER + "\n" + body


def _load_meta(meta_path: Path) -> dict:
    if not meta_path.is_file():
        raise ValueError(f"{meta_path} does not exist")
    meta = json.loads(meta_path.read_text(encoding="utf-8"))
    if not isinstance(meta, dict):
        raise ValueError(f"{meta_path} must contain a JSON object")
    comment = meta.get("rules_yaml_comment")
    if not isinstance(comment, str):
        raise ValueError(f"{meta_path}: `rules_yaml_comment` must be a string")
    for field in ("project_settings", "settings"):
        order = meta.get(field)
        if not isinstance(order, list) or not order:
            raise ValueError(f"{meta_path}: `{field}` must be a non-empty list of setting keys")
        if any(not isinstance(key, str) or not key for key in order):
            raise ValueError(f"{meta_path}: `{field}` entries must be non-empty strings")
        if len(order) != len(set(order)):
            raise ValueError(f"{meta_path}: `{field}` lists a key more than once")
    return meta


def _load_listed(directory: Path, keys: list[str], meta_path: Path, field: str) -> list[dict]:
    if not directory.is_dir():
        raise ValueError(f"no setting files found in {directory}")
    found = {path.stem: path for path in sorted(directory.glob("*.json")) if path.is_file()}
    if not found:
        raise ValueError(f"no setting files found in {directory}")
    listed = set(keys)
    extras = sorted(set(found) - listed)
    if extras:
        joined = ", ".join(f"{name}.json" for name in extras)
        raise ValueError(
            f"{directory} has setting files not listed in {meta_path} `{field}`: {joined}"
        )
    settings = []
    for key in keys:
        path = found.get(key)
        if path is None:
            raise ValueError(f"{meta_path}: `{field}` lists {key!r} but {directory}/{key}.json does not exist")
        setting = json.loads(path.read_text(encoding="utf-8"))
        if not isinstance(setting, dict):
            raise ValueError(f"{path}: expected a JSON object")
        if setting.get("key") != key:
            raise ValueError(
                f"{path}: `key` is {setting.get('key')!r}, expected {key!r} to match the file name"
            )
        settings.append(setting)
    return settings


def _reject_shared_keys(project_settings: list[dict], settings: list[dict]) -> None:
    overlap = sorted({item["key"] for item in project_settings} & {item["key"] for item in settings})
    if overlap:
        raise ValueError(
            f"setting {overlap[0]!r} is listed under both project_settings and settings"
        )


def _emit_setting_list(lines: list[str], settings: list[dict]) -> None:
    for setting in settings:
        _emit_mapping_items(lines, 2, setting)


def _emit_mapping_items(lines: list[str], dash_indent: int, mapping: dict) -> None:
    pairs = list(mapping.items())
    if not pairs:
        lines.append(f"{' ' * dash_indent}- {{}}")
        return
    key, value = pairs[0]
    _emit_key(lines, dash_indent, key, value, as_list_item=True)
    for key, value in pairs[1:]:
        _emit_key(lines, dash_indent + 2, key, value, as_list_item=False)


def _emit_key(lines: list[str], indent: int, key: str, value: object, *, as_list_item: bool) -> None:
    pad = " " * indent
    head = f"{pad}- " if as_list_item else pad
    rendered_key = _format_key(key)
    if _is_inline(value):
        lines.append(f"{head}{rendered_key}: {_format_scalar(value, key)}")
        return
    lines.append(f"{head}{rendered_key}:")
    _emit_complex(lines, indent + (4 if as_list_item else 2), value)


def _emit_complex(lines: list[str], indent: int, value: object) -> None:
    if isinstance(value, dict):
        for key, item in value.items():
            _emit_key(lines, indent, str(key), item, as_list_item=False)
        return
    if isinstance(value, list):
        for item in value:
            if isinstance(item, dict):
                _emit_mapping_items(lines, indent, item)
            elif _is_inline(item):
                lines.append(f"{' ' * indent}- {_format_scalar(item, None)}")
            else:
                raise TypeError(f"cannot render a {type(item).__name__} list item in lint-settings YAML")
        return
    raise TypeError(f"cannot render a {type(value).__name__} in lint-settings YAML")


def _is_inline(value: object) -> bool:
    if isinstance(value, (str, bool, int, float)) or value is None:
        return True
    if isinstance(value, list):
        return len(value) == 0
    if isinstance(value, dict):
        return len(value) == 0
    return False


def _format_key(key: str) -> str:
    if _PLAIN_KEY.fullmatch(key):
        return key
    return json.dumps(key)


def _format_scalar(value: object, key: str | None) -> str:
    if value is None:
        return "null"
    if isinstance(value, bool):
        return "true" if value else "false"
    if isinstance(value, int):
        return str(value)
    if isinstance(value, float):
        return format(value, "g")
    if isinstance(value, str):
        return _format_string(value, key)
    if isinstance(value, list) and not value:
        return "[]"
    if isinstance(value, dict) and not value:
        return "{}"
    raise TypeError(f"cannot render {type(value).__name__} as a lint-settings scalar")


def _format_string(value: str, key: str | None) -> str:
    if key in _DOUBLE_QUOTED_KEYS or "\n" in value or "\\" in value:
        return json.dumps(value)
    if _plain_string_ok(value):
        return value
    if "'" not in value:
        return f"'{value}'"
    return json.dumps(value)


def _plain_string_ok(value: str) -> bool:
    if not value or value != value.strip():
        return False
    if value in _YAML11_BOOL or value in _YAML11_NULL:
        return False
    if _PLAIN_NUMBER.fullmatch(value):
        return False
    if value in {"[]", "{}", "?", "-", ":"}:
        return False
    if value[0] in _PLAIN_START:
        return False
    return ": " not in value and "#" not in value
