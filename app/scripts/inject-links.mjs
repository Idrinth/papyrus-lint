#!/usr/bin/env node
// Loads shared/links.yaml and fills <!--TAG-LINKS--> / <!--LINKS--> in
// index.html at Vite transform time. Labels are the YAML keys; the marker
// encodes the tag to filter by, so destinations never name a label.

import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

export function parseLinks(source, origin = "links.yaml") {
  const links = [];
  const seen = new Set();
  let label = null;
  let url = null;
  let tags = [];
  let inTypeList = false;

  const finish = () => {
    if (label === null) {
      return;
    }
    if (!url) {
      throw new Error(`${origin}: ${JSON.stringify(label)} is missing url`);
    }
    if (!/^https?:\/\//.test(url)) {
      throw new Error(`${origin}: ${JSON.stringify(label)} url must be an HTTP(S) URL: ${url}`);
    }
    if (tags.length === 0) {
      throw new Error(`${origin}: ${JSON.stringify(label)} is missing type`);
    }
    links.push({ label, url, tags: new Set(tags) });
    label = null;
    url = null;
    tags = [];
    inTypeList = false;
  };

  const lines = source.split(/\r?\n/);
  for (let index = 0; index < lines.length; index += 1) {
    const rawLine = lines[index];
    const strippedFull = rawLine.trim();
    if (!strippedFull || strippedFull.startsWith("#")) {
      continue;
    }
    const line = rawLine.replace(/\s+$/, "");
    const indent = line.length - line.trimStart().length;
    const stripped = line.trim();
    const location = `${origin}:${index + 1}`;

    if (indent === 0) {
      finish();
      if (!stripped.endsWith(":") || stripped === ":") {
        throw new Error(`${location}: expected 'Label:'`);
      }
      label = stripped.slice(0, -1).trim();
      if (!label) {
        throw new Error(`${location}: empty label`);
      }
      if (seen.has(label)) {
        throw new Error(`${location}: duplicate label ${JSON.stringify(label)}`);
      }
      seen.add(label);
      continue;
    }

    if (label === null) {
      throw new Error(`${location}: indented line is not under a label`);
    }

    if (stripped.startsWith("- ")) {
      if (!inTypeList) {
        throw new Error(`${location}: list item is not under type`);
      }
      const tag = stripped.slice(2).trim();
      if (!tag) {
        throw new Error(`${location}: empty type tag`);
      }
      if (tags.includes(tag)) {
        throw new Error(`${location}: duplicate type tag ${JSON.stringify(tag)}`);
      }
      tags.push(tag);
      continue;
    }

    const sep = stripped.indexOf(":");
    if (sep < 0) {
      throw new Error(`${location}: expected 'key: value'`);
    }
    const key = stripped.slice(0, sep).trim();
    const value = stripped.slice(sep + 1).trim();
    if (key === "url") {
      if (!value) {
        throw new Error(`${location}: empty url`);
      }
      url = value;
      inTypeList = false;
    } else if (key === "type") {
      inTypeList = true;
      if (value) {
        throw new Error(`${location}: type must be a list, not ${JSON.stringify(value)}`);
      }
    } else {
      throw new Error(`${location}: unknown key ${JSON.stringify(key)}`);
    }
  }

  finish();
  if (links.length === 0) {
    throw new Error(`${origin}: no links`);
  }
  return links;
}

export function filterByTag(links, tag) {
  if (tag == null) {
    return [...links];
  }
  return links.filter((link) => link.tags.has(tag));
}

function escapeHtml(value) {
  return value.replace(/[&<>"]/g, (char) => ({
    "&": "\u0026amp;",
    "<": "\u0026lt;",
    ">": "\u0026gt;",
    '"': "\u0026quot;",
  }[char]));
}

export function renderHtmlListItems(links) {
  return links
    .map(
      (link) =>
        `<li><a href="${escapeHtml(link.url)}" target="_blank" rel="noopener noreferrer">${escapeHtml(link.label)}</a></li>`,
    )
    .join("\n");
}

export function replaceHtmlLinkMarkers(text, renderer, links) {
  const pattern = /<!--(?:([A-Z][A-Z0-9]*)-)?LINKS-->/g;
  if (!pattern.test(text)) {
    return text;
  }
  pattern.lastIndex = 0;
  return text.replace(pattern, (_, tag) => {
    const filtered = filterByTag(links, tag ? tag.toLowerCase() : null);
    if (filtered.length === 0) {
      if (!tag) {
        throw new Error("links.yaml: no links");
      }
      throw new Error(`links.yaml: no links tagged ${JSON.stringify(tag.toLowerCase())}`);
    }
    return renderer(filtered);
  });
}

export function loadLinks(linksPath) {
  return parseLinks(fs.readFileSync(linksPath, "utf8"), linksPath);
}

export function injectContactLinks(linksPath) {
  const resolved =
    linksPath ??
    path.resolve(path.dirname(fileURLToPath(import.meta.url)), "../../shared/links.yaml");
  return {
    name: "inject-links",
    transformIndexHtml(html) {
      return replaceHtmlLinkMarkers(html, renderHtmlListItems, loadLinks(resolved));
    },
  };
}
