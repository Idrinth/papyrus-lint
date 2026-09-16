import { vi } from "vitest";

// Shared Tauri spies. Each `*.test.ts` that loads `main.ts` re-declares the
// `vi.mock` factories so they close over these same instances (Vitest hoists
// `vi.mock` per test file; putting the spies here avoids the temporal-dead-
// zone that a same-file `const invokeMock = vi.fn()` hits once the mock
// factory runs).
export const invokeMock = vi.fn();
export const onDragDropEventMock = vi.fn();
