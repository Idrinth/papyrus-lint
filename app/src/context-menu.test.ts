import { afterEach, describe, expect, it } from "vitest";
import { bindContextMenu, shouldAllowNativeContextMenu } from "./context-menu";

describe("shouldAllowNativeContextMenu", () => {
  it("is false for a non-editable element", () => {
    expect(shouldAllowNativeContextMenu(document.createElement("div"))).toBe(false);
  });

  it.each([
    ["a missing target", null],
    ["a non-Node event target", new EventTarget()],
    ["a detached text node", document.createTextNode("detached")],
  ])("is false for %s", (_description, target) => {
    expect(shouldAllowNativeContextMenu(target)).toBe(false);
  });

  it("is true for an input", () => {
    expect(shouldAllowNativeContextMenu(document.createElement("input"))).toBe(true);
  });

  it("is true for a textarea", () => {
    expect(shouldAllowNativeContextMenu(document.createElement("textarea"))).toBe(true);
  });

  it.each(["", "true", "plaintext-only"])(
    "is true for a contenteditable=%j host",
    (contenteditable) => {
      const host = document.createElement("div");
      host.setAttribute("contenteditable", contenteditable);
      expect(shouldAllowNativeContextMenu(host)).toBe(true);
    },
  );

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

  it.each(["input", "textarea"])("leaves contextmenu alone on an %s", (tagName) => {
    bindContextMenu();
    const field = document.createElement(tagName);
    document.body.append(field);
    const event = new MouseEvent("contextmenu", { bubbles: true, cancelable: true });
    field.dispatchEvent(event);
    expect(event.defaultPrevented).toBe(false);
  });

  it("leaves contextmenu alone on a descendant of a contenteditable host", () => {
    bindContextMenu();
    const host = document.createElement("div");
    host.setAttribute("contenteditable", "true");
    const child = document.createElement("span");
    host.append(child);
    document.body.append(host);

    const event = new MouseEvent("contextmenu", { bubbles: true, cancelable: true });
    child.dispatchEvent(event);

    expect(event.defaultPrevented).toBe(false);
  });
});
