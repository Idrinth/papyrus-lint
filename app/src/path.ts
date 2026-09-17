// Path helpers: no state, no UI, no Tauri calls — pure string manipulation
// shared by main.ts's drop handling and tests. Project-root discovery
// itself (projectDirForAchlist/projectDirForDirectory/projectDirForPscPath)
// lives in project.ts, since it now calls into the Rust backend.

const ACHLIST_EXTENSION = ".achlist";
const PSC_EXTENSION = ".psc";

export function isAchlistPath(path: string): boolean {
  return path.toLowerCase().endsWith(ACHLIST_EXTENSION);
}

export function isPscPath(path: string): boolean {
  return path.toLowerCase().endsWith(PSC_EXTENSION);
}

export function dirnameOf(path: string): string {
  const index = Math.max(path.lastIndexOf("/"), path.lastIndexOf("\\"));
  return index === -1 ? path : path.slice(0, index);
}

// Formats `path` relative to `base` (the project root; see
// projectDirForAchlist/projectDirForPscPath) for display in the lint
// results list, so long absolute paths stay readable. Falls back to the
// absolute path if `base` isn't known yet or `path` doesn't live under it.
export function relativePath(path: string, base: string | null): string {
  if (!base) {
    return path;
  }
  for (const sep of ["/", "\\"]) {
    const prefix = base.endsWith(sep) ? base : `${base}${sep}`;
    if (path.startsWith(prefix)) {
      return path.slice(prefix.length);
    }
  }
  return path;
}

export function scriptRootsForAchlist(entries: string[]): string[] {
  return [...new Set(entries.filter(isPscPath).map(dirnameOf))];
}
