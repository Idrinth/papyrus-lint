"""Generate the documented default papyrus-lint.yaml from shared sources.

Source of truth:
  - shared/configuration/lint-settings.yaml (project_settings, settings,
    rules_yaml_comment)
  - shared/rules.json (or shared/rules/*.json) for each rule's description
    and enabled_by_default

Rules in the generated default YAML are ordered alphabetically by config key.
Settings UI order follows declaration order in lint-settings.yaml.

The checked-in papyrus-lint.default.yaml is no longer a source; this module
(and papyrus-lint-config/build.rs) produce the same artifact for
build/release/docs.
"""

from __future__ import annotations

import json
from pathlib import Path

try:
    import yaml
except ImportError:  # pragma: no cover - CI jobs may not have preinstalled PyYAML
    import subprocess
    import sys

    subprocess.check_call(
        [sys.executable, "-m", "pip", "install", "--user", "-q", "PyYAML==6.0.2"]
    )
    import yaml

RULE_ID_TO_CONFIG_KEY = {
    "float-to-int": "float_int_conversion",
    "too-many-named-states": "too_many_states",
}


def config_key_for(rule_id: str) -> str:
    return RULE_ID_TO_CONFIG_KEY.get(rule_id, rule_id.replace("-", "_"))


def load_lint_settings(repo_root: Path) -> dict:
    path = repo_root / "shared" / "configuration" / "lint-settings.yaml"
    data = yaml.safe_load(path.read_text(encoding="utf-8"))
    if not isinstance(data, dict):
        raise ValueError(f"{path} must contain a mapping")
    return data


def load_rules(repo_root: Path) -> list[dict]:
    combined = repo_root / "shared" / "rules.json"
    if combined.is_file():
        return json.loads(combined.read_text(encoding="utf-8"))
    rules_dir = repo_root / "shared" / "rules"
    rules = []
    for path in sorted(rules_dir.glob("*.json")):
        rule = json.loads(path.read_text(encoding="utf-8"))
        stem = path.stem
        if rule.get("id") != stem:
            raise ValueError(f"{path}: id {rule.get('id')!r} != {stem!r}")
        rules.append(rule)
    return rules


def _comment_block(text: str) -> str:
    return "\n".join(f"# {line}" if line else "#" for line in text.split("\n"))


def yaml_default(setting: dict) -> str:
    value = setting["yaml"]["default"]
    if not isinstance(value, str):
        raise TypeError(
            f"lint setting {setting.get('key')!r} yaml.default must be a string snippet, "
            f"got {type(value).__name__}"
        )
    return value


def yaml_comment(setting: dict) -> str:
    return setting["yaml"]["comment"]


def alphabetical_rule_keys(rules: list[dict]) -> list[str]:
    by_key: dict[str, dict] = {}
    for rule in rules:
        key = config_key_for(rule["id"])
        if key in by_key:
            raise ValueError(f"duplicate rule config key: {key}")
        by_key[key] = rule
    return sorted(by_key)


def render_default_yaml(settings: dict, rules: list[dict]) -> str:
    by_key = {config_key_for(rule["id"]): rule for rule in rules}
    order = alphabetical_rule_keys(rules)

    lines: list[str] = []

    lint_by_key = {s["key"]: s for s in settings["settings"]}
    game = lint_by_key["game"]
    lines.append(_comment_block(yaml_comment(game)))
    lines.append(f"game: {yaml_default(game)}")

    for ps in settings["project_settings"]:
        lines.append(_comment_block(yaml_comment(ps)))
        lines.append(f"{ps['key']}: {yaml_default(ps)}")

    for setting in settings["settings"]:
        if setting["key"] == "game":
            continue
        lines.append(_comment_block(yaml_comment(setting)))
        lines.append(f"{setting['key']}: {yaml_default(setting)}")

    lines.append(_comment_block(settings["rules_yaml_comment"]))
    lines.append("rules:")
    for key in order:
        rule = by_key[key]
        enabled = rule.get("enabled_by_default", True)
        lines.append(f"  {key}: {'true' if enabled else 'false'} # {rule['description']}")
    lines.append("")
    return "\n".join(lines)


def write_default_yaml(repo_root: Path, destination: Path | None = None) -> Path:
    settings = load_lint_settings(repo_root)
    rules = load_rules(repo_root)
    text = render_default_yaml(settings, rules)
    dest = destination or (
        repo_root / "shared" / "configuration" / "papyrus-lint.default.yaml"
    )
    dest.parent.mkdir(parents=True, exist_ok=True)
    dest.write_text(text, encoding="utf-8")
    return dest
