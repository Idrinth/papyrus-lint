import { mkdtempSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import path from "node:path";
import { describe, expect, it } from "vitest";
import {
  filterByTag,
  injectContactLinks,
  parseLinks,
  renderHtmlListItems,
  replaceHtmlLinkMarkers,
} from "./inject-links.mjs";

const SAMPLE = `Discord:
  url: https://discord.example/invite
  type:
  - contact
Nexus Mods:
  url: https://nexus.example/mod
  type:
  - contact
  - download
Docs:
  url: https://docs.example/
  type:
  - documentation
`;

describe("parseLinks", () => {
  it("preserves YAML order, labels, urls, and tags", () => {
    const links = parseLinks(SAMPLE);

    expect(links.map((link) => link.label)).toEqual(["Discord", "Nexus Mods", "Docs"]);
    expect(links.map((link) => link.url)).toEqual([
      "https://discord.example/invite",
      "https://nexus.example/mod",
      "https://docs.example/",
    ]);
    expect([...links[0].tags]).toEqual(["contact"]);
    expect([...links[1].tags].sort()).toEqual(["contact", "download"]);
  });

  it("rejects a document with no links", () => {
    expect(() => parseLinks("# nothing\n")).toThrow(/no links/);
  });

  it("rejects a missing url", () => {
    expect(() => parseLinks("Discord:\n  type:\n  - contact\n")).toThrow(/missing url/);
  });
});

describe("filterByTag", () => {
  it("filters by tag without selecting labels by name", () => {
    const links = parseLinks(SAMPLE);

    expect(filterByTag(links, "contact").map((link) => link.url)).toEqual([
      "https://discord.example/invite",
      "https://nexus.example/mod",
    ]);
    expect(filterByTag(links, "documentation").map((link) => link.label)).toEqual(["Docs"]);
    expect(filterByTag(links, null)).toEqual(links);
  });
});

describe("replaceHtmlLinkMarkers", () => {
  it("fills CONTACT-LINKS from the contact tag and LINKS from every entry", () => {
    const links = parseLinks(SAMPLE);
    const result = replaceHtmlLinkMarkers(
      "<ul><!--CONTACT-LINKS--></ul>\n<div><!--LINKS--></div>",
      renderHtmlListItems,
      links,
    );

    expect(result).toContain("https://discord.example/invite");
    expect(result).toContain("https://docs.example/");
    expect(result.split("</ul>")[0]).not.toContain("https://docs.example/");
    expect(result).not.toContain("CONTACT-LINKS");
  });

  it("escapes labels and urls in list items", () => {
    const rendered = renderHtmlListItems([
      { label: 'A & B', url: 'https://example.test/?q=a&b="c"', tags: new Set(["contact"]) },
    ]);

    expect(rendered).toContain("&" + "amp;");
    expect(rendered).toContain("&" + "quot;");
    expect(rendered).not.toContain("A & B");
    expect(rendered.startsWith("<li><a href=")).toBe(true);
  });

  it("errors when a marker's tag matches no entries", () => {
    expect(() =>
      replaceHtmlLinkMarkers("<!--FORUM-LINKS-->", renderHtmlListItems, parseLinks(SAMPLE)),
    ).toThrow(/no links tagged "forum"/);
  });
});

describe("injectContactLinks", () => {
  it("is a Vite plugin that fills index.html markers from a yaml file", () => {
    const directory = mkdtempSync(path.join(tmpdir(), "inject-links-"));
    const yamlPath = path.join(directory, "links.yaml");
    writeFileSync(yamlPath, SAMPLE);

    const plugin = injectContactLinks(yamlPath);
    const html = plugin.transformIndexHtml("<ul><!--CONTACT-LINKS--></ul>");

    expect(plugin.name).toBe("inject-links");
    expect(html).toContain("https://discord.example/invite");
    expect(html).not.toContain("https://docs.example/");
    expect(html).not.toContain("CONTACT-LINKS");
  });
});
