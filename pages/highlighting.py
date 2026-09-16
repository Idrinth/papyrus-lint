"""Lightweight, build-time syntax highlighting for the Pages builder.

Keeping highlighting in the generated HTML avoids shipping a JavaScript
parser (and a flash of unhighlighted code) on every documentation page.
"""

from __future__ import annotations

import html
import re

SYNTAX_PATTERNS = {
    "json": re.compile(
        r'(?P<string>"(?:\\.|[^"\\])*")|'
        r"(?P<number>-?\b\d+(?:\.\d+)?(?:[eE][+-]?\d+)?\b)|"
        r"(?P<keyword>\b(?:true|false|null)\b)"
    ),
    "yaml": re.compile(
        r'(?P<comment>#[^\n]*)|'
        r'(?P<string>"(?:\\.|[^"\\])*"|\'(?:\'\'|[^\'])*\')|'
        r"(?P<keyword>\b(?:true|false|null|yes|no|on|off)\b)|"
        r"(?P<number>(?<![\w.])-?\d+(?:\.\d+)?(?![\w.]))"
    ),
    "shell": re.compile(
        r'(?P<comment>#[^\n]*)|'
        r'(?P<string>"(?:\\.|[^"\\])*"|\'(?:[^\'])*\')|'
        r"(?P<keyword>\b(?:if|then|else|elif|fi|for|while|do|done|case|esac|in|function)\b)"
    ),
    "bbcode": re.compile(r"(?P<tag>\[/?[A-Za-z][^\]\n]*\])"),
    # Kept in sync with the KEYWORDS/TYPES token classes app/src/highlight.ts
    # uses for the desktop app's own code viewer, so a ```papyrus fence (the
    # docs/examples.md walkthroughs) reads the same way there and here.
    "papyrus": re.compile(
        r"(?P<comment>;/[\s\S]*?(?:/;|$)|\{[^}]*\}?|;[^\n]*)|"
        r'(?P<string>"(?:\\.|[^"\\\n])*"?)|'
        r"(?P<number>0[xX][0-9a-fA-F]+|\b\d+(?:\.\d+)?\b)|"
        r"(?P<keyword>\b(?:scriptname|extends|hidden|conditional|import|function|"
        r"endfunction|event|endevent|property|endproperty|auto|autoreadonly|global|"
        r"native|return|if|elseif|else|endif|while|endwhile|state|endstate|new|as|"
        r"true|false|none|self|parent|length|debugonly|betaonly)\b)|"
        r"(?P<type>\b(?:int|float|bool|string|var)\b)",
        re.IGNORECASE,
    ),
}

LANGUAGE_ALIASES = {
    "bash": "shell",
    "console": "shell",
    "json-schema": "json",
    "sh": "shell",
    "yml": "yaml",
}

SYNTAX_CLASSES = {
    "comment": "cm",
    "keyword": "kw",
    "number": "num",
    "string": "str",
    "tag": "tag",
    "type": "ty",
}


def highlight_code(source: str, language: str | None) -> str:
    """Escape source and add lightweight, build-time syntax markup.

    Keeping highlighting in the generated HTML avoids shipping a JavaScript
    parser (and a flash of unhighlighted code) on every documentation page.
    Unknown and deliberately plain-text fences still receive safe escaping.
    """
    normalized = LANGUAGE_ALIASES.get((language or "").lower(), (language or "").lower())
    pattern = SYNTAX_PATTERNS.get(normalized)
    if pattern is None:
        return html.escape(source)

    out: list[str] = []
    cursor = 0
    for match in pattern.finditer(source):
        out.append(html.escape(source[cursor : match.start()]))
        token_type = match.lastgroup
        css_class = SYNTAX_CLASSES[token_type] if token_type else ""
        out.append(f'<span class="{css_class}">{html.escape(match.group(0))}</span>')
        cursor = match.end()
    out.append(html.escape(source[cursor:]))
    return "".join(out)
