// A small, self-contained Papyrus syntax highlighter for the code viewer.
//
// It mirrors the token grammar of crates/papyrus-parser/src/lexer.rs closely
// enough for display purposes (comments, strings, numbers, keywords) without
// needing to go through Tauri to re-tokenize on the Rust side.

// Keeps in sync with `Keyword::from_word` in crates/papyrus-parser/src/token.rs.
// Exported so autocomplete.ts's declaration scanner can avoid mistaking a
// keyword for a type/identifier pair (e.g. "If bReady" or "Return akRef").
export const KEYWORDS = new Set([
  "scriptname",
  "extends",
  "hidden",
  "conditional",
  "import",
  "function",
  "endfunction",
  "event",
  "endevent",
  "property",
  "endproperty",
  "auto",
  "autoreadonly",
  "global",
  "native",
  "return",
  "if",
  "elseif",
  "else",
  "endif",
  "while",
  "endwhile",
  "state",
  "endstate",
  "new",
  "as",
  "true",
  "false",
  "none",
  "self",
  "parent",
  "length",
  "debugonly",
  "betaonly",
]);

// Papyrus's built-in primitive types are ordinary identifiers grammatically,
// but it reads better to give them their own color.
const TYPES = new Set(["int", "float", "bool", "string", "var"]);

type TokenClass = "comment" | "string" | "number" | "keyword" | "type" | null;

interface HighlightToken {
  text: string;
  cls: TokenClass;
}

// Tried in priority order at each position: block comments (`;/ ... /;`),
// brace comments (`{ ... }`), line comments (`; ...`), string literals,
// hex/decimal numbers, and words. Anything else (operators, punctuation,
// whitespace) falls through unclassified. Kept as separate sticky (`y`)
// patterns, rather than one combined alternation, so each stays simple
// enough for static analysis to reason about.
const TOKEN_PATTERNS = [
  /;\/[\s\S]*?(?:\/;|$)/y,
  /\{[^}]*\}?/y,
  /;[^\n]*/y,
  /"(?:\\.|[^"\\\n])*"?/y,
  /0[xX][0-9a-fA-F]+/y,
  /\d+(?:\.\d+)?/y,
  /[A-Za-z_]\w*/y,
];

function matchTokenAt(source: string, index: number): string | null {
  for (const pattern of TOKEN_PATTERNS) {
    pattern.lastIndex = index;
    const match = pattern.exec(source);
    if (match) {
      return match[0];
    }
  }
  return null;
}

function classify(match: string): TokenClass {
  const first = match[0];
  if (first === ";" || first === "{") {
    return "comment";
  }
  if (first === '"') {
    return "string";
  }
  if (first >= "0" && first <= "9") {
    return "number";
  }
  const lower = match.toLowerCase();
  if (KEYWORDS.has(lower)) {
    return "keyword";
  }
  if (TYPES.has(lower)) {
    return "type";
  }
  return null;
}

// Tokenizes `source` into a flat run of classified/unclassified fragments,
// in order, covering every character (including whitespace and newlines).
function tokenize(source: string): HighlightToken[] {
  const tokens: HighlightToken[] = [];
  let lastIndex = 0;
  let index = 0;
  while (index < source.length) {
    const match = matchTokenAt(source, index);
    if (!match) {
      index++;
      continue;
    }
    if (index > lastIndex) {
      tokens.push({ text: source.slice(lastIndex, index), cls: null });
    }
    tokens.push({ text: match, cls: classify(match) });
    index += match.length;
    lastIndex = index;
  }
  if (lastIndex < source.length) {
    tokens.push({ text: source.slice(lastIndex), cls: null });
  }
  return tokens;
}

function escapeHtml(text: string): string {
  return text.replace(/&/g, "&amp;").replace(/</g, "&lt;").replace(/>/g, "&gt;");
}

// Renders `source` as an array of highlighted-HTML strings, one per line
// (split on "\n", the only newline the lexer recognizes), so a caller can
// pair each entry with a line number in a gutter. A span's class is
// re-opened on every line it spans, so multi-line block/brace comments stay
// colored on their continuation lines.
export function highlightPapyrusLines(source: string): string[] {
  const lines: string[] = [];
  let current = "";

  for (const token of tokenize(source)) {
    const parts = token.text.split("\n");
    parts.forEach((part, i) => {
      if (i > 0) {
        lines.push(current);
        current = "";
      }
      if (part.length === 0) {
        return;
      }
      const escaped = escapeHtml(part);
      current += token.cls ? `<span class="cm-${token.cls}">${escaped}</span>` : escaped;
    });
  }
  lines.push(current);

  return lines;
}
