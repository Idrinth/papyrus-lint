import { memberDocumentation } from "./autocomplete-members";
import type { IdentifierAt, Member } from "./autocomplete-types";
import { declaredTypes, IDENTIFIER, stripComments } from "./papyrus-source";

function lastPhysicalLine(lines: string[], start: number): number {
  let index = start;
  while (index < lines.length && /\\\s*$/.test(lines[index]!)) index += 1;
  return index;
}

function braceCommentAtLine(lines: string[], line: number): string | null {
  const text = lines[line];
  if (text === undefined || !text.trimStart().startsWith("{")) return null;
  const fromHere = lines.slice(line).join("\n");
  const open = fromHere.indexOf("{");
  if (open < 0) return null;
  const close = fromHere.indexOf("}", open + 1);
  const inner = close < 0 ? fromHere.slice(open + 1) : fromHere.slice(open + 1, close);
  const doc = inner.replace(/\r\n/g, "\n").replace(/\r/g, "\n").trim();
  return doc || null;
}

const SCRIPT_NAME_LINE = new RegExp(`^\\s*ScriptName\\s+(${IDENTIFIER})\\b`, "i");
const PROPERTY_NAME_LINE = new RegExp(`\\bProperty\\s+(${IDENTIFIER})\\b`, "i");
const FUNCTION_NAME_LINE = new RegExp(`\\b(?:Function|Event)\\s+(${IDENTIFIER})\\b`, "i");

export function documentationByName(source: string): Map<string, string> {
  const docs = new Map<string, string>();
  const originalLines = source.split("\n");
  const cleanLines = stripComments(source).split("\n");
  const remember = (name: string, headerLine: number) => {
    const doc = braceCommentAtLine(originalLines, lastPhysicalLine(cleanLines, headerLine) + 1);
    if (doc) docs.set(name.toLowerCase(), doc);
  };

  for (let line = 0; line < cleanLines.length; line++) {
    const text = cleanLines[line]!;
    const script = SCRIPT_NAME_LINE.exec(text);
    if (script) {
      remember(script[1], line);
      const doc = docs.get(script[1].toLowerCase());
      if (doc) docs.set("self", doc);
      continue;
    }
    const property = PROPERTY_NAME_LINE.exec(text);
    if (property) {
      remember(property[1], line);
      continue;
    }
    const fn = FUNCTION_NAME_LINE.exec(text);
    if (fn) remember(fn[1], line);
  }
  return docs;
}

export function declarationDocumentationOnLine(source: string, line: number): string | null {
  const docs = documentationByName(source);
  const text = stripComments(source).split("\n")[line - 1];
  if (text === undefined) return null;
  const script = SCRIPT_NAME_LINE.exec(text);
  if (script) return docs.get(script[1].toLowerCase()) ?? docs.get("self") ?? null;
  const property = PROPERTY_NAME_LINE.exec(text);
  if (property) return docs.get(property[1].toLowerCase()) ?? null;
  const fn = FUNCTION_NAME_LINE.exec(text);
  return fn ? (docs.get(fn[1].toLowerCase()) ?? null) : null;
}

export function documentationForIdentifier(
  source: string,
  ident: IdentifierAt,
  membersFor: (typeName: string) => Member[] | undefined,
): string | null {
  const docs = documentationByName(source);
  if (!ident.receiver) return docs.get(ident.name.toLowerCase()) ?? null;
  const types = declaredTypes(source);
  const receiverType = types.get(ident.receiver.toLowerCase());
  if (!receiverType) return docs.get(ident.name.toLowerCase()) ?? null;
  const selfType = types.get("self");
  if (selfType?.toLowerCase() === receiverType.toLowerCase()) {
    const local = docs.get(ident.name.toLowerCase());
    if (local) return local;
  }
  const member = membersFor(receiverType)?.find((candidate) => candidate.name.toLowerCase() === ident.name.toLowerCase());
  return memberDocumentation(member);
}

export function overlayLocalDocumentation(members: Member[], source: string, receiverType: string): Member[] {
  const selfType = declaredTypes(source).get("self");
  if (!selfType || selfType.toLowerCase() !== receiverType.toLowerCase()) return members;
  const docs = documentationByName(source);
  return members.map((member) => {
    const doc = docs.get(member.name.toLowerCase());
    return doc ? { ...member, doc } : member;
  });
}
