import { describe, expect, it } from "vitest";
import { highlightPapyrusLines } from "./highlight";

describe("highlightPapyrusLines", () => {
  it("returns one entry per line, splitting on \\n", () => {
    const lines = highlightPapyrusLines("a\nb\nc");
    expect(lines).toEqual(["a", "b", "c"]);
  });

  it("returns a single empty-string entry for empty source", () => {
    expect(highlightPapyrusLines("")).toEqual([""]);
  });

  it("preserves a trailing blank line", () => {
    expect(highlightPapyrusLines("a\n")).toEqual(["a", ""]);
  });

  it("highlights keywords", () => {
    expect(highlightPapyrusLines("if true")).toEqual([
      '<span class="cm-keyword">if</span> <span class="cm-keyword">true</span>',
    ]);
  });

  it("keyword matching is case-insensitive but preserves original casing", () => {
    expect(highlightPapyrusLines("If TRUE")).toEqual([
      '<span class="cm-keyword">If</span> <span class="cm-keyword">TRUE</span>',
    ]);
  });

  it("highlights primitive types distinctly from keywords", () => {
    expect(highlightPapyrusLines("int x")).toEqual(['<span class="cm-type">int</span> x']);
  });

  it("does not classify ordinary identifiers", () => {
    expect(highlightPapyrusLines("MyFunction")).toEqual(["MyFunction"]);
  });

  it("highlights decimal and hex integer literals", () => {
    expect(highlightPapyrusLines("42 0xFF")).toEqual([
      '<span class="cm-number">42</span> <span class="cm-number">0xFF</span>',
    ]);
  });

  it("highlights float literals as a single number token", () => {
    expect(highlightPapyrusLines("3.14")).toEqual(['<span class="cm-number">3.14</span>']);
  });

  it("highlights string literals, including escaped quotes", () => {
    expect(highlightPapyrusLines('Debug.Trace("hi \\"there\\"")')).toEqual([
      'Debug.Trace(<span class="cm-string">"hi \\"there\\""</span>)',
    ]);
  });

  it("treats an unterminated string as a string token through end of line", () => {
    expect(highlightPapyrusLines('"unterminated')).toEqual(['<span class="cm-string">"unterminated</span>']);
  });

  it("highlights line comments through end of line", () => {
    expect(highlightPapyrusLines("int x ; a comment")).toEqual([
      '<span class="cm-type">int</span> x <span class="cm-comment">; a comment</span>',
    ]);
  });

  it("highlights brace comments", () => {
    expect(highlightPapyrusLines("{ a brace comment }")).toEqual([
      '<span class="cm-comment">{ a brace comment }</span>',
    ]);
  });

  it("keeps a block comment's class active across its spanned lines", () => {
    const lines = highlightPapyrusLines(";/\nblock body\n/;\nafter");
    expect(lines).toEqual([
      '<span class="cm-comment">;/</span>',
      '<span class="cm-comment">block body</span>',
      '<span class="cm-comment">/;</span>',
      "after",
    ]);
  });

  it("escapes HTML-significant characters in unclassified text", () => {
    expect(highlightPapyrusLines("a < b && b > c")).toEqual(["a &lt; b &amp;&amp; b &gt; c"]);
  });

  it("escapes HTML-significant characters inside classified tokens", () => {
    expect(highlightPapyrusLines('"<tag>"')).toEqual(['<span class="cm-string">"&lt;tag&gt;"</span>']);
  });
});
