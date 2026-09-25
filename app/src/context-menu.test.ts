import { afterEach, describe, expect, it } from "vitest";
import { bindContextMenu, shouldAllowNativeContextMenu } from "./context-menu";

describe("shouldAllowNativeContextMenu", () => {
  it("is false for a non-editable element", () => {
    expect(shouldAllowNativeContextMenu(document.createElement("div"))).toBe(false);
  });

  it("is false when the target is missing", () => {
    expect(shouldAllowNativeContextMenu(null)).toBe(false);
  });

  it("is true for an input", () => {
    expect(shouldAllowNativeContextMenu(document.createElement("input"))).toBe(true);
  });

  it("is true for a textarea", () => {
    expect(shouldAllowNativeContextMenu(document.createElement("textarea"))).toBe(true);
  });

  it("is true for a contenteditable host", () => {
    const host = document.createElement("div");
    host.setAttribute("contenteditable", "true");
    expect(shouldAllowNativeContextMenu(host)).toBe(true);
  });

  it("is true for a text node inside a contenteditable host", () => {
    const host = document.createElement("div");
    host.setAttribute("contenteditable", "true");
    const text = document.createTextNode("copy me");
    host.append(text);
    expect(shouldAllowNativeContextMenu(text)).toBe(true);
  });

  it("is false for contenteditable=false", () => {
    const host = document.createElement("div");
    host.setAttribute("contenteditable", "false");
    expect(shouldAllowNativeContextMenu(host)).toBe(false);
  });
});

describe("bindContextMenu", () => {
  afterEach(() => {
    document.body.replaceChildren();
  });

  it("cancels contextmenu outside text fields", () => {
    bindContextMenu();
    const button = document.createElement("button");
    document.body.append(button);
    const event = new MouseEvent("contextmenu", { bubbles: true, cancelable: true });
    button.dispatchEvent(event);
    expect(event.defaultPrevented).toBe(true);
  });

  it("leaves contextmenu alone on a textarea", () => {
    bindContextMenu();
    const textarea = document.createElement("textarea");
    document.body.append(textarea);
    const event = new MouseEvent("contextmenu", { bubbles: true, cancelable: true });
    textarea.dispatchEvent(event);
    expect(event.defaultPrevented).toBe(false);
  });
});
