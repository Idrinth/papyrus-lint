// Path/project-root resolution helpers: no state, no UI, no Tauri calls —
// pure string manipulation shared by main.ts's drop handling and tests.

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

function basenameOf(path: string): string {
  const index = Math.max(path.lastIndexOf("/"), path.lastIndexOf("\\"));
  return index === -1 ? path : path.slice(index + 1);
}

// `path` itself, followed by each of its ancestor directories up to the
// root (where dirnameOf stops changing anything), mirroring Rust's
// `Path::ancestors()`.
function ancestorsOf(path: string): string[] {
  const ancestors = [path];
  let current = path;
  for (;;) {
    const parent = dirnameOf(current);
    if (parent === current) {
      return ancestors;
    }
    ancestors.push(parent);
    current = parent;
  }
}

// (outer, inner) pairs, mirroring papyrus-lint-cli's `CANDIDATE_DIRS`
// (`scripts/source`, `source/scripts`).
const CANDIDATE_DIR_PAIRS: readonly (readonly [string, string])[] = [
  ["scripts", "source"],
  ["source", "scripts"],
];

// Mirrors papyrus-lint-cli's `find_candidate_pair_root`: walks up `path`'s
// ancestors looking for a `scripts/source`/`source/scripts` directory pair
// (matched case-insensitively), and returns the directory above that pair,
// or null if no such pair appears anywhere in `path`'s ancestry.
export function findCandidatePairRoot(path: string): string | null {
  const ancestors = ancestorsOf(path);
  for (let i = 1; i < ancestors.length - 1; i++) {
    const innerName = basenameOf(ancestors[i]).toLowerCase();
    const outerName = basenameOf(ancestors[i + 1]).toLowerCase();
    const matches = CANDIDATE_DIR_PAIRS.some(([outer, inner]) => outer === outerName && inner === innerName);
    if (matches) {
      return dirnameOf(ancestors[i + 1]);
    }
  }
  return null;
}

// Finds the project root for a dropped `.achlist`: tries each of its
// resolved `.psc` entries' own position under a `scripts/source`/
// `source/scripts` directory pair first (see findCandidatePairRoot), so a
// project whose `.achlist` doesn't live in the project root itself (e.g. it
// was dropped next to a game's `Data` directory while the project lives in
// a subfolder) still resolves correctly. Falls back to the achlist's own
// parent directory (the conventional layout) if none of its entries match.
export function projectDirForAchlist(achlistPath: string, entries: string[]): string {
  for (const entry of entries) {
    if (!isPscPath(entry)) {
      continue;
    }
    const root = findCandidatePairRoot(entry);
    if (root) {
      return root;
    }
  }
  return dirnameOf(achlistPath);
}

// Finds the project root for a dropped directory (see handleDroppedPaths'
// directory-scan mode, for a project with no .achlist at all whose scripts
// are spread across arbitrarily nested subfolders, e.g. Requiem's own
// layout): tries each recursively-found .psc entry's own position under a
// `scripts/source`/`source/scripts` directory pair first (see
// findCandidatePairRoot), the same way projectDirForAchlist does for an
// achlist's entries. Falls back to the dropped directory itself if none of
// the entries match that layout, since there's no achlist file whose parent
// directory would otherwise apply.
export function projectDirForDirectory(dirPath: string, entries: string[]): string {
  for (const entry of entries) {
    const root = findCandidatePairRoot(entry);
    if (root) {
      return root;
    }
  }
  return dirPath;
}

// A bare `.psc` file conventionally lives two directories under the project
// root (e.g. `Data/Scripts/Source/abc.psc` under `Data`), matching
// papyrus-lint-cli's handling of a `.psc` path given directly (see its
// `root_ancestor_levels`) so a project's `papyrus-lint.yaml` and
// cross-script lookups are still found for a script dropped on its own,
// without an `.achlist`.
export function projectDirForPscPath(path: string): string {
  return dirnameOf(dirnameOf(dirnameOf(path)));
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
