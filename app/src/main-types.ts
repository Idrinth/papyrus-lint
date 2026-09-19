// Shared configuration-picker types. Kept out of main.ts so presets and
// tests can import them without pulling in the UI façade (or exporting
// them from main.ts only so tests can reach them).

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

// What promptForConfigSelection resolved to (see useProjectDir): stick with
// whatever useProjectDir's own auto-detection would already do ("detected" —
// the project's existing papyrus-lint.yaml/.yml, or the engine's silent
// defaults if it has none), point at a specific configuration file instead
// ("path"), or seed a fresh one from a preset ("preset", handled the same
// way applyConfigPreset already is elsewhere).
export type ConfigSelectionResult =
  | { kind: "detected" }
  | { kind: "path"; path: string }
  | { kind: "preset"; preset: string };
