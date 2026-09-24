// Detects Papyrus function headers eligible for the code viewer's "Add
// nodiscard" quick action - a function that returns a value or is Native -
// and whether they already carry the `; @nodiscard` flag, working off raw
// source text the same way papyrus-source.ts's declaration scanner and the
// highlighter do, rather than requiring an AST parse or a Tauri round-trip.
// Mirrors papyrus_lints::unused_nodiscard's own header lookback (the
// header line, or the line directly above it) by hand, since this
// package doesn't share Rust's token/AST info.
import { IDENTIFIER, stripComments } from "./papyrus-source";

const FUNCTION_HEADER = new RegExp(
  `^[ \\t]*(?:(${IDENTIFIER})(?:\\[\\])?\\s+)?Function\\s+${IDENTIFIER}\\s*\\([^)]*\\)(.*)$`,
  "i",
);

// Bounded by whitespace/comma (or line start/end) so "@nodiscardable"
// isn't mistaken for the flag, the same rule
// papyrus_lints::unused_nodiscard::line_has_nodiscard applies.
const NODISCARD_FLAG = /(?:^|[\s,])@nodiscard(?:$|[\s,])/i;

function isEligibleHeaderLine(strippedLine: string): boolean {
  const match = FUNCTION_HEADER.exec(strippedLine);
  if (!match) {
    return false;
  }
  const [, returnType, flags] = match;
  return returnType !== undefined || /\bNative\b/i.test(flags);
}

// Returns the text of `line`'s trailing `;` line comment, or undefined if
// it doesn't have one - ignoring semicolons inside string literals, the
// same as papyrus_lints::unused_nodiscard::line_comment_text.
function commentText(line: string): string | undefined {
  let inString = false;
  for (let index = 0; index < line.length; index++) {
    const char = line[index];
    if (char === '"') {
      inString = !inString;
    } else if (char === "\\" && inString) {
      index += 1;
    } else if (char === ";" && !inString) {
      return line.slice(index + 1);
    }
  }
  return undefined;
}

function lineHasNodiscard(line: string): boolean {
  const comment = commentText(line);
  return comment !== undefined && NODISCARD_FLAG.test(comment);
}

// Every 1-indexed line number in `source` whose function header is
// eligible for "Add nodiscard" (returns a value or is Native) and isn't
// flagged already, on that line or the one directly above it - the set
// the code viewer's per-line "Nodiscard" button (see code-viewer-view.ts)
// is shown for.
export function nodiscardEligibleLines(source: string): Set<number> {
  const lines = source.split(/\r?\n/);
  const strippedLines = stripComments(source).split(/\r?\n/);
  const eligible = new Set<number>();
  strippedLines.forEach((strippedLine, index) => {
    if (!isEligibleHeaderLine(strippedLine)) {
      return;
    }
    if (lineHasNodiscard(lines[index]) || (index > 0 && lineHasNodiscard(lines[index - 1]))) {
      return;
    }
    eligible.add(index + 1);
  });
  return eligible;
}
