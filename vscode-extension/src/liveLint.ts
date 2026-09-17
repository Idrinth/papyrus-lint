import * as vscode from 'vscode';
import { liveLintDebounceMs, liveLintEnabled } from './config';
import { isPapyrusDocument } from './documents';
import type { PapyrusLinter } from './linter';

/** Per-document debounce timers backing `scheduleLiveLint`, keyed by the document
 * uri's string form. Reset on each activation so tests (and a re-activated host)
 * never inherit a previous document's pending run. */
const liveLintTimers = new Map<string, ReturnType<typeof setTimeout>>();

export function resetLiveLint(): void {
  for (const timer of liveLintTimers.values()) {
    clearTimeout(timer);
  }
  liveLintTimers.clear();
}

/** Cancels a document's pending live lint, if any (e.g. on close, where linting a
 * document that's gone would be pointless and `document.getText()` may throw). */
export function cancelLiveLint(document: vscode.TextDocument): void {
  const key = document.uri.toString();
  const timer = liveLintTimers.get(key);
  if (timer !== undefined) {
    clearTimeout(timer);
    liveLintTimers.delete(key);
  }
}

/** Debounces `linter.lintBlob(document)` so a burst of keystrokes triggers one CLI
 * run `liveLintDebounceMs()` after the last of them, not one per keystroke. */
export function scheduleLiveLint(linter: PapyrusLinter, document: vscode.TextDocument): void {
  if (!isPapyrusDocument(document) || !liveLintEnabled()) {
    return;
  }
  cancelLiveLint(document);
  const key = document.uri.toString();
  liveLintTimers.set(
    key,
    setTimeout(() => {
      liveLintTimers.delete(key);
      void linter.lintBlob(document);
    }, liveLintDebounceMs()),
  );
}
