// Shared configuration-picker types. Kept out of main.ts so presets and
// tests can import them without pulling in the UI façade (or exporting
// them from main.ts only so tests can reach them).

import { SELECTABLE_GAMES, type Game } from "./config-types";

// One configuration preset's identity/description — a built-in one, or a
// user preset found under a presets directory next to the executable — as
// returned by the backend's list_config_presets command
// (papyrus_lint_core::presets::PresetInfo, made JSON-friendly). Offered
// inline in the config-picker dialog (see promptForConfigSelection) for a
// project directory that has no papyrus-lint.yaml/.yml of its own yet.
export interface ConfigPreset {
  id: string;
  label: string;
  description: string;
}

// Games the desktop picker, and the editor init prompts, can write.
export function isSelectableGame(value: string | null | undefined): value is Game {
  return !!value && (SELECTABLE_GAMES as readonly string[]).includes(value);
}

// What promptForConfigSelection resolved to (see loadProjectConfig): stick
// with whatever useProjectDir's own auto-detection would already do
// ("detected" — the project's existing papyrus-lint.yaml/.yml, or the
// engine's silent defaults if it has none), point at a specific
// configuration file instead ("path"), or seed a fresh one from a preset
// ("preset", handled the same way applyConfigPreset already is elsewhere).
// `game` is set only when the project has no configuration file yet, so a
// choice made in the first-run picker can be written into that new file.
// Closing the dialog (Escape, backdrop) omits it and leaves the defaults.
export type ConfigSelectionResult =
  | { kind: "detected"; game?: Game }
  | { kind: "path"; path: string }
  | { kind: "preset"; preset: string; game: Game };
