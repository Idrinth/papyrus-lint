# Desktop UI

Framework-free TypeScript UI for the Tauri app. Reusable linting stays in
`app/crates/`.

- A dropped `.ppj`'s `<Import>` entries land in `currentPpjImportRoots`
  (`project-state.ts`) and are folded into `effectiveScriptRoots()` the
  same way `currentAchlistScriptRoots` is. Never `lookup_script_roots`.
- The Settings tab and first-run picker offer `skyrim`, `fallout4`, and
  `starfield`. A project already set to an unrecognized `game` value is
  shown and kept, not rewritten.
- Lint-rule checkboxes come from generated `RULE_SETTINGS` in
  `config-types.ts`. `index.html` only keeps the empty `#lint-rules`
  fieldset.

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
