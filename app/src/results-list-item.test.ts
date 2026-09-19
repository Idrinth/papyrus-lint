import { vi } from "vitest";
import { invokeMock, onDragDropEventMock, showWindowMock } from "./test/mocks";
vi.mock("@tauri-apps/api/core", () => ({ invoke: (...args: unknown[]) => invokeMock(...args), isTauri: () => true }));
vi.mock("@tauri-apps/api/webview", () => ({ getCurrentWebview: () => ({ onDragDropEvent: onDragDropEventMock }) }));
vi.mock("@tauri-apps/api/window", () => ({ getCurrentWindow: () => ({ show: showWindowMock }) }));

import { describe, expect, it } from "vitest";
import { invokeImplFor } from "./test/harness";
import { applyRuleTags } from "./main";
import { type PscParseOutcome } from "./backend";
import { DEFAULT_LINT_CONFIG } from "./config-types";
import { useProjectDir } from "./project-settings";
import { buildPscResultItem } from "./results-list-item";

describe("buildPscResultItem", () => {
  function outcome(overrides: Partial<PscParseOutcome> = {}): PscParseOutcome {
    return { path: "/a.psc", ok: true, detail: 'parsed as "A"', findings: [], ...overrides };
  }

  it("skips a clean, successfully parsed file", () => {
    expect(buildPscResultItem(outcome())).toBeNull();
  });

  it("renders the findings it is given, without applying filters itself", () => {
    document.querySelector<HTMLInputElement>("#filter-error")!.checked = false;
    document.querySelector<HTMLInputElement>("#filter-error")!.dispatchEvent(new Event("change"));

    try {
      const item = buildPscResultItem(
        outcome({ findings: [{ line: 1, column: 1, message: "[error] bad" }] }),
      );
      expect(item!.querySelectorAll(".psc-result__finding")).toHaveLength(1);
    } finally {
      document.querySelector<HTMLInputElement>("#filter-error")!.checked = true;
      document.querySelector<HTMLInputElement>("#filter-error")!.dispatchEvent(new Event("change"));
    }
  });

  it("always shows a file that failed to parse, even with no findings", () => {
    const item = buildPscResultItem(outcome({ ok: false, detail: "boom" }));
    expect(item).not.toBeNull();
    expect(item!.classList.contains("psc-result__item--error")).toBe(true);
    expect(item!.textContent).toContain("boom");
  });

  it("shows a fix button only when findings are auto-fixable", () => {
    const fixable = buildPscResultItem(
      outcome({
        findings: [
          { line: 1, column: 1, message: "[warning] Line contains trailing whitespace", rule: "trailing-whitespace" },
        ],
      }),
    );
    expect(fixable!.querySelector(".psc-result__fix-button")).not.toBeNull();

    const unfixable = buildPscResultItem(
      outcome({ findings: [{ line: 1, column: 1, message: "[error] forbidden function used" }] }),
    );
    expect(unfixable!.querySelector(".psc-result__fix-button")).toBeNull();
  });

  it("shows the path relative to the current project dir, when known", async () => {
    invokeImplFor({
      load_lint_config: () => DEFAULT_LINT_CONFIG,
      load_compiler_path: () => null,
      load_compile_check: () => false,
      load_script_roots: () => [],
    });
    await useProjectDir("/a");

    const item = buildPscResultItem(outcome({ path: "/a/b.psc", ok: false, detail: "boom" }));
    expect(item!.textContent).toContain("b.psc: boom");
    expect(item!.textContent).not.toContain("/a/b.psc");
  });

  it("always shows a compile button, even for a file with no findings", () => {
    const item = buildPscResultItem(outcome({ ok: false, detail: "boom" }));
    expect(item!.querySelector(".psc-result__compile-button")).not.toBeNull();
    expect(item!.querySelector(".psc-result__compile-output")).not.toBeNull();
  });

  it("opens the code viewer when the result's View code button is clicked", async () => {
    invokeImplFor({ read_psc_file: () => "ScriptName A" });
    const item = buildPscResultItem(
      outcome({ ok: false, findings: [{ line: 1, column: 1, message: "[error] bad thing" }] }),
    );
    document.body.append(item!);

    item!.querySelector<HTMLButtonElement>(".psc-result__view-button")!.click();

    await vi.waitFor(() => {
      expect(document.querySelector<HTMLDialogElement>("#code-viewer")!.open).toBe(true);
      expect(document.querySelector("#code-viewer-line-1")).not.toBeNull();
    });
    expect(document.querySelector("#code-viewer-title")!.textContent).toMatch(/a\.psc$/);
    expect(document.querySelector("#code-viewer-line-1")!.classList).toContain("code-viewer__line--error");
  });

  it("runs all automatic fixes when the result's Apply fixes button is clicked", async () => {
    invokeImplFor({ repair_psc_file: () => [] });
    const result = outcome({
      findings: [
        { line: 1, column: 1, message: "[warning] Line contains trailing whitespace", rule: "trailing-whitespace" },
      ],
    });
    const item = buildPscResultItem(result);
    document.body.append(item!);

    item!.querySelector<HTMLButtonElement>(".psc-result__fix-button")!.click();

    await vi.waitFor(() => expect(result.findings).toEqual([]));
    expect(invokeMock).toHaveBeenCalledWith("repair_psc_file", expect.any(Object));
  });

  it("compiles a file when the result's Compile button is clicked", async () => {
    invokeImplFor({
      compile_psc_file: () => ({ success: true, stdout: "Compilation succeeded.", stderr: "" }),
    });
    const item = buildPscResultItem(outcome({ ok: false }));
    document.body.append(item!);

    item!.querySelector<HTMLButtonElement>(".psc-result__compile-button")!.click();

    const output = item!.querySelector<HTMLElement>(".psc-result__compile-output")!;
    await vi.waitFor(() => expect(output.hidden).toBe(false));
    expect(output.textContent).toBe("Compilation succeeded.");
    expect(output.classList).toContain("psc-result__compile-output--ok");
  });

  it("renders one finding entry per finding, tagged with its severity", () => {
    const item = buildPscResultItem(
      outcome({
        findings: [
          { line: 3, column: 5, message: "[error] bad thing" },
          { line: 4, column: 1, message: "[warning] risky thing" },
        ],
      }),
    );
    const findingEls = item!.querySelectorAll(".psc-result__finding");
    expect(findingEls).toHaveLength(2);
    expect(findingEls[0].classList.contains("psc-result__finding--error")).toBe(true);
    expect(findingEls[0].textContent).toContain("line 3, col 5");
    expect(findingEls[1].classList.contains("psc-result__finding--warning")).toBe(true);
  });

  it("shows a finding's tag badges when its rule has known tag metadata", () => {
    // See the matchesTagFilters describe block above for why applyRuleTags
    // is called synchronously right here, with no `await` before it.
    applyRuleTags([
      { rule: "trailing-whitespace", description: "Test description for trailing whitespace.", kinds: ["style"], importance: "low", auto_fixable: true, doc_url: "https://papyrus-lint.idrinth.de/rules.html#rule-trailing-whitespace" },
    ]);
    try {
      const item = buildPscResultItem(
        outcome({
          findings: [{ line: 1, column: 1, message: "[warning] trailing whitespace", rule: "trailing-whitespace" }],
        }),
      );
      const badges = item!.querySelectorAll(".psc-result__tag-badge");
      const badgeText = Array.from(badges).map((badge) => badge.textContent);
      expect(badgeText).toContain("style");
      expect(badgeText).toContain("low importance");
      expect(badgeText).toContain("auto-fixable");
      const docsLink = item!.querySelector<HTMLAnchorElement>(".psc-result__tag-badge--docs-link");
      expect(docsLink?.href).toBe("https://papyrus-lint.idrinth.de/rules.html#rule-trailing-whitespace");
      expect(docsLink?.target).toBe("_blank");
    } finally {
      applyRuleTags([]);
    }
  });

  it("omits the auto-fixable badge for a fixable rule's finding that its own message says can't be fixed", () => {
    applyRuleTags([{ rule: "type-casing", description: "Test description for type casing.", kinds: ["style"], importance: "low", auto_fixable: true, doc_url: "https://papyrus-lint.idrinth.de/rules.html#rule-type-casing" }]);
    try {
      const item = buildPscResultItem(
        outcome({
          findings: [
            {
              line: 1,
              column: 1,
              message:
                "[warning] Script name 'IDR__TIF__050000F5' does not follow the configured PascalCase casing (fixing this would rename the script, so no automatic fix is applied)",
              rule: "type-casing",
            },
          ],
        }),
      );
      const badges = item!.querySelectorAll(".psc-result__tag-badge");
      const badgeText = Array.from(badges).map((badge) => badge.textContent);
      expect(badgeText).toContain("style");
      expect(badgeText).not.toContain("auto-fixable");
      expect(item!.querySelector(".psc-result__finding-fix-button")).toBeNull();
    } finally {
      applyRuleTags([]);
    }
  });

  it("shows no tag badges for a finding whose rule has no known tag metadata", () => {
    const item = buildPscResultItem(
      outcome({ findings: [{ line: 1, column: 1, message: "[error] compile error", rule: "compiler-error" }] }),
    );
    expect(item!.querySelector(".psc-result__tag-badge")).toBeNull();
  });

  it("shows a 'Fix this issue' button only on findings whose own rule is auto-fixable", () => {
    const item = buildPscResultItem(
      outcome({
        findings: [
          { line: 3, column: 5, message: "[warning] missing space", rule: "comma-spacing" },
          { line: 4, column: 1, message: "[error] forbidden function used", rule: "forbidden-functions" },
        ],
      }),
    );
    const findingEls = item!.querySelectorAll(".psc-result__finding");
    expect(findingEls[0].querySelector(".psc-result__finding-fix-button")).not.toBeNull();
    expect(findingEls[1].querySelector(".psc-result__finding-fix-button")).toBeNull();
  });

  it("clicking a finding's fix button does not also open the code viewer", () => {
    invokeImplFor({ repair_psc_finding: () => [] });
    const item = buildPscResultItem(
      outcome({
        findings: [{ line: 3, column: 5, message: "[warning] missing space", rule: "comma-spacing" }],
      }),
    );

    item!.querySelector<HTMLButtonElement>(".psc-result__finding-fix-button")!.click();

    expect(document.querySelector<HTMLDialogElement>("#code-viewer")!.open).toBe(false);
  });

});
