// Property/method autocompletion for the code viewer's editor.
//
// The Rust side (`app/crates/papyrus-lint-core/src/function_table.rs`, exposed
// as the `list_script_members` Tauri command) already knows how to list
// every function/property available on a given script type, walking its
// `Extends` chain. What's missing is the frontend half: figuring out, from
// the raw text being edited and the cursor position, which type a `.` was
// just typed after, and turning the resulting member list into an
// insertable dropdown.
//
// Like the raw-text-based lints in app/crates/papyrus-lints (rather than the
// AST-based parser), the declaration scanner below works off regular
// expressions instead of requiring the script to parse cleanly - the
// editor's content is mid-edit essentially all the time, so autocompletion
// has to tolerate code that doesn't parse (a trailing "akRef." is by
// definition not yet valid Papyrus).

import { KEYWORDS } from "./highlight";

export interface TypeNameRef {
  name: string;
  is_array: boolean;
}

export interface ParamRef {
  name: string;
  type_name: TypeNameRef;
}

export interface FunctionMember {
  kind: "function";
  name: string;
  params: ParamRef[];
  return_type: TypeNameRef | null;
  is_global: boolean;
  is_native: boolean;
  is_event: boolean;
}

export interface PropertyMember {
  kind: "property";
  name: string;
  type_name: TypeNameRef;
}

// Mirrors `papyrus_lint_core::function_table::Member`, as returned by the
// `list_script_members` Tauri command.
export type Member = FunctionMember | PropertyMember;

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

const IDENTIFIER = "[A-Za-z_]\\w*";

// A declaration-like "Type name" pair, immediately followed by "=", ";", or
// end of line - distinguishing `ObjectReference akRef = None` (a
// declaration) from `akRef.MoveTo(akTarget)` or `akRef = None` (not one).
const VARIABLE_DECLARATION = new RegExp(`^[ \\t]*(${IDENTIFIER})(?:\\[\\])?\\s+(${IDENTIFIER})\\s*(?:=|;|$)`, "gim");

const PROPERTY_DECLARATION = new RegExp(`\\b(${IDENTIFIER})(?:\\[\\])?\\s+Property\\s+(${IDENTIFIER})\\b`, "gi");

const FUNCTION_PARAMS = new RegExp(`\\b(?:Function|Event)\\s+${IDENTIFIER}\\s*\\(([^)]*)\\)`, "gi");

const PARAM = new RegExp(`^\\s*(${IDENTIFIER})(?:\\[\\])?\\s+(${IDENTIFIER})`);

const SCRIPT_NAME = new RegExp(`^\\s*ScriptName\\s+(${IDENTIFIER})(?:\\s+Extends\\s+(${IDENTIFIER}))?`, "im");

// Scans `source` for every "Type name" declaration it can find - the
// script's own name/Extends parent (as "self"/"parent"), Property
// declarations, plain variable declarations (locals and script-level
// fields alike), and Function/Event parameters - and returns a map from
// each declared identifier (lowercased) to its declared type name.
//
// This isn't scope-aware (a local reused with a different type in another
// function would collide), matching the same text-based tradeoff the
// existing lints make in exchange for working on code that doesn't parse.
export function declaredTypes(source: string): Map<string, string> {
  const types = new Map<string, string>();
  const clean = stripComments(source);

  const remember = (name: string, type: string) => {
    const nameLower = name.toLowerCase();
    const typeLower = type.toLowerCase();
    if (KEYWORDS.has(nameLower) || KEYWORDS.has(typeLower)) {
      return;
    }
    types.set(nameLower, type);
  };

  // "self" and "parent" are themselves Papyrus keywords, so they're set
  // directly rather than through `remember` (which rejects a keyword name).
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

export interface CompletionQuery {
  // The declared type of the expression before the ".", e.g. "ObjectReference".
  receiverType: string;
  // What's been typed of the member name so far (possibly empty, right
  // after typing the ".").
  prefix: string;
  // Index into the source at which `prefix` starts, i.e. where an accepted
  // completion should be spliced in.
  prefixStart: number;
}

// Looks for a `receiver.prefix` immediately ending at `cursorIndex` (no
// intervening whitespace), and, if `receiver`'s declared type is known,
// returns enough to query and splice in its members. Returns null if the
// text just before the cursor isn't a simple member access (nothing to
// autocomplete: a compound receiver like `Foo().bar`, or an identifier
// whose type isn't known) or its receiver's type can't be resolved.
export function completionQueryAt(source: string, cursorIndex: number): CompletionQuery | null {
  const before = source.slice(0, cursorIndex);
  const match = new RegExp(`(${IDENTIFIER})\\.(\\w*)$`).exec(before);
  if (!match) {
    return null;
  }
  const [, receiver, prefix] = match;
  const receiverType = declaredTypes(source).get(receiver.toLowerCase());
  if (!receiverType) {
    return null;
  }
  return { receiverType, prefix, prefixStart: cursorIndex - prefix.length };
}

// Members whose name starts with `prefix` (case-insensitively), sorted
// alphabetically.
export function filterMembers(members: Member[], prefix: string): Member[] {
  const prefixLower = prefix.toLowerCase();
  return members
    .filter((member) => member.name.toLowerCase().startsWith(prefixLower))
    .sort((a, b) => a.name.localeCompare(b.name));
}

// The label shown for `member` in the dropdown.
export function completionLabel(member: Member): string {
  if (member.kind === "property") {
    return `${member.name}: ${member.type_name.name}${member.type_name.is_array ? "[]" : ""}`;
  }
  const params = member.params
    .map((param) => `${param.type_name.name}${param.type_name.is_array ? "[]" : ""} ${param.name}`)
    .join(", ");
  const returns = member.return_type ? ` -> ${member.return_type.name}${member.return_type.is_array ? "[]" : ""}` : "";
  return `${member.name}(${params})${returns}`;
}

// The text spliced in when `member` is accepted: just the name for a
// property, or the name plus an opening paren for a function (positioning
// the cursor ready to type its arguments).
export function completionInsertText(member: Member): string {
  return member.kind === "function" ? `${member.name}(` : member.name;
}
