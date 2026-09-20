import * as vscode from 'vscode';
import { liveLintDebounceMs, liveLintEnabled } from './config';
import { isPapyrusDocument } from './documents';
import type { PapyrusLinter } from './linter';

/** Per-document debounce timers and request generations backing `scheduleLiveLint`,
 * keyed by the document uri's string form. Reset on each activation so tests (and
 * a re-activated host) never inherit a previous document's pending or in-flight run. */
const liveLintTimers = new Map<string, ReturnType<typeof setTimeout>>();
const liveLintRequests = new Map<string, number>();
let nextLiveLintRequest = 0;

export function resetLiveLint(): void {
  for (const timer of liveLintTimers.values()) {
    clearTimeout(timer);
  }
  liveLintTimers.clear();
  liveLintRequests.clear();
}

/** Cancels a document's pending live lint and invalidates any in-flight result
 * (e.g. on close, where publishing diagnostics for a document that's gone would
 * recreate diagnostics that the close handler just cleared). */
export function cancelLiveLint(document: vscode.TextDocument): void {
  const key = document.uri.toString();
  liveLintRequests.set(key, ++nextLiveLintRequest);
  const timer = liveLintTimers.get(key);
  if (timer !== undefined) {
    clearTimeout(timer);
    liveLintTimers.delete(key);
  }
}

/** Debounces `linter.lintBlob(document)` so a burst of keystrokes triggers one CLI
 * run `liveLintDebounceMs()` after the last of them, not one per keystroke. */
export function scheduleLiveLint(linter: PapyrusLinter, document: vscode.TextDocument): void {
  if (!isPapyrusDocument(document) || !liveLintEnabled(document.uri)) {
    return;
  }
  cancelLiveLint(document);
  const key = document.uri.toString();
  const request = liveLintRequests.get(key);
  liveLintTimers.set(
    key,
    setTimeout(() => {
      liveLintTimers.delete(key);
      void linter.lintBlob(document, () => liveLintRequests.get(key) === request);
    }, liveLintDebounceMs(document.uri)),
  );
}
