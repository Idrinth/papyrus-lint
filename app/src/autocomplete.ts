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
  // Inner text of the `{ ... }` documentation comment after this function's
  // header, when the backend (or a local overlay of the buffer being edited)
  // found one. Absent/null/empty means there's nothing to show.
  doc?: string | null;
}

export interface PropertyMember {
  kind: "property";
  name: string;
  type_name: TypeNameRef;
  // Inner text of the `{ ... }` documentation comment after this property's
  // header; same rules as [`FunctionMember.doc`].
  doc?: string | null;
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

// Matches `receiver[index].prefix` - an array element access - ending at the
// cursor. Tried before the plain identifier pattern below, since an indexed
// receiver's own declared type (the array's element type, e.g. "Actor" for
// an `Actor[]`) is what member completion should resolve against, not the
// array itself.
const ARRAY_ELEMENT_RECEIVER = new RegExp(`(${IDENTIFIER})\\s*\\[[^[\\]]*\\]\\s*\\.(\\w*)$`);

const PLAIN_RECEIVER = new RegExp(`(${IDENTIFIER})\\.(\\w*)$`);

// Looks for a `receiver.prefix` or `receiver[index].prefix` immediately
// ending at `cursorIndex`, and, if `receiver`'s declared type is known,
// returns enough to query and splice in its members. Returns null if the
// text just before the cursor isn't a simple member access (nothing to
// autocomplete: a compound receiver like `Foo().bar`, or an identifier
// whose type isn't known) or its receiver's type can't be resolved.
export function completionQueryAt(source: string, cursorIndex: number): CompletionQuery | null {
  const before = source.slice(0, cursorIndex);
  const match = ARRAY_ELEMENT_RECEIVER.exec(before) ?? PLAIN_RECEIVER.exec(before);
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

// Inner text of `member`'s `{ ... }` documentation comment, or null when
// there isn't one to show.
export function memberDocumentation(member: Member | undefined): string | null {
  const doc = member?.doc?.trim();
  return doc ? doc : null;
}

// The identifier (if any) whose character occupies `index` in `source`,
// plus the identifier immediately before a `.` if this is a member access.
// Comments are ignored (via stripComments), so hovering a commented-out
// name doesn't resolve it. `index` may sit on the character after the last
// ident char (the usual "caret at the end" case) and still match.
export interface IdentifierAt {
  name: string;
  start: number;
  end: number;
  receiver: string | null;
}

function isIdentChar(character: string | undefined): boolean {
  return character !== undefined && /[A-Za-z_0-9]/.test(character);
}

export function identifierAt(source: string, index: number): IdentifierAt | null {
  if (source.length === 0) {
    return null;
  }
  const clean = stripComments(source);
  let pos = index;
  if (pos >= clean.length) {
    pos = clean.length - 1;
  }
  if (pos < 0) {
    return null;
  }
  if (!isIdentChar(clean[pos]) && pos > 0 && isIdentChar(clean[pos - 1])) {
    pos -= 1;
  }
  if (!isIdentChar(clean[pos])) {
    return null;
  }
  let start = pos;
  let end = pos + 1;
  while (start > 0 && isIdentChar(clean[start - 1])) {
    start -= 1;
  }
  while (end < clean.length && isIdentChar(clean[end])) {
    end += 1;
  }
  if (/[0-9]/.test(clean[start]!)) {
    return null;
  }
  const name = source.slice(start, end);
  if (!name) {
    return null;
  }

  let receiver: string | null = null;
  let cursor = start - 1;
  while (cursor >= 0 && (clean[cursor] === " " || clean[cursor] === "\t")) {
    cursor -= 1;
  }
  if (clean[cursor] === ".") {
    cursor -= 1;
    while (cursor >= 0 && (clean[cursor] === " " || clean[cursor] === "\t")) {
      cursor -= 1;
    }
    const recEnd = cursor + 1;
    while (cursor >= 0 && isIdentChar(clean[cursor])) {
      cursor -= 1;
    }
    const recStart = cursor + 1;
    if (recStart < recEnd && !/[0-9]/.test(clean[recStart]!)) {
      receiver = source.slice(recStart, recEnd);
    }
  }

  return { name, start, end, receiver };
}

// Last physical line (0-indexed) of a header that may be continued with a
// trailing `\`, matching papyrus-parser swallowing `\` + the newline so the
// documentation comment is looked for after the header's real end.
function lastPhysicalLine(lines: string[], start: number): number {
  let index = start;
  while (index < lines.length && /\\\s*$/.test(lines[index]!)) {
    index += 1;
  }
  return index;
}

function braceCommentAtLine(lines: string[], line: number): string | null {
  const text = lines[line];
  if (text === undefined) {
    return null;
  }
  const trimmed = text.trimStart();
  if (!trimmed.startsWith("{")) {
    return null;
  }
  const fromHere = lines.slice(line).join("\n");
  const open = fromHere.indexOf("{");
  if (open < 0) {
    return null;
  }
  const close = fromHere.indexOf("}", open + 1);
  const inner = close < 0 ? fromHere.slice(open + 1) : fromHere.slice(open + 1, close);
  const doc = inner.replace(/\r\n/g, "\n").replace(/\r/g, "\n").trim();
  return doc ? doc : null;
}

const SCRIPT_NAME_LINE = new RegExp(`^\\s*ScriptName\\s+(${IDENTIFIER})\\b`, "i");
const PROPERTY_NAME_LINE = new RegExp(`\\bProperty\\s+(${IDENTIFIER})\\b`, "i");
const FUNCTION_NAME_LINE = new RegExp(`\\b(?:Function|Event)\\s+(${IDENTIFIER})\\b`, "i");

// Maps each ScriptName / Property / Function / Event declaration in
// `source` (lowercased name) to the inner text of the `{ ... }` comment
// on the line immediately after its header. The script's own name is also
// recorded under "self", so hovering `self` can show the script header's
// documentation. Commented-out declarations are ignored.
export function documentationByName(source: string): Map<string, string> {
  const docs = new Map<string, string>();
  const originalLines = source.split("\n");
  const cleanLines = stripComments(source).split("\n");

  const remember = (name: string, headerLine: number) => {
    const last = lastPhysicalLine(cleanLines, headerLine);
    const doc = braceCommentAtLine(originalLines, last + 1);
    if (doc) {
      docs.set(name.toLowerCase(), doc);
    }
  };

  for (let line = 0; line < cleanLines.length; line++) {
    const text = cleanLines[line]!;
    const script = SCRIPT_NAME_LINE.exec(text);
    if (script) {
      remember(script[1], line);
      const doc = docs.get(script[1].toLowerCase());
      if (doc) {
        docs.set("self", doc);
      }
      continue;
    }
    const property = PROPERTY_NAME_LINE.exec(text);
    if (property) {
      remember(property[1], line);
      continue;
    }
    const fn = FUNCTION_NAME_LINE.exec(text);
    if (fn) {
      remember(fn[1], line);
    }
  }

  return docs;
}

// Documentation for the ScriptName / Property / Function / Event
// declaration on 1-indexed `line`, if that line is a header with a `{ ... }`
// comment after it. Used as a fallback when the pointer's column can't be
// resolved to a specific identifier (jsdom tests, or a hover on the
// declaration line itself).
export function declarationDocumentationOnLine(source: string, line: number): string | null {
  const docs = documentationByName(source);
  const cleanLines = stripComments(source).split("\n");
  const text = cleanLines[line - 1];
  if (text === undefined) {
    return null;
  }
  const script = SCRIPT_NAME_LINE.exec(text);
  if (script) {
    return docs.get(script[1].toLowerCase()) ?? docs.get("self") ?? null;
  }
  const property = PROPERTY_NAME_LINE.exec(text);
  if (property) {
    return docs.get(property[1].toLowerCase()) ?? null;
  }
  const fn = FUNCTION_NAME_LINE.exec(text);
  if (fn) {
    return docs.get(fn[1].toLowerCase()) ?? null;
  }
  return null;
}

// Resolves the `{ ... }` documentation for `ident` in `source`. A member
// access (`receiver.name`) prefers a locally-declared doc when `receiver`
// is this script (`self` / the ScriptName type), then falls back to
// `membersFor(receiverType)` (typically the `list_script_members` result
// for that type). A bare identifier looks up `documentationByName` only.
export function documentationForIdentifier(
  source: string,
  ident: IdentifierAt,
  membersFor: (typeName: string) => Member[] | undefined,
): string | null {
  const docs = documentationByName(source);
  if (!ident.receiver) {
    return docs.get(ident.name.toLowerCase()) ?? null;
  }
  const types = declaredTypes(source);
  const receiverType = types.get(ident.receiver.toLowerCase());
  if (!receiverType) {
    return docs.get(ident.name.toLowerCase()) ?? null;
  }
  const selfType = types.get("self");
  if (selfType && selfType.toLowerCase() === receiverType.toLowerCase()) {
    const local = docs.get(ident.name.toLowerCase());
    if (local) {
      return local;
    }
  }
  const member = membersFor(receiverType)?.find((candidate) => candidate.name.toLowerCase() === ident.name.toLowerCase());
  return memberDocumentation(member);
}

// When autocompleting members of the script currently being edited, overlay
// `{ ... }` comments from the unsaved buffer onto the backend's member list
// so a doc comment typed (or edited) since the last save still shows up.
export function overlayLocalDocumentation(members: Member[], source: string, receiverType: string): Member[] {
  const types = declaredTypes(source);
  const selfType = types.get("self");
  if (!selfType || selfType.toLowerCase() !== receiverType.toLowerCase()) {
    return members;
  }
  const docs = documentationByName(source);
  return members.map((member) => {
    const doc = docs.get(member.name.toLowerCase());
    return doc ? { ...member, doc } : member;
  });
}
