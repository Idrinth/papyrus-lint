"""Generates the desktop app's `app/src/config-types.ts`.

Mirrors `papyrus-lints/build.rs` writing `Rules` / `default_rules()` from
`shared/rules.json` plus `configuration/papyrus-lint.default.yaml`'s
`rules:` field order. The TypeScript file also carries the hand-shaped
top-level `LintConfig` fields (the analogue of `config.rs`'s `Config`),
whose defaults are taken from that same YAML so they cannot drift from
the Rust `Config::default()`.
"""

from __future__ import annotations

from collections.abc import Mapping, Sequence
from pathlib import Path

# `shared/rules/<id>.json` ids that don't become their `LintRules` /
# `Config.rules` field name by replacing `-` with `_`. Keep in sync with
# `RULE_ID_TO_CONFIG_KEY` in papyrus-lints/build_support/metadata.rs and
# papyrus-lint-config/build.rs.
RULE_ID_TO_CONFIG_KEY: dict[str, str] = {
    "float-to-int": "float_int_conversion",
    "too-many-named-states": "too_many_states",
}

# Top-level keys that belong on the frontend `LintConfig` (and on
# `papyrus_lints::Config`). Everything else in the default YAML is
# project-file metadata owned by papyrus-lint-config, not this type.
LINT_CONFIG_KEYS: tuple[str, ...] = (
    "semicolon",
    "indentation",
    "indentation_width",
    "identifier_casing",
    "cyclomatic_complexity_warning",
    "cyclomatic_complexity_error",
    "type_casing",
    "named_arguments",
    "min_wait_interval",
    "magic_numbers",
    "fail_on_warning",
    "fail_on_info",
    "bool_like_int",
    "assume_auto_properties_filled",
)

STRING_LINT_CONFIG_KEYS = frozenset(
    {
        "indentation",
        "identifier_casing",
        "type_casing",
        "named_arguments",
        "magic_numbers",
    }
)

LINT_CONFIG_FIELD_TYPES: dict[str, str] = {
    "semicolon": "boolean",
    "indentation": '"tab" | "space"',
    "indentation_width": "number",
    "identifier_casing": "IdentifierCasingStyle",
    "cyclomatic_complexity_warning": "number",
    "cyclomatic_complexity_error": "number",
    "type_casing": "TypeCasingStyle",
    "named_arguments": "NamedArgumentsStyle",
    "min_wait_interval": "number",
    "magic_numbers": "MagicNumbersMode",
    "fail_on_warning": "boolean",
    "fail_on_info": "boolean",
    "bool_like_int": "boolean",
    "assume_auto_properties_filled": "boolean",
}

HEADER = """\
// Generated from `shared/rules/*.json` and
// `configuration/papyrus-lint.default.yaml` by
// `.github/scripts/generate_config_types.py`. Do not edit by hand.

export type TypeCasingStyle = "PascalCase" | "camelCase" | "lowercase" | "UPPERCASE";
export type IdentifierCasingStyle = "camelCase" | "PascalCase" | "snake_case" | "CONSTANT_CASE";
export type NamedArgumentsStyle = "always" | "instead_of_defaults" | "never";
export type MagicNumbersMode = "loose" | "strict";
"""


def config_key_for(rule_id: str) -> str:
    return RULE_ID_TO_CONFIG_KEY.get(rule_id, rule_id.replace("-", "_"))


def parse_default_yaml(source: str) -> tuple[dict[str, str], list[tuple[str, bool]]]:
    """Pulls top-level `key: value` pairs and the `rules:` block out of the
    annotated default YAML without a YAML library. Matches the line-oriented
    parser in `papyrus-lints/build_support/mod.rs`'s `default_config_order`.
    """
    top: dict[str, str] = {}
    rules: list[tuple[str, bool]] = []
    in_rules = False
    for line in source.splitlines():
        stripped = line.strip()
        if not stripped or stripped.startswith("#"):
            continue
        if not in_rules:
            if stripped == "rules:":
                in_rules = True
                continue
            key, sep, value = stripped.partition(":")
            if not sep:
                raise ValueError(f"default YAML line is not key: value: {line!r}")
            top[key.strip()] = value.strip()
            continue
        if not line.startswith("  ") or line.startswith("   "):
            break
        key, sep, value = stripped.partition(":")
        if not sep:
            raise ValueError(f"default YAML rules line is not key: value: {line!r}")
        rules.append((key.strip(), value.strip() == "true"))
    if not rules:
        raise ValueError("default YAML has no `rules:` entries")
    return top, rules


def order_rules(
    rules: Sequence[Mapping[str, object]],
    field_order: Sequence[str],
) -> list[Mapping[str, object]]:
    by_key: dict[str, Mapping[str, object]] = {}
    for rule in rules:
        rule_id = str(rule["id"])
        key = config_key_for(rule_id)
        if key in by_key:
            raise ValueError(f"duplicate Rules field `{key}`")
        by_key[key] = rule
    ordered: list[Mapping[str, object]] = []
    for key in field_order:
        try:
            ordered.append(by_key.pop(key))
        except KeyError as error:
            raise ValueError(
                f"configuration/papyrus-lint.default.yaml lists rules.{key} "
                "but shared/rules has no matching id"
            ) from error
    if by_key:
        missing = ", ".join(sorted(by_key))
        raise ValueError(
            "configuration/papyrus-lint.default.yaml is missing rules: "
            f"{missing}; add them next to the other `rules:` keys"
        )
    return ordered


def _ts_default(key: str, raw: str) -> str:
    if key in STRING_LINT_CONFIG_KEYS:
        return f'"{raw}"'
    return raw


def render_config_types(
    rules: Sequence[Mapping[str, object]],
    default_yaml: str,
) -> str:
    top, yaml_rules = parse_default_yaml(default_yaml)
    missing_top = [key for key in LINT_CONFIG_KEYS if key not in top]
    if missing_top:
        raise ValueError(f"default YAML is missing LintConfig keys: {missing_top}")

    ordered = order_rules(rules, [key for key, _ in yaml_rules])
    yaml_values = dict(yaml_rules)
    lines = [HEADER, "export interface LintRules {"]
    defaults: list[str] = []
    for rule in ordered:
        key = config_key_for(str(rule["id"]))
        enabled = bool(rule.get("enabled_by_default", True))
        if yaml_values[key] != enabled:
            raise ValueError(
                f"rules.{key} is {yaml_values[key]} in the default YAML but "
                f"enabled_by_default is {enabled} in shared/rules"
            )
        lines.append(f"  {key}: boolean;")
        defaults.append(f"  {key}: {'true' if enabled else 'false'},")
    lines.append("}")
    lines.append("")
    lines.append("export interface LintConfig {")
    for key in LINT_CONFIG_KEYS:
        lines.append(f"  {key}: {LINT_CONFIG_FIELD_TYPES[key]};")
    lines.append("  rules: LintRules;")
    lines.append("}")
    lines.append("")
    lines.append("export const DEFAULT_RULES: LintRules = {")
    lines.extend(defaults)
    lines.append("};")
    lines.append("")
    lines.append("export const DEFAULT_LINT_CONFIG: LintConfig = {")
    for key in LINT_CONFIG_KEYS:
        lines.append(f"  {key}: {_ts_default(key, top[key])},")
    lines.append("  rules: DEFAULT_RULES,")
    lines.append("};")
    lines.append("")
    lines.append("export const RULE_KEYS = Object.keys(DEFAULT_RULES) as (keyof LintRules)[];")
    lines.append("")
    lines.append("export let currentLintConfig: LintConfig = DEFAULT_LINT_CONFIG;")
    lines.append("")
    lines.append("export function setCurrentLintConfig(config: LintConfig) {")
    lines.append("  currentLintConfig = config;")
    lines.append("}")
    lines.append("")
    return "\n".join(lines)


def write_config_types(
    rules: Sequence[Mapping[str, object]],
    default_yaml: str,
    destination: Path,
) -> str:
    rendered = render_config_types(rules, default_yaml)
    destination.parent.mkdir(parents=True, exist_ok=True)
    destination.write_text(rendered, encoding="utf-8")
    return rendered
