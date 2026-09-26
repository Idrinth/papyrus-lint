// A minimal DOM fixture mirroring the elements index.html defines that
// main.ts (and its feature modules) look up by id on DOMContentLoaded.
// Kept in one place so the per-module UI tests build the same structure they
// expect to wire up.
export const FIXTURE_HTML = `
  <div class="theme-switch">
    <select id="theme-select">
      <option value="system">System</option>
      <option value="light">Light</option>
      <option value="dark">Dark</option>
    </select>
  </div>

  <main class="container">
    <div class="tabs">
      <div class="tabs__list" role="tablist">
        <button type="button" id="tab-import" class="tabs__tab" role="tab" aria-selected="true">Import</button>
        <button type="button" id="tab-settings" class="tabs__tab" role="tab" aria-selected="false">Settings</button>
        <button type="button" id="tab-presets" class="tabs__tab" role="tab" aria-selected="false" hidden>Presets</button>
        <button type="button" id="tab-files" class="tabs__tab" role="tab" aria-selected="false">Files</button>
        <button type="button" id="tab-lint" class="tabs__tab" role="tab" aria-selected="false">Lint results</button>
        <button type="button" id="tab-contact" class="tabs__tab" role="tab" aria-selected="false">Contact</button>
      </div>

      <div id="lint-progress" class="lint-progress" hidden aria-live="polite">
        <div class="lint-progress__status">
          <span class="lint-progress__spinner" aria-hidden="true"></span>
          <label id="lint-progress-label" for="lint-progress-bar" class="lint-progress__label"></label>
        </div>
        <progress id="lint-progress-bar" value="0" max="1"></progress>
      </div>

      <div id="panel-import" class="tabs__panel" role="tabpanel">
        <div id="drop-zone" class="drop-zone">
          <div id="drop-zone-loading" class="drop-zone__loading" role="status" hidden>
            <span class="drop-zone__spinner" aria-hidden="true"></span>
            <span>Finding Papyrus files…</span>
          </div>
          <p id="drop-zone-error" class="drop-zone__error" aria-live="polite"></p>
        </div>
      </div>

      <div id="panel-settings" class="tabs__panel" role="tabpanel" hidden>
        <p id="settings-locked-notice" class="settings-locked-notice" aria-live="polite"></p>
        <fieldset id="settings-fieldset" class="settings-fieldset" disabled>
        <output id="detected-script-roots">No project loaded</output>
        <output id="used-configuration-file">No project loaded</output>
        <div id="lint-config-game"></div>
        <input id="config-path-override" type="text" />
        <input id="compiler-path" type="text" />
        <input id="compile-check" type="checkbox" />
        <textarea id="script-roots"></textarea>
        <textarea id="lookup-script-roots"></textarea>
        <div id="lint-config-settings"></div>
        <fieldset id="lint-rules"></fieldset>
        <button type="button" id="save-config-as-preset">Save current settings as preset&hellip;</button>
        <select id="reset-to-preset-select"></select>
        <button type="button" id="reset-to-preset">Reset&hellip;</button>
        </fieldset>
      </div>

      <div id="panel-presets" class="tabs__panel" role="tabpanel" hidden>
        <ul id="preset-management-list"></ul>
      </div>

      <div id="panel-files" class="tabs__panel" role="tabpanel" hidden>
        <div id="achlist-result" class="achlist-result" hidden>
          <h2 id="achlist-result-title"></h2>
          <ul id="achlist-result-list"></ul>
        </div>
      </div>

      <div id="panel-lint" class="tabs__panel" role="tabpanel" hidden>
        <div id="psc-result" class="psc-result" hidden>
          <label>
            <input type="checkbox" id="watch-mode-toggle" />
          </label>
          <span id="watch-mode-status"></span>
          <input id="filename-filter" type="text" />
          <fieldset id="psc-result-filters">
            <input type="checkbox" id="filter-error" checked />
            <input type="checkbox" id="filter-warning" checked />
            <input type="checkbox" id="filter-info" checked />
          </fieldset>
          <fieldset id="psc-result-tag-filters">
            <input type="checkbox" id="filter-kind-style" checked />
            <select id="filter-rule-style" multiple></select>
            <input type="checkbox" id="filter-kind-performance" checked />
            <select id="filter-rule-performance" multiple></select>
            <input type="checkbox" id="filter-kind-correctness" checked />
            <select id="filter-rule-correctness" multiple></select>
            <input type="checkbox" id="filter-kind-maintainability" checked />
            <select id="filter-rule-maintainability" multiple></select>
          </fieldset>
          <fieldset id="psc-result-tag-importance-filters">
            <input type="checkbox" id="filter-importance-low" checked />
            <input type="checkbox" id="filter-importance-medium" checked />
            <input type="checkbox" id="filter-importance-high" checked />
          </fieldset>
          <fieldset id="psc-result-auto-fixable-filter">
            <input type="checkbox" id="filter-auto-fixable-only" />
          </fieldset>
          <div id="psc-result-mass-fix" hidden>
            <ul id="psc-result-mass-fix-list"></ul>
          </div>
          <div id="psc-result-export">
            <select id="export-format">
              <option value="text">Text</option>
              <option value="json">JSON</option>
            </select>
            <button type="button" id="export-issues-button" disabled>Export issues</button>
            <button
              type="button"
              id="export-ai-button"
              title="Export a self-contained JSON file with the Papyrus source, findings, rule details, and Papyrus Lint version—everything an AI assistant needs to help resolve the issues"
              disabled
            >Export for AI</button>
            <label for="export-ai-hash-source">
              <input type="checkbox" id="export-ai-hash-source" />
              Redact source (attach hash only)
            </label>
          </div>
          <ul id="psc-result-list"></ul>
        </div>
      </div>

      <div id="panel-contact" class="tabs__panel" role="tabpanel" hidden>
        <ul class="contact-list">
          <li><a href="https://discord.gg/idrinth">Discord</a></li>
          <li><a href="https://www.nexusmods.com/skyrimspecialedition/mods/189862">NexusMods</a></li>
          <li><a href="https://github.com/idrinth/papyrus-lint">GitHub</a></li>
          <li><a href="https://tally.so/r/aQL1dB">Feedback</a></li>
        </ul>
      </div>
    </div>
  </main>

  <dialog id="code-viewer" class="code-viewer">
    <div class="code-viewer__header">
      <h2 id="code-viewer-title" class="code-viewer__title"></h2>
      <div class="code-viewer__actions">
        <button type="button" id="code-viewer-edit" class="code-viewer__action">Edit</button>
        <button type="button" id="code-viewer-fix" class="code-viewer__action" hidden>Apply fixes</button>
        <button type="button" id="code-viewer-preview-fix" class="code-viewer__action" hidden>Preview fixes</button>
        <button type="button" id="code-viewer-save" class="code-viewer__action" hidden>Save</button>
        <button type="button" id="code-viewer-save-compile" class="code-viewer__action" hidden>Save & Compile</button>
        <button type="button" id="code-viewer-cancel" class="code-viewer__action" hidden>Cancel</button>
      </div>
      <button type="button" id="code-viewer-fullscreen" aria-label="Enter fullscreen" aria-pressed="false">⛶</button>
      <button type="button" id="code-viewer-close" aria-label="Close">&times;</button>
    </div>
    <div id="code-viewer-body" class="code-viewer__body">
      <div class="code-viewer__stage">
        <div id="code-viewer-view" class="code-viewer__view"></div>
        <div id="code-viewer-editor" class="code-viewer__editor" hidden>
          <div id="code-viewer-editor-gutter" class="code-viewer__editor-gutter" aria-hidden="true"></div>
          <div class="code-viewer__editor-code">
            <pre id="code-viewer-editor-highlight" class="code-viewer__editor-highlight" aria-hidden="true"><code></code></pre>
            <textarea id="code-viewer-editor-textarea" class="code-viewer__editor-textarea" spellcheck="false"></textarea>
            <ul id="code-viewer-autocomplete" class="code-viewer__autocomplete" role="listbox" hidden></ul>
          </div>
        </div>
      </div>
      <pre id="code-viewer-compile-output" class="code-viewer__compile-output" hidden></pre>
      <pre id="code-viewer-diff-output" class="code-viewer__diff-output" hidden></pre>
    </div>
  </dialog>

  <dialog id="config-picker" class="config-picker">
    <div class="config-picker__header">
      <h2 class="config-picker__title">Select this project's configuration</h2>
      <button type="button" id="config-picker-continue" class="config-picker__skip">Continue</button>
    </div>
    <p id="config-picker-detected" class="config-picker__detected" hidden>
      Found <code id="config-picker-detected-path"></code> for this project.
    </p>
    <p id="config-picker-none" class="config-picker__none" hidden>
      This project doesn't have a papyrus-lint.yaml yet.
    </p>
    <div id="config-picker-game" class="config-picker__game" hidden>
      <label for="config-picker-game-select">Target game</label>
      <select id="config-picker-game-select"></select>
    </div>
    <div id="config-picker-preset-list" class="config-picker__list" hidden></div>
    <div class="config-picker__browse">
      <label for="config-picker-path-input">Use a different configuration file instead</label>
      <input id="config-picker-path-input" type="text" />
      <button type="button" id="config-picker-use-path">Use this file</button>
    </div>
  </dialog>
`;

// jsdom doesn't implement <dialog>'s showModal()/close(), which main.ts
// relies on for the code viewer; polyfill just enough of it for tests.
function polyfillDialog() {
  const proto = HTMLDialogElement.prototype as HTMLDialogElement & {
    showModal?: () => void;
    close?: () => void;
  };
  if (!proto.showModal) {
    proto.showModal = function (this: HTMLDialogElement) {
      this.setAttribute("open", "");
    };
  }
  if (!proto.close) {
    proto.close = function (this: HTMLDialogElement) {
      this.removeAttribute("open");
      this.dispatchEvent(new Event("close"));
    };
  }
  if (!HTMLElement.prototype.scrollIntoView) {
    HTMLElement.prototype.scrollIntoView = function () {};
  }
}

// Rebuilds the fixture in `document.body` and re-fires DOMContentLoaded so
// main.ts's setup code (which only runs on that event) re-queries the fresh
// elements and rewires its listeners against them.
export function mountFixture() {
  polyfillDialog();
  document.body.innerHTML = FIXTURE_HTML;
  document.dispatchEvent(new Event("DOMContentLoaded", { bubbles: true, cancelable: true }));
}
