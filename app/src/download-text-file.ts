// Triggers a browser "Save As" download of `contents` named `filename`, via
// a throwaway Blob URL and a clicked anchor element - the standard
// technique for a framework-free page, and one the desktop app's own
// WebView handles the same way an ordinary browser does, without needing a
// Tauri fs/dialog plugin.
export function downloadTextFile(filename: string, contents: string, mimeType: string) {
  const blob = new Blob([contents], { type: mimeType });
  const url = URL.createObjectURL(blob);
  const anchor = document.createElement("a");
  anchor.href = url;
  anchor.download = filename;
  anchor.click();
  // Download processing can be asynchronous, so the WebView may still need
  // the URL after this task finishes; revoking it only once the event loop
  // is free again avoids racing an in-progress download.
  setTimeout(() => URL.revokeObjectURL(url), 0);
}
