"""Generate schema/papyrus-lint.schema.json from lint-settings + rules."""

from __future__ import annotations

import json
from pathlib import Path

from .default_config import (
    alphabetical_rule_keys,
    config_key_for,
    load_lint_settings,
    load_rules,
)

SCHEMA_ID = "https://papyrus-lint.idrinth.de/schema/papyrus-lint.schema.json"


def _setting_schema(setting: dict) -> dict:
    schema = dict(setting.get("schema") or {})
    schema.pop("description_extra", None)
    if "description" not in schema:
        yaml_meta = setting.get("yaml") or {}
        comment = yaml_meta.get("comment") or setting.get("doc") or setting["key"]
        schema["description"] = " ".join(str(comment).split())
    return schema


def render_schema(settings: dict, rules: list[dict]) -> dict:
    properties: dict = {}

    lint_by_key = {s["key"]: s for s in settings["settings"]}
    properties["game"] = _setting_schema(lint_by_key["game"])

    for ps in settings["project_settings"]:
        properties[ps["key"]] = _setting_schema(ps)

    for setting in settings["settings"]:
        if setting["key"] == "game":
            continue
        properties[setting["key"]] = _setting_schema(setting)

    by_key = {config_key_for(rule["id"]): rule for rule in rules}
    order = alphabetical_rule_keys(rules)

    rule_props = {}
    for key in order:
        rule = by_key[key]
        enabled = rule.get("enabled_by_default", True)
        rule_props[key] = {
            # Prefer the short display name so the generated schema stays a
            # manageable size for API-based commits; rule docs live in
            # shared/rules/<id>.json and the website.
            "description": rule.get("name") or key,
            "type": "boolean",
            "default": enabled,
        }

    properties["rules"] = {
        "description": "Per-rule enable/disable switches. Omitted keys use each rule's default.",
        "type": "object",
        "additionalProperties": False,
        "properties": rule_props,
    }

    return {
        "$schema": "https://json-schema.org/draft/2020-12/schema",
        "$id": SCHEMA_ID,
        "title": "Papyrus Lint project configuration",
        "description": (
            "A papyrus-lint.yaml / papyrus-lint.yml file at a project root. Every key is "
            "optional; omitted keys fall back to the defaults documented here and in the "
            "generated shared/configuration/papyrus-lint.default.yaml artifact. The loader "
            "currently ignores unknown keys, but this schema sets additionalProperties to "
            "false so editors can flag typos."
        ),
        "type": "object",
        "additionalProperties": False,
        "properties": properties,
    }


def write_schema(repo_root: Path, destination: Path | None = None) -> Path:
    settings = load_lint_settings(repo_root)
    rules = load_rules(repo_root)
    schema = render_schema(settings, rules)
    dest = destination or (repo_root / "schema" / "papyrus-lint.schema.json")
    dest.write_text(json.dumps(schema, indent=2) + "\n", encoding="utf-8")
    return dest
