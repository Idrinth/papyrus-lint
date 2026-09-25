// Suppresses the WebView's native context menu except on text fields.
// Reload/Update in that menu remounts the frontend without the Tauri
// startup path in main.ts and can leave the desktop app unusable.

export function shouldAllowNativeContextMenu(target: EventTarget | null): boolean {
  if (!(target instanceof Node)) {
    return false;
  }
  const element = target instanceof Element ? target : target.parentElement;
  if (!element) {
    return false;
  }
  return element.closest("input, textarea, [contenteditable]:not([contenteditable='false'])") !== null;
}

function onContextMenu(event: Event) {
  if (shouldAllowNativeContextMenu(event.target)) {
    return;
  }
  event.preventDefault();
}

export function bindContextMenu() {
  window.addEventListener("contextmenu", onContextMenu);
}
