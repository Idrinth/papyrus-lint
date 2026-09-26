# Desktop UI

Framework-free TypeScript UI for the Tauri app. Reusable linting stays in
`app/crates/`.

- A dropped `.ppj`'s `<Import>` entries land in `currentPpjImportRoots`
  (`project-state.ts`) and are folded into `effectiveScriptRoots()` the
  same way `currentAchlistScriptRoots` is. Never `lookup_script_roots`.
- Dropped and re-linted batches go through one `lint_project_scripts`
  command (`drop.ts`). Each finished file is appended to the results
  list; finding rows stay unmounted once that list passes a few hundred
  of them (`results-list-render.ts`). Watch mode and the code viewer
  still lint one file at a time via `lint_psc_file`.
- The Settings tab and first-run picker offer `skyrim`, `fallout4`, and
  `starfield`. A project already set to an unrecognized `game` value is
  shown and kept, not rewritten.
- Lint-rule checkboxes come from generated `RULE_SETTINGS` in
  `config-types.ts`. `index.html` only keeps the empty `#lint-rules`
  fieldset. The other Settings controls (game, formatting, thresholds)
  come from generated `LINT_SETTINGS` and are rendered into
  `#lint-config-game` and `#lint-config-settings`. Both are produced from
  `configuration/lint-settings.json` plus `shared/rules/*.json`; don't
  hand-write either list.
- The code viewer and live editor highlight, title, and act on the same
  filtered findings the Lint results list is showing. `codeViewerState`
  still keeps the full lint result so a filter change can re-apply
  without re-linting.

Start at `drop.ts`, `results-filter.ts`, `live-edit.ts`, `watch.ts`, and
`presets.ts`.

From `app/`:

```sh
npm install
npm run dev
npm test
npm run lint
npm run lint:css
npm run build
npm run test:browser
```

`npm run build` generates git-ignored `src/config-types.ts`. `npm test`
is Vitest on jsdom and does not catch layout; `npm run test:browser` runs
`e2e/*.spec.ts` in Chromium. `typescript-eslint` still needs TypeScript 6,
aliased in `app/package.json`; `npm run build` uses `@typescript/native`.
