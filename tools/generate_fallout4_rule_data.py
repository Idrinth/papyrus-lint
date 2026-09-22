#!/usr/bin/env python3
"""Generate shared/rules/data/fallout4/*.yaml from the bundled script zips.

Re-run from the repository root after updating
`shared/scripts/fallout4-scripts.zip` or
`shared/scripts/fallout4-extender-scripts.zip`:

    python3 tools/generate_fallout4_rule_data.py
"""

from __future__ import annotations

import re
import sys
import zipfile
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
OUT_DIR = ROOT / "shared/rules/data/fallout4"
FO4_ZIP = ROOT / "shared/scripts/fallout4-scripts.zip"
F4SE_ZIP = ROOT / "shared/scripts/fallout4-extender-scripts.zip"

EVENT_PRIORITY = [
    "ScriptObject",
    "Form",
    "ObjectReference",
    "Actor",
    "ActiveMagicEffect",
    "Alias",
    "ReferenceAlias",
    "LocationAlias",
    "RefCollectionAlias",
    "Quest",
    "Scene",
    "TopicInfo",
    "Terminal",
    "Perk",
    "MagicEffect",
]
EVENT_PRIO = {name: index for index, name in enumerate(EVENT_PRIORITY)}

TYPE_CASE = {
    "bool": "Bool",
    "int": "Int",
    "float": "Float",
    "string": "String",
    "String": "String",
}


def read_zip_psc(archive: Path) -> dict[str, str]:
    files: dict[str, str] = {}
    with zipfile.ZipFile(archive) as zf:
        for info in zf.infolist():
            if not info.filename.lower().endswith(".psc"):
                continue
            raw = zf.read(info).decode("utf-8", errors="replace")
            raw = raw.replace("\r\n", "\n").replace("\r", "\n")
            raw = re.sub(r"\\\n", " ", raw)
            files[info.filename] = raw
    return files


def script_header(text: str) -> tuple[str, str] | None:
    match = re.search(r"(?im)^Scriptname[ \t]+(\S+)(.*)$", text)
    if not match:
        return None
    return match.group(1), match.group(2)


def is_native_hidden(flags: str) -> bool:
    return bool(re.search(r"\bNative\b", flags, re.I))


def native_base_scripts(files: dict[str, str]) -> list[tuple[str, str]]:
    scripts: list[tuple[str, str]] = []
    for path, text in files.items():
        if "/DLC" in path.replace("\\", "/"):
            continue
        header = script_header(text)
        if not header:
            continue
        name, flags = header
        if is_native_hidden(flags):
            scripts.append((name, text))
    scripts.sort(key=lambda item: item[0].lower())
    return scripts


def extract_native_methods(scripts: list[tuple[str, str]]) -> list[tuple[str, str]]:
    methods: list[tuple[str, str]] = []
    seen: set[tuple[str, str]] = set()
    for name, text in scripts:
        for line in text.splitlines():
            if re.match(r"^\s*;", line) or not re.search(r"\bnative\b", line, re.I):
                continue
            match = re.search(r"(?i)\bFunction[ \t]+(\w+)[ \t]*\(", line)
            if not match:
                continue
            function = match.group(1)
            key = (name.lower(), function.lower())
            if key in seen:
                continue
            seen.add(key)
            methods.append((name, function))
    methods.sort(key=lambda item: (item[0].lower(), item[1].lower()))
    return methods


def parse_event_args(argstr: str) -> list[tuple[str, str]]:
    argstr = argstr.strip()
    if not argstr:
        return []
    args: list[tuple[str, str]] = []
    for part in [piece.strip() for piece in argstr.split(",") if piece.strip()]:
        part = re.sub(r"\s+", " ", part).strip()
        match = re.match(r"^([\w:.]+(?:\s*\[\s*\])?)\s+(\w+)(?:\s*=.*)?$", part)
        if match:
            typ, nam = match.group(1), match.group(2)
        else:
            bits = part.split()
            if len(bits) < 2:
                continue
            typ, nam = bits[0], bits[1].split("=")[0]
        typ = TYPE_CASE.get(typ, re.sub(r"\s+", "", typ))
        args.append((typ, nam))
    return args


def extract_known_events(scripts: list[tuple[str, str]]) -> list[tuple[str, str, list[tuple[str, str]]]]:
    events: dict[str, tuple[int, str, str, list[tuple[str, str]]]] = {}
    event_re = re.compile(r"(?im)^[ \t]*Event[ \t]+(\w+)[ \t]*\((.*?)\)")
    for name, text in scripts:
        for match in event_re.finditer(text):
            event = match.group(1)
            args = parse_event_args(match.group(2))
            priority = EVENT_PRIO.get(name, 100)
            key = event.lower()
            if key not in events or priority < events[key][0]:
                events[key] = (priority, name, event, args)
    ordered = list(events.values())
    ordered.sort(key=lambda item: (item[0], item[1].lower(), item[2].lower()))
    return [(form, event, args) for _priority, form, event, args in ordered]


def write(path: Path, content: str) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(content, encoding="utf-8")
    print(f"wrote {path.relative_to(ROOT)} ({len(content)} bytes)")


def render_native_methods(methods: list[tuple[str, str]]) -> str:
    out = """# Native functions declared by Fallout 4's base-game Papyrus scripts.
#
# Compiled in by papyrus-lints/build.rs for the \"Non-base-game native
# function usage\" lint (see native_function_usage.rs), which flags a
# `Native` function/event declared on a linted script whose (object,
# function) pair isn't listed here — a strong signal it's instead supplied
# by SKSE/F4SE or some other native extension the project depends on.
# Functions supplied by F4SE or other extensions are intentionally excluded
# from this list itself.
#
# Sourced from `shared/scripts/fallout4-scripts.zip` (Creation Kit base
# scripts whose `Scriptname` line is `Native Hidden`).
#
# Each entry:
#   object:   the script or object type that declares the native function
#   function: the native function's Papyrus name
"""
    for obj, fn in methods:
        out += f"\n- object: {obj}\n  function: {fn}\n"
    return out


def render_known_events(events: list[tuple[str, str, list[tuple[str, str]]]]) -> str:
    out = """# Native events Bethesda's engine invokes directly with a fixed argument
# list, taken from Fallout 4's base-game native Hidden scripts
# (`shared/scripts/fallout4-scripts.zip`). Compiled in by
# papyrus-lints/build.rs for the \"Event signature mismatch\" lint (see
# ../app/crates/papyrus-lints/src/event_signature.rs), which flags an
# `Event` declaration whose name matches one of these but whose parameter
# list doesn't match the signature listed here. Papyrus itself never
# validates an `Event`'s signature against what the engine actually calls
# it with, so a mismatched declaration still compiles fine — the engine
# just never invokes it (or invokes it with arguments the script doesn't
# expect), silently dropping that event instead of raising any kind of
# error.
#
# Each entry:
#   event: the Event's own name, exactly as the engine calls it (matched
#          case-insensitively against a declared `Event`'s name)
#   form:  the script/Form that first declares this event; a script whose
#          `Extends` chain reaches this Form (directly or transitively) may
#          declare/override it
#   args:  the event's exact parameter list, in order; each entry is the
#          parameter's Papyrus type and its own conventional name (matched
#          by type only, case-insensitively — parameter names are free to
#          differ)
"""
    for form, event, args in events:
        if not args:
            rendered = "[]"
        else:
            lines = [""]
            for typ, nam in args:
                lines.append(f"    - type: {typ}")
                lines.append(f"      name: {nam}")
            rendered = "\n".join(lines)
        out += f"\n- event: {event}\n  form: {form}\n  args: {rendered}\n"
    return out


def f4se_singleton_scripts(files: dict[str, str]) -> list[str]:
    names: list[str] = []
    for text in files.values():
        header = script_header(text)
        if not header:
            continue
        name, flags = header
        if is_native_hidden(flags) and "extends" not in flags.lower():
            names.append(name)
    return sorted(set(names), key=str.lower)


def main() -> int:
    if not FO4_ZIP.is_file():
        print(f"missing {FO4_ZIP}", file=sys.stderr)
        return 1
    fo4_files = read_zip_psc(FO4_ZIP)
    scripts = native_base_scripts(fo4_files)
    methods = extract_native_methods(scripts)
    events = extract_known_events(scripts)
    write(OUT_DIR / "native-methods.yaml", render_native_methods(methods))
    write(OUT_DIR / "known-events.yaml", render_known_events(events))
    print(f"native methods: {len(methods)}")
    print(f"known events: {len(events)}")
    if F4SE_ZIP.is_file():
        singletons = f4se_singleton_scripts(read_zip_psc(F4SE_ZIP))
        print("F4SE native singletons:", ", ".join(singletons))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
