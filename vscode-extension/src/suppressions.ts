/** PapyrusCompiler.exe diagnostics (see `papyrus_lint_core::compile_diagnostics`)
 * are appended after `@disable`/`@disable-file` filtering and have no `rules.*`
 * config toggle, so none of the ignore actions can silence them. */
export const UNCONFIGURABLE_RULES = new Set(['compiler-error']);

export function isIgnorableRule(rule: string): boolean {
  return rule !== '' && !UNCONFIGURABLE_RULES.has(rule);
}

/** Config keys under `rules:` use underscores (`float_to_int` for `float-to-int`). */
export function ruleConfigKey(ruleId: string): string {
  return ruleId.replace(/-/g, '_');
}

function detectEol(text: string): '\n' | '\r\n' {
  return text.includes('\r\n') ? '\r\n' : '\n';
}

function splitLines(text: string): { lines: string[]; eol: '\n' | '\r\n'; trailingEol: boolean } {
  const eol = detectEol(text);
  if (text === '') {
    return { lines: [], eol, trailingEol: false };
  }
  const trailingEol = text.endsWith('\n');
  const lines = text.split(/\r?\n/);
  if (trailingEol) {
    lines.pop();
  }
  return { lines, eol, trailingEol };
}

function joinLines(lines: string[], eol: string, trailingEol: boolean): string {
  if (lines.length === 0) {
    return trailingEol ? eol : '';
  }
  return lines.join(eol) + (trailingEol ? eol : '');
}

function lineCommentText(line: string): string | undefined {
  let inString = false;
  for (let index = 0; index < line.length; index += 1) {
    const character = line[index];
    if (character === '"') {
      inString = !inString;
    } else if (character === '\\' && inString) {
      index += 1;
    } else if (character === ';' && !inString) {
      return line[index + 1] === '/' ? undefined : line.slice(index + 1);
    }
  }
  return undefined;
}

type DisableDirective = { kind: 'all' } | { kind: 'rules'; ids: string[] };

function parseDirective(line: string, keyword: string): DisableDirective | undefined {
  const comment = lineCommentText(line);
  if (comment === undefined) {
    return undefined;
  }
  const index = comment.toLowerCase().indexOf(keyword);
  if (index < 0) {
    return undefined;
  }
  const after = comment.slice(index + keyword.length);
  if (after.length > 0 && !/\s/.test(after[0])) {
    return undefined;
  }
  const rest = after.trim();
  if (rest === '') {
    return { kind: 'all' };
  }
  const ids = rest
    .split(/[,\s]+/)
    .map((part) => part.trim().toLowerCase())
    .filter((part) => part !== '');
  return { kind: 'rules', ids };
}

function parseDisableFile(line: string): DisableDirective | undefined {
  return parseDirective(line, '@disable-file');
}

function parseLineDisable(line: string): DisableDirective | undefined {
  return parseDirective(line, '@disable');
}

function directiveCovers(directive: DisableDirective | undefined, rule: string): boolean {
  if (directive === undefined) {
    return false;
  }
  if (directive.kind === 'all') {
    return true;
  }
  return directive.ids.includes(rule.toLowerCase());
}

function lineWithoutCarriageReturn(line: string): { content: string; trailingCr: string } {
  if (line.endsWith('\r')) {
    return { content: line.slice(0, -1), trailingCr: '\r' };
  }
  return { content: line, trailingCr: '' };
}

export function fileDisableCovers(source: string, rule: string): boolean {
  for (const line of source.split(/\r?\n/)) {
    if (directiveCovers(parseDisableFile(line), rule)) {
      return true;
    }
  }
  return false;
}

export function lineDisableCovers(source: string, line: number, rule: string): boolean {
  if (rule === '' || line < 1) {
    return false;
  }
  const { lines } = splitLines(source);
  const index = line - 1;
  if (index >= lines.length) {
    return false;
  }
  return directiveCovers(parseLineDisable(lineWithoutCarriageReturn(lines[index]).content), rule);
}

/** Adds (or extends) a `; @disable-file <rule>` comment in `source`, matching the
 * merge rules `papyrus_lints::add_disable_comment` uses for per-line `@disable`:
 * a bare `@disable-file` is left untouched, an existing named list is extended
 * in place, and otherwise a new comment is inserted as the first line. */
export function addFileDisableComment(source: string, rule: string): string {
  if (rule === '') {
    return source;
  }
  const { lines, eol, trailingEol } = splitLines(source);
  const needle = rule.toLowerCase();
  for (let index = 0; index < lines.length; index += 1) {
    const { content, trailingCr } = lineWithoutCarriageReturn(lines[index]);
    const directive = parseDisableFile(content);
    if (directive?.kind === 'all') {
      return source;
    }
    if (directive?.kind === 'rules') {
      if (directive.ids.includes(needle)) {
        return source;
      }
      lines[index] = `${content}, ${rule}${trailingCr}`;
      return joinLines(lines, eol, trailingEol);
    }
  }
  const comment = `; @disable-file ${rule}`;
  if (source === '') {
    return `${comment}${eol}`;
  }
  return `${comment}${eol}${source}`;
}

/** Adds (or extends) a `; @disable <rule>` comment on the 1-indexed `line` of
 * `source`, matching `papyrus_lints::add_disable_comment`: a bare `@disable`
 * is left untouched, an existing named list is extended in place, a trailing
 * comment without a directive gets ` @disable <rule>` appended, and otherwise
 * ` ; @disable <rule>` is added. An empty rule id or an out-of-range line
 * leaves `source` untouched. */
export function addLineDisableComment(source: string, line: number, rule: string): string {
  if (rule === '' || line < 1) {
    return source;
  }
  const { lines, eol, trailingEol } = splitLines(source);
  const index = line - 1;
  if (index >= lines.length) {
    return source;
  }
  const { content, trailingCr } = lineWithoutCarriageReturn(lines[index]);
  const directive = parseLineDisable(content);
  if (directive?.kind === 'all') {
    return source;
  }
  if (directive?.kind === 'rules') {
    if (directive.ids.includes(rule.toLowerCase())) {
      return source;
    }
    lines[index] = `${content}, ${rule}${trailingCr}`;
    return joinLines(lines, eol, trailingEol);
  }
  const separator = lineCommentText(content) === undefined ? ' ; ' : ' ';
  lines[index] = `${content}${separator}@disable ${rule}${trailingCr}`;
  return joinLines(lines, eol, trailingEol);
}

const BOOLEAN = String.raw`(true|false|yes|no|on|off)`;

function isBlankOrComment(line: string): boolean {
  return /^\s*(?:#.*)?$/.test(line);
}

function indentWidth(line: string): number {
  const match = /^(?<indent>[ \t]*)/.exec(line);
  return match?.groups?.indent.length ?? 0;
}

function ruleKeyPattern(key: string): string {
  return String.raw`(?:${key}|['"]${key}['"])`;
}

/** Sets `rules.<underscore-id>: false` in a papyrus-lint YAML document, preserving
 * comments and unrelated keys. Creates a `rules:` block when the file has none. */
export function disableRuleInConfigYaml(yaml: string, ruleId: string): string {
  const key = ruleConfigKey(ruleId);
  if (key === '') {
    return yaml;
  }
  const { lines, eol, trailingEol } = splitLines(yaml);
  if (lines.length === 0) {
    return `rules:${eol}  ${key}: false${eol}`;
  }

  const flowIndex = lines.findIndex((candidate) => /^rules:\s*\{/.test(candidate));
  if (flowIndex >= 0) {
    const updated = disableRuleInFlowMapping(lines[flowIndex], key, eol);
    if (updated === lines[flowIndex]) {
      return yaml;
    }
    lines[flowIndex] = updated;
    return joinLines(lines, eol, trailingEol);

  }

  const blockIndex = lines.findIndex((candidate) => /^rules:\s*(?:#.*)?$/.test(candidate));
  if (blockIndex < 0) {
    const prefix = joinLines(lines, eol, true);
    return `${prefix}rules:${eol}  ${key}: false${eol}`;
  }

  const keyLine = new RegExp(`^(?<indent>[ \\t]+)${ruleKeyPattern(key)}\\s*:\\s*${BOOLEAN}\\b(?<rest>.*)$`, 'i');
  let childIndent: string | undefined;
  for (let index = blockIndex + 1; index < lines.length; index += 1) {
    const candidate = lines[index];
    if (isBlankOrComment(candidate)) {
      continue;
    }
    const indent = indentWidth(candidate);
    if (indent === 0) {
      break;
    }
    childIndent ??= candidate.slice(0, indent);
    const matched = keyLine.exec(candidate);
    if (matched?.groups) {
      const current = candidate.slice(matched.groups.indent.length);
      const replaced = current.replace(new RegExp(`^${ruleKeyPattern(key)}\\s*:\\s*${BOOLEAN}\\b`, 'i'), (whole) =>
        whole.replace(new RegExp(BOOLEAN, 'i'), 'false'),
      );
      if (replaced === current) {
        return yaml;
      }
      lines[index] = `${matched.groups.indent}${replaced}`;
      return joinLines(lines, eol, trailingEol);
    }
  }

  const indent = childIndent ?? '  ';
  lines.splice(blockIndex + 1, 0, `${indent}${key}: false`);
  return joinLines(lines, eol, trailingEol);
}

function disableRuleInFlowMapping(line: string, key: string, eol: string): string {
  const match = /^rules:\s*\{(?<body>.*)\}(?<rest>\s*(?:#.*)?)?$/.exec(line);
  if (!match?.groups) {
    return `rules:${eol}  ${key}: false`;
  }
  const body = match.groups.body.trim();
  const rest = match.groups.rest ?? '';
  const entry = new RegExp(`${ruleKeyPattern(key)}\\s*:\\s*${BOOLEAN}\\b`, 'i');
  if (entry.test(body)) {
    const updated = body.replace(entry, (whole) => whole.replace(new RegExp(BOOLEAN, 'i'), 'false'));
    return `rules: { ${updated} }${rest}`;
  }
  const separator = body === '' ? '' : `${body.replace(/,?\s*$/, '')}, `;
  return `rules: { ${separator}${key}: false }${rest}`;
}
