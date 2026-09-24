import type { Member } from "./autocomplete-types";

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
