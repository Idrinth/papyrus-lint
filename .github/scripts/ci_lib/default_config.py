"""Generate the documented default papyrus-lint.yaml from shared sources.

Source of truth:
  - shared/configuration/project-settings/*.json
  - shared/configuration/lint-settings/*.json
  - shared/configuration/lint-settings.yaml
    (declaration order and rules_comment)
  - shared/rules/*.json for each rule's description and enabled_by_default

Rules in the generated default YAML are ordered alphabetically by config key.
Settings UI order follows declaration order in lint-settings.yaml.

Nothing in the Rust build or the desktop UI reads this file. `cargo build`
and `npm run build` assemble the same inputs themselves. This module still
renders the document for the website, the release archive, and
`generate_default_config.py`.
"""

from __future__ import annotations

import json
from pathlib import Path

import yaml

RULE_ID_TO_CONFIG_KEY = {
    "float-to-int": "float_int_conversion",
    "too-many-named-states": "too_many_states",
}


def config_key_for(rule_id: str) -> str:
    return RULE_ID_TO_CONFIG_KEY.get(rule_id, rule_id.replace("-", "_"))


def load_lint_settings(repo_root: Path) -> dict:
    """The per-setting JSON files plus lint-settings.yaml are the source of
    truth. A tree that only has the generated YAML (unit tests) is loaded
    as-is. This does not write that YAML.
    """
    configuration = repo_root / "shared" / "configuration"
    project_dir = configuration / "project-settings"
    lint_dir = configuration / "lint-settings"
    meta_path = configuration / "lint-settings.yaml"
    if project_dir.is_dir() and lint_dir.is_dir() and meta_path.is_file():
        from ci_lib.lint_settings_yaml import assemble_lint_settings

        return assemble_lint_settings(project_dir, lint_dir, meta_path)
    generated_path = configuration / "lint-settings.generated.yaml"
    yaml_path = generated_path if generated_path.is_file() else meta_path
    data = yaml.safe_load(yaml_path.read_text(encoding="utf-8"))
    if not isinstance(data, dict):
        raise ValueError(f"{yaml_path} must contain a mapping")
    return data


def load_rules(repo_root: Path) -> list[dict]:
    rules_dir = repo_root / "shared" / "rules"
    if rules_dir.is_dir() and any(rules_dir.glob("*.json")):
        from ci_lib.rules_json import assemble_rules

        return assemble_rules(rules_dir)
    combined = repo_root / "shared" / "rules.json"
    if combined.is_file():
        loaded = json.loads(combined.read_text(encoding="utf-8"))
        if not isinstance(loaded, list):
            raise ValueError(f"{combined} must contain a JSON array")
        return loaded
    raise ValueError(f"no rule files found in {rules_dir}")


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

    for ps in settings["project"]:
        lines.append(_comment_block(yaml_comment(ps)))
        lines.append(f"{ps['key']}: {yaml_default(ps)}")

    for setting in settings["settings"]:
        if setting["key"] == "game":
            continue
        lines.append(_comment_block(yaml_comment(setting)))
        lines.append(f"{setting['key']}: {yaml_default(setting)}")

    lines.append(_comment_block(settings["rules_comment"]))
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
