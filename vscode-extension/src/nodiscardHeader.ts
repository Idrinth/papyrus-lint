// Detects Papyrus function headers eligible for the "Add ; @nodiscard flag"
// quick action -- a function that returns a value or is Native -- and
// whether they already carry the flag, working off the document's own raw
// text rather than an AST: this extension only shells out to the CLI for
// diagnostics (see linter.ts), it never gets parsed structure back. Mirrors
// papyrus_lints::unused_nodiscard's own header lookback (the header line,
// or the line directly above it) by hand, since this package doesn't share
// Rust's token/AST info.

const IDENTIFIER = '[A-Za-z_]\\w*';

const FUNCTION_HEADER = new RegExp(
  `^[ \\t]*(?:(${IDENTIFIER})(?:\\[\\])?\\s+)?Function\\s+${IDENTIFIER}\\s*\\([^)]*\\)(.*)$`,
  'i',
);

// Bounded by whitespace/comma (or line start/end) so "@nodiscardable" isn't
// mistaken for the flag, the same rule
// papyrus_lints::unused_nodiscard::line_has_nodiscard applies.
const NODISCARD_FLAG = /(?:^|[\s,])@nodiscard(?:$|[\s,])/i;

/** Returns the code portion of `line`, with any trailing `;` line comment
 * removed -- ignoring a `;` inside a string literal -- so a comment's text
 * (e.g. mentioning "Native" in passing) can't be mistaken for the header
 * flags that follow a function's closing paren. */
function stripLineComment(line: string): string {
  let inString = false;
  for (let index = 0; index < line.length; index++) {
    const char = line[index];
    if (char === '"') {
      inString = !inString;
    } else if (char === '\\' && inString) {
      index += 1;
    } else if (char === ';' && !inString) {
      return line.slice(0, index);
    }
  }
  return line;
}

/** Returns the text of `line`'s trailing `;` line comment, or undefined if
 * it doesn't have one -- ignoring semicolons inside string literals, the
 * same as papyrus_lints::unused_nodiscard::line_comment_text. */
function commentText(line: string): string | undefined {
  let inString = false;
  for (let index = 0; index < line.length; index++) {
    const char = line[index];
    if (char === '"') {
      inString = !inString;
    } else if (char === '\\' && inString) {
      index += 1;
    } else if (char === ';' && !inString) {
      return line.slice(index + 1);
    }
  }
  return undefined;
}

/** Whether `line` is a function header that returns a value or is Native --
 * the eligibility rule for the "Add ; @nodiscard flag" quick action. An
 * `Event` declaration never matches (it doesn't use the `Function`
 * keyword). */
export function isEligibleNodiscardHeader(line: string): boolean {
  const match = FUNCTION_HEADER.exec(stripLineComment(line));
  if (!match) {
    return false;
  }
  const [, returnType, flags] = match;
  return returnType !== undefined || /\bNative\b/i.test(flags);
}

/** Whether `lines[headerIndex]` (0-indexed) already carries `; @nodiscard`,
 * on that line itself or the one directly above it. */
export function isAlreadyNodiscard(lines: string[], headerIndex: number): boolean {
  const line = lines[headerIndex];
  if (line !== undefined && lineHasNodiscard(line)) {
    return true;
  }
  const above = lines[headerIndex - 1];
  return above !== undefined && lineHasNodiscard(above);
}

function lineHasNodiscard(line: string): boolean {
  const comment = commentText(line);
  return comment !== undefined && NODISCARD_FLAG.test(comment);
}

/** The text to insert at the end of `line` to flag it `@nodiscard`: extends
 * an existing trailing comment with ` @nodiscard`, or starts a new one with
 * ` ; @nodiscard` if `line` doesn't have one. */
export function nodiscardInsertion(line: string): string {
  return commentText(line) !== undefined ? ' @nodiscard' : ' ; @nodiscard';
}
