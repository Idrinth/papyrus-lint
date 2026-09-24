import type { IdentifierAt } from "./autocomplete-types";
import { declarationDocumentationOnLine, documentationForIdentifier } from "./member-documentation";
import { declaredTypes, identifierAt } from "./papyrus-source";
import { codeViewerAutocompleteEl, codeViewerEditTextareaEl } from "./code-viewer-state";
import { findingsForEditorLine } from "./live-edit-highlight";
import { cachedMembersForType, getCachedMembers } from "./live-edit-members";
let hoverRequestId = 0;
let codeViewerEditLastMouseY: number | null = null;
let codeViewerEditLastMouseX: number | null = null;

// Invalidates any in-flight hover documentation lookup, e.g. when the
// pointer leaves the editor or live linting is cancelled, so a response
// that resolves afterwards can't overwrite a tooltip it no longer applies
// to.
export function invalidateHover(): void {
  hoverRequestId += 1;
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
    typeName.toLowerCase() === receiverType.toLowerCase() ? members : getCachedMembers(typeName),
  );
  codeViewerEditTextareaEl.title = joinTooltip(doc, findingsTitle);
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
export function positionAutocomplete() {
  if (!codeViewerAutocompleteEl || !codeViewerEditTextareaEl) {
    return;
  }
  const { top, left } = caretPixelPosition(codeViewerEditTextareaEl);
  codeViewerAutocompleteEl.style.top = `${top}px`;
  codeViewerAutocompleteEl.style.left = `${left}px`;
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
// hides the other.
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
  const lineFindings = findingsForEditorLine(line);
  const findingsTitle = lineFindings ? lineFindings.map((finding) => finding.message).join("\n") : "";
  const source = codeViewerEditTextareaEl.value;
  const index = sourceIndexFromPoint(codeViewerEditTextareaEl, computed, clientX, clientY);
  const ident = index !== null ? identifierAt(source, index) : null;
  const localDoc = ident
    ? documentationForIdentifier(source, ident, (typeName) => getCachedMembers(typeName))
    : declarationDocumentationOnLine(source, line);
  codeViewerEditTextareaEl.title = joinTooltip(localDoc, findingsTitle);

  hoverRequestId += 1;
  if (ident?.receiver) {
    void fillMemberDocumentationTooltip(ident, source, findingsTitle, hoverRequestId);
  }
}

// Remembers the pointer's last position over the editor (mousemove) and
// re-evaluates its tooltip against that position - used directly on
// mousemove, and again on scroll (see `refreshPointerTooltip`) so a
// stationary pointer over a newly scrolled-in line isn't left describing
// the line that used to be under it.
export function recordPointerPosition(clientX: number, clientY: number): void {
  codeViewerEditLastMouseX = clientX;
  codeViewerEditLastMouseY = clientY;
  updateCodeViewerEditTooltip(clientX, clientY);
}

// Clears the tooltip and the remembered pointer position once the pointer
// leaves the editor, and invalidates any in-flight hover lookup so a late
// response can't repopulate it.
export function clearPointerPosition(): void {
  codeViewerEditLastMouseX = null;
  codeViewerEditLastMouseY = null;
  invalidateHover();
  if (codeViewerEditTextareaEl) {
    codeViewerEditTextareaEl.title = "";
  }
}

// Re-evaluates the tooltip against the pointer's last known position -
// called on scroll, since the pointer itself didn't move but the line
// underneath it may have.
export function refreshPointerTooltip(): void {
  if (codeViewerEditLastMouseY !== null && codeViewerEditLastMouseX !== null) {
    updateCodeViewerEditTooltip(codeViewerEditLastMouseX, codeViewerEditLastMouseY);
  }
}
