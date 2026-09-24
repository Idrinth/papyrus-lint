import type { IdentifierAt } from "./autocomplete-types";
import { KEYWORDS } from "./highlight";

// Strips Papyrus comments (block `;/ ... /;`, brace `{ ... }`, and line
// `; ...`), replacing their text with spaces (keeping newlines) so a
// commented-out declaration doesn't get picked up, while leaving every
// other character's position unchanged.
function blank(match: string): string {
  return match.replace(/[^\n]/g, " ");
}

export function stripComments(source: string): string {
  return source
    .replace(/;\/[\s\S]*?(?:\/;|$)/g, blank)
    .replace(/\{[^}]*\}?/g, blank)
    .replace(/;[^\n]*/g, blank);
}

export const IDENTIFIER = "[A-Za-z_]\\w*";

// A declaration-like "Type name" pair, immediately followed by "=", ";", or
// end of line - distinguishing `ObjectReference akRef = None` (a
// declaration) from `akRef.MoveTo(akTarget)` or `akRef = None` (not one).
const VARIABLE_DECLARATION = new RegExp(`^[ \\t]*(${IDENTIFIER})(?:\\[\\])?\\s+(${IDENTIFIER})\\s*(?:=|;|$)`, "gim");
const PROPERTY_DECLARATION = new RegExp(`\\b(${IDENTIFIER})(?:\\[\\])?\\s+Property\\s+(${IDENTIFIER})\\b`, "gi");
const FUNCTION_PARAMS = new RegExp(`\\b(?:Function|Event)\\s+${IDENTIFIER}\\s*\\(([^)]*)\\)`, "gi");
const PARAM = new RegExp(`^\\s*(${IDENTIFIER})(?:\\[\\])?\\s+(${IDENTIFIER})`);
const SCRIPT_NAME = new RegExp(`^\\s*ScriptName\\s+(${IDENTIFIER})(?:\\s+Extends\\s+(${IDENTIFIER}))?`, "im");

// Scans `source` for every "Type name" declaration it can find. This isn't
// scope-aware, trading precision for the ability to inspect incomplete code.
export function declaredTypes(source: string): Map<string, string> {
  const types = new Map<string, string>();
  const clean = stripComments(source);
  const remember = (name: string, type: string) => {
    const nameLower = name.toLowerCase();
    const typeLower = type.toLowerCase();
    if (!KEYWORDS.has(nameLower) && !KEYWORDS.has(typeLower)) {
      types.set(nameLower, type);
    }
  };

  const scriptMatch = SCRIPT_NAME.exec(clean);
  if (scriptMatch) {
    types.set("self", scriptMatch[1]);
    if (scriptMatch[2]) {
      types.set("parent", scriptMatch[2]);
    }
  }
  for (const match of clean.matchAll(PROPERTY_DECLARATION)) {
    remember(match[2], match[1]);
  }
  for (const match of clean.matchAll(VARIABLE_DECLARATION)) {
    remember(match[2], match[1]);
  }
  for (const call of clean.matchAll(FUNCTION_PARAMS)) {
    for (const param of call[1].split(",")) {
      const paramMatch = PARAM.exec(param);
      if (paramMatch) {
        remember(paramMatch[2], paramMatch[1]);
      }
    }
  }
  return types;
}

function isIdentChar(character: string | undefined): boolean {
  return character !== undefined && /[A-Za-z_0-9]/.test(character);
}

// Finds the identifier at `index` and, for member access, its receiver.
export function identifierAt(source: string, index: number): IdentifierAt | null {
  if (source.length === 0) return null;
  const clean = stripComments(source);
  let pos = Math.min(index, clean.length - 1);
  if (pos < 0) return null;
  if (!isIdentChar(clean[pos]) && pos > 0 && isIdentChar(clean[pos - 1])) pos -= 1;
  if (!isIdentChar(clean[pos])) return null;

  let start = pos;
  let end = pos + 1;
  while (start > 0 && isIdentChar(clean[start - 1])) start -= 1;
  while (end < clean.length && isIdentChar(clean[end])) end += 1;
  if (/[0-9]/.test(clean[start]!)) return null;
  const name = source.slice(start, end);
  if (!name) return null;

  let receiver: string | null = null;
  let cursor = start - 1;
  while (cursor >= 0 && (clean[cursor] === " " || clean[cursor] === "\t")) cursor -= 1;
  if (clean[cursor] === ".") {
    cursor -= 1;
    while (cursor >= 0 && (clean[cursor] === " " || clean[cursor] === "\t")) cursor -= 1;
    const receiverEnd = cursor + 1;
    while (cursor >= 0 && isIdentChar(clean[cursor])) cursor -= 1;
    const receiverStart = cursor + 1;
    if (receiverStart < receiverEnd && !/[0-9]/.test(clean[receiverStart]!)) {
      receiver = source.slice(receiverStart, receiverEnd);
    }
  }
  return { name, start, end, receiver };
}
