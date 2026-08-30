// A minimal DOM fixture mirroring the elements index.html defines that
// main.ts's DOMContentLoaded handler looks up by id. Kept in one place so
// tests build the same structure main.ts expects to wire up.
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
        <button type="button" id="tab-files" class="tabs__tab" role="tab" aria-selected="false">Files</button>
        <button type="button" id="tab-lint" class="tabs__tab" role="tab" aria-selected="false">Lint results</button>
        <button type="button" id="tab-contact" class="tabs__tab" role="tab" aria-selected="false">Contact</button>
      </div>

      <div id="panel-import" class="tabs__panel" role="tabpanel">
        <div id="drop-zone" class="drop-zone">
          <p id="drop-zone-error" class="drop-zone__error" aria-live="polite"></p>
        </div>
      </div>

      <div id="panel-settings" class="tabs__panel" role="tabpanel" hidden>
        <input id="compiler-path" type="text" />
        <textarea id="script-roots"></textarea>
        <select id="semicolon-style">
          <option value="forbid">Remove where possible</option>
          <option value="require">Add to non-empty lines</option>
        </select>
        <select id="indentation-style">
          <option value="tabs">Tabs</option>
          <option value="spaces">Spaces</option>
        </select>
        <input id="indentation-width" type="number" min="1" max="16" value="4" disabled />
        <select id="type-casing-style">
          <option value="PascalCase">PascalCase</option>
          <option value="camelCase">camelCase</option>
          <option value="lowercase">lowercase</option>
          <option value="UPPERCASE">UPPERCASE</option>
        </select>
        <select id="identifier-casing-style">
          <option value="camelCase">camelCase</option>
          <option value="PascalCase">PascalCase</option>
          <option value="snake_case">snake_case</option>
          <option value="CONSTANT_CASE">CONSTANT_CASE</option>
        </select>
        <select id="named-arguments-style">
          <option value="never">Never</option>
          <option value="instead_of_defaults">Instead of defaults</option>
          <option value="always">Always</option>
        </select>
        <input id="cyclomatic-complexity-warning" type="number" min="1" value="10" />
        <input id="cyclomatic-complexity-error" type="number" min="1" value="20" />
        <input id="min-wait-interval" type="number" min="0" step="0.01" value="0.1" />
        <input type="checkbox" id="fail-on-warning" />
        <input type="checkbox" id="fail-on-info" />
        <fieldset id="lint-rules">
          <input type="checkbox" id="rule-trailing_whitespace" checked />
          <input type="checkbox" id="rule-comma_spacing" checked />
          <input type="checkbox" id="rule-forbidden_functions" checked />
          <input type="checkbox" id="rule-slow_functions" checked />
          <input type="checkbox" id="rule-unused_getter" checked />
          <input type="checkbox" id="rule-unused_property" checked />
          <input type="checkbox" id="rule-semicolon" checked />
          <input type="checkbox" id="rule-float_int_conversion" checked />
          <input type="checkbox" id="rule-strict_boolean" checked />
          <input type="checkbox" id="rule-argument_types" checked />
          <input type="checkbox" id="rule-return_types" checked />
          <input type="checkbox" id="rule-function_override" checked />
          <input type="checkbox" id="rule-argument_naming" checked />
          <input type="checkbox" id="rule-numeric_comparison" checked />
          <input type="checkbox" id="rule-indentation" checked />
          <input type="checkbox" id="rule-cyclomatic_complexity" checked />
          <input type="checkbox" id="rule-unreachable_statement" checked />
          <input type="checkbox" id="rule-static_condition" checked />
          <input type="checkbox" id="rule-division_by_zero" checked />
          <input type="checkbox" id="rule-empty_body" checked />
          <input type="checkbox" id="rule-unused_local_variable" checked />
          <input type="checkbox" id="rule-none_form_usage" checked />
          <input type="checkbox" id="rule-local_variable_shadowing" checked />
          <input type="checkbox" id="rule-chain_whitespace" checked />
          <input type="checkbox" id="rule-exclamation_spacing" checked />
          <input type="checkbox" id="rule-identifier_casing" checked />
          <input type="checkbox" id="rule-type_casing" checked />
          <input type="checkbox" id="rule-named_arguments" checked />
          <input type="checkbox" id="rule-operator_spacing" checked />
          <input type="checkbox" id="rule-property_sorting" />
          <input type="checkbox" id="rule-explicit_return" checked />
          <input type="checkbox" id="rule-unchecked_form_parameter" />
          <input type="checkbox" id="rule-unchecked_cast" checked />
          <input type="checkbox" id="rule-short_wait_interval" checked />
        </fieldset>
      </div>

      <div id="panel-files" class="tabs__panel" role="tabpanel" hidden>
        <div id="achlist-result" class="achlist-result" hidden>
          <h2 id="achlist-result-title"></h2>
          <ul id="achlist-result-list"></ul>
        </div>
      </div>

      <div id="panel-lint" class="tabs__panel" role="tabpanel" hidden>
        <div id="psc-result" class="psc-result" hidden>
          <input id="filename-filter" type="text" />
          <fieldset id="psc-result-filters">
            <input type="checkbox" id="filter-error" checked />
            <input type="checkbox" id="filter-warning" checked />
            <input type="checkbox" id="filter-info" checked />
            <input type="checkbox" id="filter-other" checked />
          </fieldset>
          <ul id="psc-result-list"></ul>
        </div>
      </div>

      <div id="panel-contact" class="tabs__panel" role="tabpanel" hidden>
        <ul class="contact-list">
          <li><a href="https://discord.gg/idrinth">Discord</a></li>
          <li><a href="https://www.nexusmods.com/skyrimspecialedition/mods/189862">NexusMods</a></li>
          <li><a href="https://github.com/idrinth/papyrus-lint">GitHub</a></li>
        </ul>
      </div>
    </div>
  </main>

  <dialog id="code-viewer" class="code-viewer">
    <div class="code-viewer__header">
      <h2 id="code-viewer-title" class="code-viewer__title"></h2>
      <div class="code-viewer__actions">
        <button type="button" id="code-viewer-edit" class="code-viewer__action">Edit</button>
        <button type="button" id="code-viewer-save" class="code-viewer__action" hidden>Save</button>
        <button type="button" id="code-viewer-save-compile" class="code-viewer__action" hidden>Save &amp; Compile</button>
        <button type="button" id="code-viewer-cancel" class="code-viewer__action" hidden>Cancel</button>
      </div>
      <button type="button" id="code-viewer-fullscreen" aria-label="Enter fullscreen" aria-pressed="false">⛶</button>
      <button type="button" id="code-viewer-close" aria-label="Close">&times;</button>
    </div>
    <div id="code-viewer-body" class="code-viewer__body">
      <div class="code-viewer__stage">
        <div id="code-viewer-view" class="code-viewer__view"></div>
        <div id="code-viewer-editor" class="code-viewer__editor" hidden>
          <pre id="code-viewer-editor-highlight" class="code-viewer__editor-highlight" aria-hidden="true"><code></code></pre>
          <textarea id="code-viewer-editor-textarea" class="code-viewer__editor-textarea" spellcheck="false"></textarea>
          <ul id="code-viewer-autocomplete" class="code-viewer__autocomplete" role="listbox" hidden></ul>
        </div>
      </div>
      <pre id="code-viewer-compile-output" class="code-viewer__compile-output" hidden></pre>
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
