"""Generate the documented default papyrus-lint.yaml from shared sources.

Source of truth:
  - shared/configuration/lint-settings.json (project_settings, settings,
    rules_yaml_comment, rule_order)
  - shared/rules.json (or shared/rules/*.json) for each rule's
    enabled_by_default

The checked-in papyrus-lint.default.yaml is no longer a source; this module
(and papyrus-lint-config/build.rs) produce the same artifact for
build/release/docs.
"""

from __future__ import annotations

import json
from pathlib import Path

RULE_ID_TO_CONFIG_KEY = {
    "float-to-int": "float_int_conversion",
    "too-many-named-states": "too_many_states",
}


def config_key_for(rule_id: str) -> str:
    return RULE_ID_TO_CONFIG_KEY.get(rule_id, rule_id.replace("-", "_"))


def load_lint_settings(repo_root: Path) -> dict:
    path = repo_root / "shared" / "configuration" / "lint-settings.json"
    return json.loads(path.read_text(encoding="utf-8"))


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


def render_default_yaml(settings: dict, rules: list[dict]) -> str:
    by_key = {config_key_for(rule["id"]): rule for rule in rules}
    order = settings["rule_order"]
    missing = [key for key in order if key not in by_key]
    if missing:
        raise ValueError(f"rule_order lists unknown keys: {missing}")
    extra = sorted(set(by_key) - set(order))
    if extra:
        raise ValueError(f"rule_order is missing keys: {extra}")

    lines: list[str] = []

    lint_by_key = {s["key"]: s for s in settings["settings"]}
    game = lint_by_key["game"]
    lines.append(_comment_block(game["yaml_comment"]))
    lines.append(f"game: {game['yaml_default']}")

    for ps in settings["project_settings"]:
        lines.append(_comment_block(ps["yaml_comment"]))
        lines.append(f"{ps['key']}: {ps['yaml_default']}")

    for setting in settings["settings"]:
        if setting["key"] == "game":
            continue
        lines.append(_comment_block(setting["yaml_comment"]))
        lines.append(f"{setting['key']}: {setting['yaml_default']}")

    lines.append(_comment_block(settings["rules_yaml_comment"]))
    lines.append("rules:")
    for key in order:
        enabled = by_key[key].get("enabled_by_default", True)
        lines.append(f"  {key}: {'true' if enabled else 'false'}")
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
