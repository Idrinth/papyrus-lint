import {
  completionInsertText,
  completionLabel,
  completionQueryAt,
  filterMembers,
  memberDocumentation,
  overlayLocalDocumentation,
  type CompletionQuery,
  type Member,
} from "./autocomplete";
import { codeViewerAutocompleteEl, codeViewerEditTextareaEl, codeViewerMode } from "./code-viewer";
import { updateCodeViewerEditHighlight } from "./live-edit-highlight";
import { cachedMembersForType } from "./live-edit-members";
import { positionAutocomplete } from "./live-edit-pointer";

let autocompleteQuery: CompletionQuery | null = null;
let autocompleteMembers: Member[] = [];
let autocompleteSelectedIndex = 0;
let autocompleteRequestId = 0;

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
