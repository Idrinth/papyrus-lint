<!-- Extracted from AGENTS.md so the always-on agent index stays small. -->
# Development

Crate-local test and run commands live in that crate's `README.md`
(`app/crates/*/README.md`, and `app/src-tauri/README.md` for the desktop
shell). Do not copy them here.

- Rule metadata: `shared/rules.json` (read by the Rust crates' build
  scripts, `pages/build.py`, and its own tests) is generated, not
  checked in — run `python3 .github/scripts/build_rules_json.py` after
  cloning and again whenever a `shared/rules/*.json` file changes, before
  any of the commands below that touch it.
- Frontend (`app/`): `npm install`, then `npm run dev` (Vite dev server) or
  `npm run build` (generate `src/config-types.ts`, typecheck + build).
  `npm run generate:config-types` runs `app/scripts/generate-config-types.mjs`
  against `shared/rules/*.json` and `configuration/papyrus-lint.default.yaml`;
  the result is git-ignored, the same way `papyrus-lints/build.rs` writes
  `Rules` into `$OUT_DIR`. That file's `RULE_SETTINGS` is also what fills
  the Settings tab's lint-rule checkboxes; `app/index.html` only keeps the
  empty `#lint-rules` fieldset. `npm run test` runs the frontend's
  Vitest unit tests (`src/**/*.test.ts`); `npm run test:coverage` runs the
  same suite instrumented with `@vitest/coverage-v8`, printing a text
  report and writing HTML/lcov reports to `coverage/`.
  `npm run lint` runs ESLint (flat config in `eslint.config.js`) over `src/`,
  using `typescript-eslint`'s recommended rules plus `@vitest/eslint-plugin`'s
  recommended rules on test files. `npm run lint:css` runs stylelint (config
  in `.stylelintrc.json`, extending `stylelint-config-recommended`) over
  `src/**/*.css`. `npm run test:browser` runs `app/e2e/*.spec.ts` (config in
  `playwright.config.ts`) against a real Chromium instance (via
  `@playwright/test`, browsers installed separately with `npx playwright
  install --with-deps chromium`) rather than jsdom, starting the Vite dev
  server itself: jsdom (used by `npm run test` above) never computes an
  actual box model, so it can't catch element-size/layout regressions
  (a collapsed drop zone, a mis-hidden tab panel, an overlay no longer
  matching its underlying element's dimensions, horizontal overflow) the
  way these tests do.
  - `typescript-eslint` doesn't yet support TypeScript 7 (this repo's
    `typescript` devDependency), so `app/package.json` installs it under an
    npm alias: `typescript` resolves to the `@typescript/typescript6` shim
    (TS 6, satisfying typescript-eslint) and the real TS 7 compiler is
    installed separately as `@typescript/native`, which is what `tsc`
    (used by `npm run build`) actually runs. See
    https://devblogs.microsoft.com/typescript/announcing-typescript-7-0/#running-side-by-side-with-typescript-6.0.
- Full desktop app: `npm run tauri dev` / `npm run tauri build` (from `app/`).
  The desktop shell is built with [Tauri](https://tauri.app/), so building
  it requires Tauri's platform prerequisites (a Rust toolchain, plus the
  usual webview dependencies for your OS — see the
  [Tauri prerequisites guide](https://v2.tauri.app/start/prerequisites/)).
  Rust-only checks for that shell are in `app/src-tauri/README.md`.
- VS Code extension (`vscode-extension/`): `npm install`, then `npm run
  watch` (or `npm run compile` for a one-off build) and F5 in VS Code to
  launch an Extension Development Host. Not part of the app's npm
  project — it has its own `package.json`/`tsconfig.json`/`eslint.config.js`.
- Rust coverage for a linting crate: `cargo llvm-cov --manifest-path
  <crate>/Cargo.toml` (requires the
  [`cargo-llvm-cov`](https://github.com/taiki-e/cargo-llvm-cov) subcommand
  and the `llvm-tools-preview` rustup component).
