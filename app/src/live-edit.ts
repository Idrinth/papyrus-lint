import {
  type CompletionQuery,
  type IdentifierAt,
  type Member,
  completionInsertText,
  completionLabel,
  completionQueryAt,
  declarationDocumentationOnLine,
  declaredTypes,
  documentationForIdentifier,
  filterMembers,
  identifierAt,
  memberDocumentation,
  overlayLocalDocumentation,
} from "./autocomplete";
import { highlightPapyrusLines } from "./highlight";
import {
  codeViewerAutocompleteEl,
  codeViewerCancelButtonEl,
  codeViewerCompileOutputEl,
  codeViewerDiffOutputEl,
  codeViewerEditButtonEl,
  codeViewerEditGutterEl,
  codeViewerEditHighlightEl,
  codeViewerEditTextareaEl,
  codeViewerMode,
  codeViewerSaveButtonEl,
  codeViewerSaveCompileButtonEl,
  codeViewerState,
  compileAndShowOutput,
  findingsGroupedByLine,
  hideCompileOutput,
  hideDiffOutput,
  lineSeverityOf,
  renderCodeViewerView,
  setCodeViewerMode,
  setCodeViewerState,
} from "./code-viewer";
import { renderPscResults } from "./results-list";
import {
  type Diagnostic,
  currentPscOutcomes,
  lintPapyrusScript,
  lintPscFile,
  listScriptMembers,
  writePscFile,
} from "./main";

let autocompleteQuery: CompletionQuery | null = null;
let autocompleteMembers: Member[] = [];
let autocompleteSelectedIndex = 0;
let autocompleteRequestId = 0;
const membersByType = new Map<string, Member[]>();
let hoverRequestId = 0;

let codeViewerEditFindingsByLine: Map<number, Diagnostic[]> = new Map();
let codeViewerEditLiveFindings: Diagnostic[] = [];
let codeViewerLiveLintTimer: ReturnType<typeof window.setTimeout> | null = null;
let codeViewerLiveLintRequestId = 0;
const LIVE_EDIT_LINT_DEBOUNCE_MS = 400;
let codeViewerEditLastMouseY: number | null = null;
let codeViewerEditLastMouseX: number | null = null;

// Cancels any pending or in-flight live lint, e.g. when leaving edit mode
// (see `setCodeViewerMode`) - linting a textarea that's no longer being
// edited would be pointless, and a response arriving after that point must
// not overwrite whatever's shown next. Exported so tests can reset this
// module-level timer between runs the same way `resetConfirmedProjectDirs`
// resets other such state.
export function cancelLiveEditLint(): void {
  if (codeViewerLiveLintTimer !== null) {
    window.clearTimeout(codeViewerLiveLintTimer);
    codeViewerLiveLintTimer = null;
  }
  codeViewerLiveLintRequestId += 1;
  hoverRequestId += 1;
  membersByType.clear();
}

// Debounces a live lint of the edit-mode textarea's current contents via
// `lintPapyrusScript`, so a burst of keystrokes triggers one CLI-equivalent
// lint pass `LIVE_EDIT_LINT_DEBOUNCE_MS` after the last of them rather than
// one per keystroke. Called from the textarea's own "input" listener.
function scheduleLiveEditLint(): void {
  if (codeViewerMode !== "edit") {
    return;
  }
  if (codeViewerLiveLintTimer !== null) {
    window.clearTimeout(codeViewerLiveLintTimer);
  }
  codeViewerLiveLintTimer = window.setTimeout(() => {
    codeViewerLiveLintTimer = null;
    void runLiveEditLint();
  }, LIVE_EDIT_LINT_DEBOUNCE_MS);
}

async function runLiveEditLint(): Promise<void> {
  if (!codeViewerEditTextareaEl || codeViewerMode !== "edit") {
    return;
  }
  const requestId = ++codeViewerLiveLintRequestId;
  const findings = await lintPapyrusScript(codeViewerEditTextareaEl.value);
  // A later keystroke may have started a new live lint (or left edit mode
  // entirely) while this one was in flight; don't clobber it with a stale
  // response.
  if (requestId !== codeViewerLiveLintRequestId || !codeViewerEditTextareaEl || codeViewerMode !== "edit") {
    return;
  }
  codeViewerEditLiveFindings = findings;
  updateCodeViewerEditHighlight();
}

// Re-renders the edit mode's syntax-highlighted overlay and line-number
// gutter from the textarea's current value, keeping both in sync as the
// user types.
function updateCodeViewerEditHighlight() {
  const code = codeViewerEditHighlightEl?.querySelector("code");
  if (!code || !codeViewerEditTextareaEl) {
    return;
  }
  const findings = codeViewerEditLiveFindings;
  const findingsByLine = findingsGroupedByLine(findings);
  codeViewerEditFindingsByLine = findingsByLine;
  const highlightedLines = highlightPapyrusLines(codeViewerEditTextareaEl.value);
  code.innerHTML = highlightedLines
    .map((lineHtml, index) => {
      const lineFindings = findingsByLine.get(index + 1);
      const severity = lineSeverityOf(lineFindings);
      const lineClass = severity
        ? `code-viewer__editor-line code-viewer__line--${severity}`
        : "code-viewer__editor-line";
      return `<span class="${lineClass}">${lineHtml}</span>`;
    })
    // Each line is already `display: block` (see styles.css), so it needs no
    // separator to end up on its own line; joining with "\n" here used to add
    // a literal newline character between spans that, because the highlight
    // layer is `white-space: pre`, rendered as its own extra blank line on
    // top of each blank line's own (empty, so zero-height) span - doubling
    // up and misaligning the overlay against the textarea underneath it.
    .join("");

  if (codeViewerEditGutterEl) {
    codeViewerEditGutterEl.innerHTML = highlightedLines
      .map((_, index) => `<span class="code-viewer__editor-gutter-line">${index + 1}</span>`)
      .join("");
  }

  // The textarea sits above the non-interactive highlighting layer, so its
  // per-line colours are the only ones actually visible; its own `title`
  // (see `updateCodeViewerEditTooltip`) is instead kept in sync with
  // whichever line the mouse currently hovers, the same as the read-only
  // view's per-row tooltips. The accessible description still summarizes
  // every finding at once, since a screen-reader user has no equivalent of
  // "hovering a line" to reveal them one at a time.
  const diagnosticSummary = findings
    .map((finding) => `Line ${finding.line}, column ${finding.column}: ${finding.message}`)
    .join("\n");
  codeViewerEditTextareaEl.setAttribute(
    "aria-label",
    diagnosticSummary ? `Papyrus source editor. Linter findings:\n${diagnosticSummary}` : "Papyrus source editor. No linter findings.",
  );
}

// Keeps the textarea's tooltip matched to the source line (and, when the
// pointer's column can be resolved, the identifier) under the mouse at
// (`clientX`, `clientY`), since the textarea sits on top of (and intercepts
// every pointer event meant for) the highlighted overlay beneath it - whose
// own per-line findings would otherwise never actually be hoverable.
//
// A `{ ... }` documentation comment on a ScriptName / Property / Function /
// Event declaration is shown when hovering that declaration or a use of it;
// lint findings for the line still appear below the doc, so neither tooltip
// hides the other. Remembered so a scroll event (mouse wheel, keyboard
// navigation) can re-evaluate the tooltip against the pointer's last known
// position even though the pointer itself didn't move - otherwise scrolling
// a new line under a stationary mouse would leave the previous line's
// tooltip showing.
function updateCodeViewerEditTooltip(clientX: number, clientY: number) {
  if (!codeViewerEditTextareaEl) {
    return;
  }
  const computed = window.getComputedStyle(codeViewerEditTextareaEl);
  const lineHeight = parseFloat(computed.lineHeight);
  const paddingTop = parseFloat(computed.paddingTop);
  const rect = codeViewerEditTextareaEl.getBoundingClientRect();
  const offsetY = clientY - rect.top + codeViewerEditTextareaEl.scrollTop - paddingTop;
  const line = Math.floor(offsetY / lineHeight) + 1;
  const lineFindings = codeViewerEditFindingsByLine.get(line);
  const findingsTitle = lineFindings ? lineFindings.map((finding) => finding.message).join("\n") : "";
  const source = codeViewerEditTextareaEl.value;
  const index = sourceIndexFromPoint(codeViewerEditTextareaEl, computed, clientX, clientY);
  const ident = index !== null ? identifierAt(source, index) : null;
  const localDoc = ident
    ? documentationForIdentifier(source, ident, (typeName) => membersByType.get(typeName.toLowerCase()))
    : declarationDocumentationOnLine(source, line);
  codeViewerEditTextareaEl.title = joinTooltip(localDoc, findingsTitle);

  hoverRequestId += 1;
  if (ident?.receiver) {
    void fillMemberDocumentationTooltip(ident, source, findingsTitle, hoverRequestId);
  }
}

function joinTooltip(doc: string | null, findingsTitle: string): string {
  return [doc, findingsTitle].filter((part): part is string => typeof part === "string" && part.length > 0).join("\n\n");
}

async function fillMemberDocumentationTooltip(
  ident: IdentifierAt,
  source: string,
  findingsTitle: string,
  requestId: number,
) {
  if (!ident.receiver) {
    return;
  }
  const receiverType = declaredTypes(source).get(ident.receiver.toLowerCase());
  if (!receiverType) {
    return;
  }
  const members = await cachedMembersForType(receiverType);
  if (requestId !== hoverRequestId || !codeViewerEditTextareaEl) {
    return;
  }
  const doc = documentationForIdentifier(source, ident, (typeName) =>
    typeName.toLowerCase() === receiverType.toLowerCase() ? members : membersByType.get(typeName.toLowerCase()),
  );
  codeViewerEditTextareaEl.title = joinTooltip(doc, findingsTitle);
}

async function cachedMembersForType(typeName: string): Promise<Member[]> {
  const key = typeName.toLowerCase();
  const cached = membersByType.get(key);
  if (cached) {
    return cached;
  }
  const members = await listScriptMembers(typeName);
  membersByType.set(key, members);
  return members;
}

// Converts a pointer position over the textarea into a source index, using
// the same line-height math as the per-line tooltip plus a measured
// monospace character width for the column. Returns null when the pointer
// isn't over a source line, or when character width can't be measured
// (jsdom tests that don't mock layout).
function sourceIndexFromPoint(
  textarea: HTMLTextAreaElement,
  computed: CSSStyleDeclaration,
  clientX: number,
  clientY: number,
): number | null {
  const lineHeight = parseFloat(computed.lineHeight);
  const paddingTop = parseFloat(computed.paddingTop) || 0;
  const paddingLeft = parseFloat(computed.paddingLeft) || 0;
  if (!Number.isFinite(lineHeight) || lineHeight <= 0) {
    return null;
  }
  const rect = textarea.getBoundingClientRect();
  const offsetY = clientY - rect.top + textarea.scrollTop - paddingTop;
  const offsetX = clientX - rect.left + textarea.scrollLeft - paddingLeft;
  if (offsetY < 0 || offsetX < 0) {
    return null;
  }
  const line = Math.floor(offsetY / lineHeight);
  const lines = textarea.value.split("\n");
  if (line < 0 || line >= lines.length) {
    return null;
  }
  const charWidth = measureEditorCharWidth(computed);
  if (charWidth <= 0) {
    return null;
  }
  const visualCol = Math.floor(offsetX / charWidth);
  const lineText = lines[line]!;
  const tabSize = 8;
  let visual = 0;
  let column = 0;
  while (column < lineText.length && visual < visualCol) {
    if (lineText[column] === "\t") {
      visual += tabSize - (visual % tabSize);
    } else {
      visual += 1;
    }
    column += 1;
  }
  let index = 0;
  for (let i = 0; i < line; i++) {
    index += lines[i]!.length + 1;
  }
  return index + column;
}

function measureEditorCharWidth(computed: CSSStyleDeclaration): number {
  const fontSize = parseFloat(computed.fontSize);
  if (!Number.isFinite(fontSize) || fontSize <= 0) {
    return 0;
  }
  const probe = document.createElement("span");
  probe.style.position = "absolute";
  probe.style.visibility = "hidden";
  probe.style.whiteSpace = "pre";
  probe.style.fontFamily = computed.fontFamily;
  probe.style.fontSize = computed.fontSize;
  probe.style.fontWeight = computed.fontWeight;
  probe.style.letterSpacing = computed.letterSpacing;
  probe.textContent = "0000000000";
  document.body.append(probe);
  const width = probe.getBoundingClientRect().width / 10;
  probe.remove();
  return Number.isFinite(width) ? width : 0;
}

// Measures where the text caret currently renders inside `textarea`, using
// a hidden, identically-styled mirror element (the standard technique for
// this - a real caret rectangle isn't exposed by the DOM). Coordinates are
// relative to the textarea's own box, matching where the autocompletion
// dropdown (its sibling, absolutely positioned within the same container)
// should be placed.
function caretPixelPosition(textarea: HTMLTextAreaElement): { top: number; left: number } {
  const computed = window.getComputedStyle(textarea);
  const mirror = document.createElement("div");
  mirror.style.position = "absolute";
  mirror.style.visibility = "hidden";
  mirror.style.top = "0";
  mirror.style.left = "-9999px";
  mirror.style.whiteSpace = "pre-wrap";
  mirror.style.wordBreak = "break-word";
  mirror.style.boxSizing = computed.boxSizing;
  mirror.style.width = computed.width;
  mirror.style.padding = computed.padding;
  mirror.style.border = `${computed.borderWidth} solid transparent`;
  mirror.style.fontFamily = computed.fontFamily;
  mirror.style.fontSize = computed.fontSize;
  mirror.style.fontWeight = computed.fontWeight;
  mirror.style.lineHeight = computed.lineHeight;
  mirror.style.letterSpacing = computed.letterSpacing;

  const caretIndex = textarea.selectionStart;
  const marker = document.createElement("span");
  marker.textContent = "​";
  mirror.append(textarea.value.slice(0, caretIndex), marker, textarea.value.slice(caretIndex) || " ");

  document.body.append(mirror);
  const top = marker.offsetTop - textarea.scrollTop + marker.offsetHeight;
  const left = marker.offsetLeft - textarea.scrollLeft;
  mirror.remove();

  return { top, left };
}

// Positions the autocompletion dropdown just below the text caret.
function positionAutocomplete() {
  if (!codeViewerAutocompleteEl || !codeViewerEditTextareaEl) {
    return;
  }
  const { top, left } = caretPixelPosition(codeViewerEditTextareaEl);
  codeViewerAutocompleteEl.style.top = `${top}px`;
  codeViewerAutocompleteEl.style.left = `${left}px`;
}

// Hides the autocompletion dropdown and clears its pending query/results.
export function hideAutocomplete() {
  // Invalidate a lookup that may still be awaiting the backend. Otherwise an
  // Escape press (or a cursor move away from member access) can hide the
  // dropdown only for the stale response to display it again.
  autocompleteRequestId += 1;
  autocompleteQuery = null;
  autocompleteMembers = [];
  autocompleteSelectedIndex = 0;
  if (codeViewerAutocompleteEl) {
    codeViewerAutocompleteEl.hidden = true;
    codeViewerAutocompleteEl.replaceChildren();
  }
}

// Renders `autocompleteMembers` into the dropdown (with the currently
// selected one highlighted), or hides it if there are none.
function renderAutocomplete() {
  if (!codeViewerAutocompleteEl) {
    return;
  }
  if (autocompleteMembers.length === 0) {
    codeViewerAutocompleteEl.hidden = true;
    codeViewerAutocompleteEl.replaceChildren();
    return;
  }

  codeViewerAutocompleteEl.replaceChildren(
    ...autocompleteMembers.map((member, index) => {
      const item = document.createElement("li");
      item.setAttribute("role", "option");
      item.classList.add("code-viewer__autocomplete-item");
      item.classList.toggle("code-viewer__autocomplete-item--active", index === autocompleteSelectedIndex);
      const label = document.createElement("span");
      label.classList.add("code-viewer__autocomplete-item-label");
      label.textContent = completionLabel(member);
      item.append(label);
      if (index === autocompleteSelectedIndex) {
        const doc = memberDocumentation(member);
        if (doc) {
          const docEl = document.createElement("span");
          docEl.classList.add("code-viewer__autocomplete-item-doc");
          docEl.textContent = doc;
          item.append(docEl);
        }
      }
      // mousedown (not click), and prevented from moving focus, so
      // accepting a completion by clicking it doesn't blur the textarea
      // first (which would otherwise close the dropdown before the click
      // that's meant to use it).
      item.addEventListener("mousedown", (event) => {
        event.preventDefault();
        applyAutocompleteSelection(index);
      });
      return item;
    }),
  );
  codeViewerAutocompleteEl.hidden = false;
  positionAutocomplete();
}

// Re-evaluates the autocompletion query at the textarea's current cursor
// position, fetching and showing matching members if the cursor is right
// after a "receiver.prefix" whose receiver's declared type is known.
// Hides the dropdown otherwise (including while a range is selected).
export async function updateAutocomplete() {
  if (!codeViewerEditTextareaEl || codeViewerMode !== "edit") {
    hideAutocomplete();
    return;
  }
  const textarea = codeViewerEditTextareaEl;
  if (textarea.selectionStart !== textarea.selectionEnd) {
    hideAutocomplete();
    return;
  }

  const query = completionQueryAt(textarea.value, textarea.selectionStart);
  if (!query) {
    hideAutocomplete();
    return;
  }

  const requestId = ++autocompleteRequestId;
  const members = overlayLocalDocumentation(
    filterMembers(await cachedMembersForType(query.receiverType), query.prefix),
    textarea.value,
    query.receiverType,
  );
  // A later keystroke may have started a new request (or left edit mode)
  // while this one was in flight; don't clobber it with a stale response.
  if (requestId !== autocompleteRequestId || !codeViewerEditTextareaEl || codeViewerMode !== "edit") {
    return;
  }

  autocompleteQuery = query;
  autocompleteMembers = members;
  autocompleteSelectedIndex = 0;
  renderAutocomplete();
}

// Splices the selected member's insertion text into the textarea in place
// of the typed prefix, then closes the dropdown.
export function applyAutocompleteSelection(index: number) {
  const member = autocompleteMembers[index];
  if (!codeViewerEditTextareaEl || !autocompleteQuery || !member) {
    return;
  }
  const textarea = codeViewerEditTextareaEl;
  const { prefixStart } = autocompleteQuery;
  textarea.setRangeText(completionInsertText(member), prefixStart, textarea.selectionStart, "end");
  hideAutocomplete();
  updateCodeViewerEditHighlight();
  textarea.focus();
}

// Handles the dropdown's navigation/acceptance/dismissal keys while it's
// open; every other key is left for the textarea to handle normally.
export function handleAutocompleteKeydown(event: KeyboardEvent) {
  if (autocompleteMembers.length === 0) {
    return;
  }
  if (event.key === "ArrowDown") {
    event.preventDefault();
    autocompleteSelectedIndex = (autocompleteSelectedIndex + 1) % autocompleteMembers.length;
    renderAutocomplete();
  } else if (event.key === "ArrowUp") {
    event.preventDefault();
    autocompleteSelectedIndex = (autocompleteSelectedIndex - 1 + autocompleteMembers.length) % autocompleteMembers.length;
    renderAutocomplete();
  } else if (event.key === "Enter" || event.key === "Tab") {
    event.preventDefault();
    applyAutocompleteSelection(autocompleteSelectedIndex);
  } else if (event.key === "Escape") {
    event.preventDefault();
    hideAutocomplete();
  }
}

// A plain textarea's default Tab handling moves focus to the next control
// instead of inserting a character, so Papyrus source (conventionally
// tab-indented) couldn't be indented by hand at all. Runs after
// handleAutocompleteKeydown, whose own Tab handling (accepting the
// highlighted completion) already calls preventDefault() when the
// dropdown is open, so this only inserts a literal tab when Tab reaches
// the textarea uncaptured.
export function handleEditorTabKeydown(event: KeyboardEvent) {
  if (event.key !== "Tab" || event.defaultPrevented || !codeViewerEditTextareaEl) {
    return;
  }
  event.preventDefault();
  const textarea = codeViewerEditTextareaEl;
  textarea.setRangeText("\t", textarea.selectionStart, textarea.selectionEnd, "end");
  updateCodeViewerEditHighlight();
}

// A textarea's `value` getter always normalizes CR/CRLF line breaks to LF
// (per the HTML spec's "API value" transform), even though its `value`
// setter stores whatever was assigned verbatim. A CRLF-saved .psc file's
// `source` therefore no longer matches the textarea's own value right
// after `enterCodeViewerEditMode` sets it, with no edit having happened;
// normalizing both sides before comparing keeps that from reading as dirty.
function normalizeLineEndings(text: string): string {
  return text.replace(/\r\n?/g, "\n");
}

export function isCodeViewerEditDirty(): boolean {
  return (
    codeViewerMode === "edit" &&
    codeViewerState !== null &&
    codeViewerEditTextareaEl !== null &&
    codeViewerEditTextareaEl.value !== normalizeLineEndings(codeViewerState.source)
  );
}

export function enterCodeViewerEditMode() {
  if (!codeViewerState || !codeViewerEditTextareaEl) {
    return;
  }
  cancelLiveEditLint();
  codeViewerEditLiveFindings = codeViewerState.findings;
  codeViewerEditTextareaEl.value = codeViewerState.source;
  updateCodeViewerEditHighlight();
  hideCompileOutput(codeViewerCompileOutputEl);
  hideDiffOutput(codeViewerDiffOutputEl);
  setCodeViewerMode("edit");
  codeViewerEditTextareaEl.focus();
}

export function cancelCodeViewerEditMode() {
  if (isCodeViewerEditDirty() && !window.confirm("Discard unsaved changes?")) {
    return;
  }
  setCodeViewerMode("view");
}

// Writes the editor's current contents to disk, re-lints the file, and
// refreshes both the code viewer's view mode and the Lint results list to
// match, switching the viewer back to view mode. Shared by the plain Save
// button and the Save & Compile button below; throws (without touching any
// UI) if the write itself fails, leaving the caller to report that.
async function persistCodeViewerEdits(): Promise<void> {
  if (!codeViewerState || !codeViewerEditTextareaEl) {
    return;
  }
  const { path } = codeViewerState;
  const contents = codeViewerEditTextareaEl.value;

  await writePscFile(path, contents);
  const findings = await lintPscFile(path);
  setCodeViewerState({ path, source: contents, findings });

  const outcome = currentPscOutcomes.find((candidate) => candidate.path === path);
  if (outcome) {
    outcome.findings = findings;
    renderPscResults(currentPscOutcomes);
  }

  renderCodeViewerView(codeViewerState.source, codeViewerState.findings);
  setCodeViewerMode("view");
}

export async function saveCodeViewerEdits() {
  if (!codeViewerState || !codeViewerEditTextareaEl || !codeViewerSaveButtonEl) {
    return;
  }
  codeViewerSaveButtonEl.disabled = true;
  try {
    await persistCodeViewerEdits();
  } catch (error) {
    console.error(error);
    const originalLabel = codeViewerSaveButtonEl.textContent;
    codeViewerSaveButtonEl.textContent = "Save failed";
    window.setTimeout(() => {
      if (codeViewerSaveButtonEl) {
        codeViewerSaveButtonEl.textContent = originalLabel;
      }
    }, 2000);
  } finally {
    codeViewerSaveButtonEl.disabled = false;
  }
}

// Saves the editor's contents (as saveCodeViewerEdits does) and, if that
// succeeds, immediately compiles the saved file, showing the compiler's
// output beneath the code viewer's view/editor area.
export async function saveAndCompileCodeViewerEdits() {
  if (!codeViewerState || !codeViewerEditTextareaEl || !codeViewerSaveCompileButtonEl) {
    return;
  }
  const { path } = codeViewerState;
  const originalLabel = codeViewerSaveCompileButtonEl.textContent;

  codeViewerSaveCompileButtonEl.disabled = true;
  try {
    await persistCodeViewerEdits();
  } catch (error) {
    console.error(error);
    codeViewerSaveCompileButtonEl.textContent = "Save failed";
    window.setTimeout(() => {
      if (codeViewerSaveCompileButtonEl) {
        codeViewerSaveCompileButtonEl.textContent = originalLabel;
      }
    }, 2000);
    codeViewerSaveCompileButtonEl.disabled = false;
    return;
  }

  if (codeViewerCompileOutputEl) {
    codeViewerSaveCompileButtonEl.textContent = "Compiling…";
    await compileAndShowOutput(path, codeViewerCompileOutputEl);
  }
  codeViewerSaveCompileButtonEl.disabled = false;
  codeViewerSaveCompileButtonEl.textContent = originalLabel;
}

export function bindLiveEdit() {
  codeViewerEditButtonEl?.addEventListener("click", () => enterCodeViewerEditMode());
  codeViewerCancelButtonEl?.addEventListener("click", () => cancelCodeViewerEditMode());
  codeViewerSaveButtonEl?.addEventListener("click", () => void saveCodeViewerEdits());
  codeViewerSaveCompileButtonEl?.addEventListener("click", () => void saveAndCompileCodeViewerEdits());
  codeViewerEditTextareaEl?.addEventListener("input", () => updateCodeViewerEditHighlight());
  codeViewerEditTextareaEl?.addEventListener("input", () => void updateAutocomplete());
  codeViewerEditTextareaEl?.addEventListener("input", () => scheduleLiveEditLint());
  codeViewerEditTextareaEl?.addEventListener("click", () => void updateAutocomplete());
  codeViewerEditTextareaEl?.addEventListener("keydown", (event) => handleAutocompleteKeydown(event));
  codeViewerEditTextareaEl?.addEventListener("keydown", (event) => handleEditorTabKeydown(event));
  codeViewerEditTextareaEl?.addEventListener("blur", () => hideAutocomplete());
  codeViewerEditTextareaEl?.addEventListener("mousemove", (event) => {
    codeViewerEditLastMouseX = event.clientX;
    codeViewerEditLastMouseY = event.clientY;
    updateCodeViewerEditTooltip(event.clientX, event.clientY);
  });
  codeViewerEditTextareaEl?.addEventListener("mouseleave", () => {
    codeViewerEditLastMouseX = null;
    codeViewerEditLastMouseY = null;
    hoverRequestId += 1;
    if (codeViewerEditTextareaEl) {
      codeViewerEditTextareaEl.title = "";
    }
  });
  codeViewerEditTextareaEl?.addEventListener("scroll", () => {
    if (codeViewerEditHighlightEl && codeViewerEditTextareaEl) {
      codeViewerEditHighlightEl.scrollTop = codeViewerEditTextareaEl.scrollTop;
      codeViewerEditHighlightEl.scrollLeft = codeViewerEditTextareaEl.scrollLeft;
    }
    if (codeViewerEditGutterEl && codeViewerEditTextareaEl) {
      codeViewerEditGutterEl.scrollTop = codeViewerEditTextareaEl.scrollTop;
    }
    if (codeViewerEditLastMouseY !== null && codeViewerEditLastMouseX !== null) {
      updateCodeViewerEditTooltip(codeViewerEditLastMouseX, codeViewerEditLastMouseY);
    }
  });
}
