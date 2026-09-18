import type { Member } from "./autocomplete";
import { listScriptMembers } from "./backend";

// Shared by autocompletion and hover documentation (see live-edit-pointer.ts
// and live-edit-autocomplete.ts), which otherwise repeat the same
// `listScriptMembers` lookup for a receiver's declared type as the user
// types or moves the pointer.
const membersByType = new Map<string, Member[]>();

export async function cachedMembersForType(typeName: string): Promise<Member[]> {
  const key = typeName.toLowerCase();
  const cached = membersByType.get(key);
  if (cached) {
    return cached;
  }
  const members = await listScriptMembers(typeName);
  membersByType.set(key, members);
  return members;
}

// A synchronous read of whatever's already cached, for callbacks (documentation
// lookups keyed by an arbitrary type name encountered while walking a
// declaration) that can't await a fresh `cachedMembersForType` lookup.
export function getCachedMembers(typeName: string): Member[] | undefined {
  return membersByType.get(typeName.toLowerCase());
}

// Clears the cache, e.g. when live editing is cancelled - a later edit
// session (possibly on a different file) must not reuse another file's
// members.
export function resetMemberCache(): void {
  membersByType.clear();
}
