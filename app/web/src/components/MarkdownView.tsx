// A minimal Markdown renderer that covers the cases the assistant
// actually uses: paragraphs, headings, bold/italic, inline code,
// fenced code blocks, ordered and unordered lists, task lists,
// blockquotes, and tables. We intentionally avoid a heavy Markdown
// library to keep the assistant bundle small. Each code block is
// rendered as a copyable monospace block; syntax highlighting is
// done by a tiny language-specific tokenizer that covers the
// common cases the assistant generates (Python, JS/TS, Rust, Go,
// JSON, bash, SQL, HTML, CSS, YAML, TOML). The tokenizer is
// intentionally simple — it is not a full parser — but it is
// enough to make the assistant's code output readable.

import { useEffect, useState } from "react";
import type { JSX, ReactNode } from "react";

type Props = { text: string };

type Block =
  | { kind: "p"; text: string }
  | { kind: "h"; level: 1 | 2 | 3; text: string }
  | { kind: "code"; lang: string; text: string }
  | { kind: "ul"; items: string[] }
  | { kind: "ol"; items: string[] }
  | { kind: "task"; items: { checked: boolean; text: string }[] }
  | { kind: "quote"; text: string }
  | { kind: "hr" }
  | { kind: "table"; head: string[]; rows: string[][]; align: Array<"left" | "right" | "center" | null> };

// ----------------- parser -----------------

function isTableSeparator(line: string): boolean {
  // A `| --- | --- |` style line. We allow a leading/trailing
  // optional `|` and `:` for alignment markers.
  const trimmed = line.trim().replace(/^\|/, "").replace(/\|$/, "");
  if (trimmed === "") return false;
  const cells = trimmed.split("|");
  if (cells.length < 2) return false;
  return cells.every((c) => /^\s*:?-{3,}:?\s*$/.test(c));
}

function parseAlign(cell: string): "left" | "right" | "center" | null {
  const t = cell.trim();
  if (!/^:?-+:?$/.test(t)) return null;
  const starts = t.startsWith(":");
  const ends = t.endsWith(":");
  if (starts && ends) return "center";
  if (ends) return "right";
  if (starts) return "left";
  return null;
}

function parseBlocks(src: string): Block[] {
  const lines = src.replace(/\r\n/g, "\n").split("\n");
  const blocks: Block[] = [];
  let i = 0;
  while (i < lines.length) {
    const line = lines[i];

    // Code fence.
    if (line.startsWith("```")) {
      const lang = line.slice(3).trim();
      const buf: string[] = [];
      i += 1;
      while (i < lines.length && !lines[i].startsWith("```")) { buf.push(lines[i]); i += 1; }
      blocks.push({ kind: "code", lang, text: buf.join("\n") });
      i += 1;
      continue;
    }

    // Heading.
    if (/^#{1,3} /.test(line)) {
      const level = line.startsWith("### ") ? 3 : line.startsWith("## ") ? 2 : 1;
      blocks.push({ kind: "h", level, text: line.replace(/^#{1,3} /, "").trim() });
      i += 1; continue;
    }

    // Horizontal rule.
    if (/^\s*[-*_]{3,}\s*$/.test(line)) {
      blocks.push({ kind: "hr" });
      i += 1; continue;
    }

    // Blockquote.
    if (/^> /.test(line)) {
      const buf = [line.replace(/^> /, "")];
      i += 1;
      while (i < lines.length && /^> /.test(lines[i])) { buf.push(lines[i].replace(/^> /, "")); i += 1; }
      blocks.push({ kind: "quote", text: buf.join("\n") });
      continue;
    }

    // Task list (`- [ ] item` / `- [x] item`).
    if (/^\s*[-*]\s+\[[ xX]\] /.test(line)) {
      const items: { checked: boolean; text: string }[] = [];
      while (i < lines.length && /^\s*[-*]\s+\[[ xX]\] /.test(lines[i])) {
        const m = lines[i].match(/^\s*[-*]\s+\[([ xX])\]\s+(.*)$/)!;
        items.push({ checked: m[1] !== " ", text: m[2].trim() });
        i += 1;
      }
      blocks.push({ kind: "task", items });
      continue;
    }

    // Unordered list.
    if (/^[-*] /.test(line)) {
      const items: string[] = [];
      while (i < lines.length && /^[-*] /.test(lines[i])) {
        items.push(lines[i].replace(/^[-*] /, "").trim());
        i += 1;
      }
      blocks.push({ kind: "ul", items });
      continue;
    }

    // Ordered list.
    if (/^\d+\.\s+/.test(line)) {
      const items: string[] = [];
      while (i < lines.length && /^\d+\.\s+/.test(lines[i])) {
        items.push(lines[i].replace(/^\d+\.\s+/, "").trim());
        i += 1;
      }
      blocks.push({ kind: "ol", items });
      continue;
    }

    // Table: header line, separator, rows.
    if (line.includes("|") && i + 1 < lines.length && isTableSeparator(lines[i + 1])) {
      const head = splitTableRow(line);
      const sepLine = lines[i + 1];
      const sepCells = sepLine.trim().replace(/^\|/, "").replace(/\|$/, "").split("|");
      const align = sepCells.map(parseAlign);
      const rows: string[][] = [];
      i += 2;
      while (i < lines.length && lines[i].includes("|") && lines[i].trim() !== "") {
        rows.push(splitTableRow(lines[i]));
        i += 1;
      }
      blocks.push({ kind: "table", head, rows, align });
      continue;
    }

    if (line.trim() === "") { i += 1; continue; }

    // Paragraph: consume consecutive non-empty, non-special lines.
    const buf = [line];
    i += 1;
    while (i < lines.length && lines[i].trim() !== "" && !/^(```|#{1,3} |> |[-*] |\d+\. )/.test(lines[i]) && !isTableSeparator(lines[i])) {
      buf.push(lines[i]); i += 1;
    }
    blocks.push({ kind: "p", text: buf.join(" ") });
  }
  return blocks;
}

function splitTableRow(line: string): string[] {
  const trimmed = line.trim().replace(/^\|/, "").replace(/\|$/, "");
  return trimmed.split("|").map((s) => s.trim());
}

// ----------------- inline renderer -----------------

function renderInline(text: string, keyPrefix: string): JSX.Element[] {
  const tokens: JSX.Element[] = [];
  let rest = text;
  let key = 0;
  const patterns: { re: RegExp; render: (m: RegExpMatchArray) => JSX.Element }[] = [
    { re: /\*\*(.+?)\*\*/, render: (m) => <strong key={`${keyPrefix}-b-${key++}`}>{m[1]}</strong> },
    { re: /`([^`]+)`/, render: (m) => <code key={`${keyPrefix}-c-${key++}`} className="px-1 py-0.5 rounded bg-ink-800/80 text-cyan2-400 font-mono text-[0.85em]">{m[1]}</code> },
    { re: /\*([^*]+)\*/, render: (m) => <em key={`${keyPrefix}-i-${key++}`}>{m[1]}</em> },
    { re: /\[([^\]]+)\]\(([^)]+)\)/, render: (m) => <a key={`${keyPrefix}-a-${key++}`} href={m[2]} className="text-cyan2-400 hover:underline" target="_blank" rel="noreferrer">{m[1]}</a> },
  ];
  while (rest.length > 0) {
    let earliest: { idx: number; len: number; el: JSX.Element } | null = null;
    for (const p of patterns) {
      const m = rest.match(p.re);
      if (m && m.index !== undefined) {
        if (!earliest || m.index < earliest.idx) {
          earliest = { idx: m.index, len: m[0].length, el: p.render(m) };
        }
      }
    }
    if (!earliest) {
      tokens.push(<span key={`${keyPrefix}-t-${key++}`}>{rest}</span>);
      break;
    }
    if (earliest.idx > 0) tokens.push(<span key={`${keyPrefix}-t-${key++}`}>{rest.slice(0, earliest.idx)}</span>);
    tokens.push(earliest.el);
    rest = rest.slice(earliest.idx + earliest.len);
  }
  return tokens;
}

// ----------------- syntax highlighting (lightweight) -----------------

// Hand-rolled tokenizer. We render each token as a <span> with a
// colour class. Output is wrapped in a <pre><code> and is safe
// because we operate on a string (not dangerouslySetInnerHTML).
function highlightCode(lang: string, src: string): JSX.Element[] {
  const norm = (lang || "").toLowerCase().trim();
  if (norm === "text" || norm === "" || norm === "txt" || norm === "log") {
    return [<span key="0">{src}</span>];
  }
  const rules = RULES[norm] ?? RULES["clike"];
  if (!rules) return [<span key="0">{src}</span>];
  const out: JSX.Element[] = [];
  let i = 0;
  let key = 0;
  const push = (cls: string, text: string) => {
    if (text === "") return;
    out.push(<span key={key++} className={cls}>{text}</span>);
  };
  while (i < src.length) {
    if (rules.lineComment) {
      const m = src.slice(i).match(rules.lineComment);
      if (m && m.index === 0) {
        const j = src.indexOf("\n", i);
        const end = j === -1 ? src.length : j;
        push("text-slate-500 italic", src.slice(i, end));
        i = end;
        continue;
      }
    }
    if (rules.blockComment) {
      const m = src.slice(i).match(rules.blockComment);
      if (m && m.index === 0) {
        const end = src.indexOf("*/", i + 2);
        const realEnd = end === -1 ? src.length : end + 2;
        push("text-slate-500 italic", src.slice(i, realEnd));
        i = realEnd;
        continue;
      }
    }
    if (rules.string) {
      const m = src.slice(i).match(rules.string);
      if (m && m.index === 0) {
        const next = m[0].length;
        push("text-emerald-300", src.slice(i, i + next));
        i += next;
        continue;
      }
    }
    const num = src.slice(i).match(/^\b\d+(\.\d+)?\b/);
    if (num && num.index === 0) {
      push("text-amber-300", num[0]);
      i += num[0].length;
      continue;
    }
    const id = src.slice(i).match(/^[A-Za-z_][A-Za-z0-9_]*/);
    if (id && id.index === 0) {
      const w = id[0];
      if (rules.keywords.has(w)) push("text-fuchsia-300", w);
      else if (rules.builtins.has(w)) push("text-cyan2-300", w);
      else if (rules.constants.has(w)) push("text-amber-300", w);
      else if (rules.types.has(w)) push("text-sky-300", w);
      else if (/^[A-Z][A-Z0-9_]+$/.test(w)) push("text-amber-300", w);
      else push("text-slate-200", w);
      i += w.length;
      continue;
    }
    const ch = src[i];
    if (/[{}\[\]()=+\-*/%<>!&|^~?:;,.\\]/.test(ch)) {
      push("text-slate-400", ch);
      i += 1;
      continue;
    }
    push("", ch);
    i += 1;
  }
  return out;
}

type LangRules = {
  lineComment?: RegExp;
  blockComment?: RegExp;
  string?: RegExp;
  keywords: Set<string>;
  builtins: Set<string>;
  constants: Set<string>;
  types: Set<string>;
};

const KW_PY = new Set(["def", "return", "if", "elif", "else", "for", "while", "in", "is", "not", "and", "or", "import", "from", "as", "class", "try", "except", "finally", "raise", "with", "yield", "lambda", "pass", "break", "continue", "global", "nonlocal", "del", "assert", "async", "await", "match", "case"]);
const BI_PY = new Set(["print", "len", "range", "enumerate", "zip", "map", "filter", "sorted", "list", "dict", "set", "tuple", "str", "int", "float", "bool", "open", "isinstance", "hasattr", "getattr", "setattr", "type", "super"]);
const CONST_PY = new Set(["None", "True", "False"]);
const TYPE_PY = new Set(["int", "str", "float", "bool", "list", "dict", "set", "tuple", "Optional", "Any", "Callable", "Iterator", "Iterable", "Union"]);

const KW_JS = new Set(["function", "return", "if", "else", "for", "while", "do", "switch", "case", "break", "continue", "const", "let", "var", "new", "this", "class", "extends", "super", "import", "export", "from", "as", "default", "try", "catch", "finally", "throw", "typeof", "instanceof", "in", "of", "yield", "async", "await", "void", "delete"]);
const BI_JS = new Set(["console", "window", "document", "Math", "JSON", "Object", "Array", "Promise", "Date", "RegExp", "Map", "Set", "Symbol", "Error", "Number", "String", "Boolean", "parseInt", "parseFloat"]);
const CONST_JS = new Set(["true", "false", "null", "undefined", "NaN", "Infinity"]);
const TYPE_JS = new Set(["string", "number", "boolean", "any", "unknown", "never", "void", "object", "Array", "Record", "Partial", "Pick", "Omit", "Map", "Set", "Promise"]);

const KW_RS = new Set(["fn", "let", "mut", "const", "static", "pub", "mod", "use", "crate", "self", "super", "impl", "trait", "struct", "enum", "match", "if", "else", "for", "while", "loop", "return", "break", "continue", "as", "where", "move", "ref", "async", "await", "dyn", "true", "false", "extern", "type", "unsafe", "in"]);
const BI_RS = new Set(["Vec", "String", "Box", "Rc", "Arc", "Option", "Result", "Ok", "Err", "Some", "None", "HashMap", "HashSet", "BTreeMap", "BTreeSet", "println", "print", "format", "panic", "vec", "Self"]);
const CONST_RS = new Set(["true", "false", "None", "Some", "Ok", "Err"]);

const KW_GO = new Set(["func", "return", "if", "else", "for", "range", "switch", "case", "default", "break", "continue", "var", "const", "type", "struct", "interface", "map", "chan", "go", "defer", "select", "package", "import", "fallthrough", "goto", "true", "false", "nil"]);
const BI_GO = new Set(["make", "len", "cap", "append", "copy", "delete", "new", "panic", "recover", "print", "println", "fmt", "context", "error", "string", "int", "int64", "float64", "bool", "byte", "rune"]);

const KW_BASH = new Set(["if", "then", "else", "elif", "fi", "for", "while", "do", "done", "case", "esac", "in", "function", "return", "export", "local", "set", "unset", "echo", "cd", "pwd", "ls", "cat", "grep", "sed", "awk", "test", "true", "false"]);
const BI_BASH = new Set(["PATH", "HOME", "PWD", "SHELL", "USER", "LANG"]);

const KW_SQL = new Set(["select", "from", "where", "group", "by", "having", "order", "limit", "offset", "join", "left", "right", "inner", "outer", "full", "on", "as", "and", "or", "not", "in", "is", "null", "insert", "into", "values", "update", "set", "delete", "create", "table", "index", "view", "drop", "alter", "add", "column", "primary", "key", "foreign", "references", "distinct", "union", "all", "case", "when", "then", "else", "end", "with", "true", "false"]);

const RULES: Record<string, LangRules> = {
  python:     { lineComment: /^#[^\n]*/, string: /^("(?:\\.|[^"\\])*"|'(?:\\.|[^'\\])*')/, keywords: KW_PY, builtins: BI_PY, constants: CONST_PY, types: TYPE_PY },
  py:         { lineComment: /^#[^\n]*/, string: /^("(?:\\.|[^"\\])*"|'(?:\\.|[^'\\])*')/, keywords: KW_PY, builtins: BI_PY, constants: CONST_PY, types: TYPE_PY },
  javascript: { lineComment: /^\/\/[^\n]*/, blockComment: /^\/\*[\s\S]*?\*\//, string: /^(`(?:\\.|[^`\\])*`|"(?:\\.|[^"\\])*"|'(?:\\.|[^'\\])*')/, keywords: KW_JS, builtins: BI_JS, constants: CONST_JS, types: TYPE_JS },
  js:         { lineComment: /^\/\/[^\n]*/, blockComment: /^\/\*[\s\S]*?\*\//, string: /^(`(?:\\.|[^`\\])*`|"(?:\\.|[^"\\])*"|'(?:\\.|[^'\\])*')/, keywords: KW_JS, builtins: BI_JS, constants: CONST_JS, types: TYPE_JS },
  jsx:        { lineComment: /^\/\/[^\n]*/, blockComment: /^\/\*[\s\S]*?\*\//, string: /^(`(?:\\.|[^`\\])*`|"(?:\\.|[^"\\])*"|'(?:\\.|[^'\\])*')/, keywords: KW_JS, builtins: BI_JS, constants: CONST_JS, types: TYPE_JS },
  typescript: { lineComment: /^\/\/[^\n]*/, blockComment: /^\/\*[\s\S]*?\*\//, string: /^(`(?:\\.|[^`\\])*`|"(?:\\.|[^"\\])*"|'(?:\\.|[^'\\])*')/, keywords: KW_JS, builtins: BI_JS, constants: CONST_JS, types: TYPE_JS },
  ts:         { lineComment: /^\/\/[^\n]*/, blockComment: /^\/\*[\s\S]*?\*\//, string: /^(`(?:\\.|[^`\\])*`|"(?:\\.|[^"\\])*"|'(?:\\.|[^'\\])*')/, keywords: KW_JS, builtins: BI_JS, constants: CONST_JS, types: TYPE_JS },
  tsx:        { lineComment: /^\/\/[^\n]*/, blockComment: /^\/\*[\s\S]*?\*\//, string: /^(`(?:\\.|[^`\\])*`|"(?:\\.|[^"\\])*"|'(?:\\.|[^'\\])*')/, keywords: KW_JS, builtins: BI_JS, constants: CONST_JS, types: TYPE_JS },
  rust:       { lineComment: /^\/\/[^\n]*/, blockComment: /^\/\*[\s\S]*?\*\//, string: /^("(?:\\.|[^"\\])*"|b?"(?:\\.|[^"\\])*")/, keywords: KW_RS, builtins: BI_RS, constants: CONST_RS, types: new Set() },
  rs:         { lineComment: /^\/\/[^\n]*/, blockComment: /^\/\*[\s\S]*?\*\//, string: /^("(?:\\.|[^"\\])*"|b?"(?:\\.|[^"\\])*")/, keywords: KW_RS, builtins: BI_RS, constants: CONST_RS, types: new Set() },
  go:         { lineComment: /^\/\/[^\n]*/, blockComment: /^\/\*[\s\S]*?\*\//, string: /^(`(?:\\.|[^`\\])*`|"(?:\\.|[^"\\])*")/, keywords: KW_GO, builtins: BI_GO, constants: new Set(), types: new Set() },
  bash:       { lineComment: /^\s*#[^\n]*/, string: /^("(?:\\.|[^"\\])*"|'(?:[^'\\]|\\.)*')/, keywords: KW_BASH, builtins: BI_BASH, constants: new Set(), types: new Set() },
  sh:         { lineComment: /^\s*#[^\n]*/, string: /^("(?:\\.|[^"\\])*"|'(?:[^'\\]|\\.)*')/, keywords: KW_BASH, builtins: BI_BASH, constants: new Set(), types: new Set() },
  json:       { string: /^("(?:\\.|[^"\\])*")/, keywords: new Set(), builtins: new Set(), constants: new Set(["true", "false", "null"]), types: new Set() },
  yaml:       { lineComment: /^\s*#[^\n]*/, string: /^("(?:\\.|[^"\\])*"|'(?:[^'\\]|\\.)*')/, keywords: new Set(), builtins: new Set(), constants: new Set(["true", "false", "null", "yes", "no"]), types: new Set() },
  yml:        { lineComment: /^\s*#[^\n]*/, string: /^("(?:\\.|[^"\\])*"|'(?:[^'\\]|\\.)*')/, keywords: new Set(), builtins: new Set(), constants: new Set(["true", "false", "null", "yes", "no"]), types: new Set() },
  toml:       { lineComment: /^\s*#[^\n]*/, string: /^("(?:\\.|[^"\\])*"|'(?:[^'\\]|\\.)*')/, keywords: new Set(), builtins: new Set(), constants: new Set(["true", "false"]), types: new Set() },
  sql:        { lineComment: /^--[^\n]*/, blockComment: /^\/\*[\s\S]*?\*\//, string: /^('(?:[^'\\]|\\.)*')/, keywords: KW_SQL, builtins: new Set(), constants: new Set(), types: new Set() },
  html:       { blockComment: /^<!--[\s\S]*?-->/, string: /^("[^"]*"|'[^']*')/, keywords: new Set(), builtins: new Set(), constants: new Set(), types: new Set() },
  css:        { blockComment: /^\/\*[\s\S]*?\*\//, string: /^("[^"]*"|'[^']*')/, keywords: new Set(), builtins: new Set(), constants: new Set(), types: new Set() },
  clike:      { lineComment: /^\/\/[^\n]*/, blockComment: /^\/\*[\s\S]*?\*\//, string: /^("[^"]*"|'[^']*')/, keywords: new Set(), builtins: new Set(), constants: new Set(), types: new Set() },
};

// ----------------- main view -----------------

export function MarkdownView({ text }: Props) {
  const blocks = parseBlocks(text);
  return (
    <div className="space-y-3">
      {blocks.map((b, i) => renderBlock(b, i))}
    </div>
  );
}

function renderBlock(b: Block, i: number): ReactNode {
  if (b.kind === "p") {
    return <p key={i} className="leading-relaxed text-slate-200">{renderInline(b.text, `p${i}`)}</p>;
  }
  if (b.kind === "h") {
    const Tag = (`h${b.level}`) as "h1" | "h2" | "h3";
    const cls = b.level === 1 ? "text-lg" : b.level === 2 ? "text-base" : "text-sm";
    return <Tag key={i} className={`${cls} font-semibold text-slate-100 mt-2`}>{renderInline(b.text, `h${i}`)}</Tag>;
  }
  if (b.kind === "ul") {
    return (
      <ul key={i} className="list-disc pl-6 space-y-1 text-slate-200">
        {b.items.map((it, j) => <li key={j}>{renderInline(it, `ul${i}-${j}`)}</li>)}
      </ul>
    );
  }
  if (b.kind === "ol") {
    return (
      <ol key={i} className="list-decimal pl-6 space-y-1 text-slate-200">
        {b.items.map((it, j) => <li key={j}>{renderInline(it, `ol${i}-${j}`)}</li>)}
      </ol>
    );
  }
  if (b.kind === "task") {
    return (
      <ul key={i} className="space-y-1 text-slate-200">
        {b.items.map((t, j) => (
          <li key={j} className="flex items-start gap-2">
            <input type="checkbox" checked={t.checked} readOnly className="mt-1" aria-label={t.checked ? "Done" : "Todo"} />
            <span className={t.checked ? "line-through text-slate-500" : ""}>{renderInline(t.text, `task${i}-${j}`)}</span>
          </li>
        ))}
      </ul>
    );
  }
  if (b.kind === "quote") {
    return <blockquote key={i} className="border-l-2 border-cyan2-500/40 pl-3 text-slate-300 italic">{renderInline(b.text, `q${i}`)}</blockquote>;
  }
  if (b.kind === "hr") {
    return <hr key={i} className="border-white/5" />;
  }
  if (b.kind === "table") {
    return <Table key={i} head={b.head} rows={b.rows} align={b.align} />;
  }
  if (b.kind === "code") {
    return <CodeBlock key={i} lang={b.lang} text={b.text} />;
  }
  return null;
}

function Table({ head, rows, align }: { head: string[]; rows: string[][]; align: Array<"left" | "right" | "center" | null> }) {
  return (
    <div className="rounded-lg border border-white/5 overflow-x-auto">
      <table className="w-full text-sm">
        <thead className="bg-ink-900/80 text-slate-300">
          <tr>
            {head.map((c, j) => (
              <th key={j} className={`py-2 px-3 font-medium ${alignClass(align[j])}`}>{renderInline(c, `th${j}`)}</th>
            ))}
          </tr>
        </thead>
        <tbody>
          {rows.map((r, j) => (
            <tr key={j} className="border-t border-white/5 hover:bg-white/[0.02]">
              {r.map((c, k) => (
                <td key={k} className={`py-2 px-3 text-slate-200 ${alignClass(align[k] ?? null)}`}>{renderInline(c, `tr${j}-${k}`)}</td>
              ))}
            </tr>
          ))}
        </tbody>
      </table>
    </div>
  );
}

function alignClass(a: "left" | "right" | "center" | null): string {
  if (a === "right") return "text-right";
  if (a === "center") return "text-center";
  return "text-left";
}

function CodeBlock({ lang, text }: { lang: string; text: string }) {
  const [copied, setCopied] = useState(false);
  const [expanded, setExpanded] = useState(false);
  const onCopy = () => {
    navigator.clipboard.writeText(text).then(() => {
      setCopied(true);
      setTimeout(() => setCopied(false), 1200);
    });
  };
  const lineCount = text.split("\n").length;
  const needsExpand = lineCount > 18;
  const displayText = needsExpand && !expanded ? text.split("\n").slice(0, 18).join("\n") : text;
  return (
    <div className="rounded-lg border border-white/5 bg-ink-950/80 overflow-hidden">
      <div className="flex items-center justify-between px-3 py-1.5 border-b border-white/5 text-[11px] text-slate-500">
        <span className="font-mono">{lang || "text"}</span>
        <div className="flex items-center gap-2">
          {needsExpand && (
            <button
              type="button"
              className="kbd"
              onClick={() => setExpanded((v) => !v)}
              aria-label={expanded ? "Collapse code block" : "Expand code block"}
            >
              {expanded ? "Collapse" : `Show all (${lineCount})`}
            </button>
          )}
          <button className="kbd" onClick={onCopy} aria-label="Copy code">{copied ? "Copied" : "Copy"}</button>
        </div>
      </div>
      <pre className="p-3 text-[12.5px] leading-relaxed font-mono overflow-x-auto">
        <code>{highlightCode(lang, displayText)}</code>
      </pre>
      {needsExpand && !expanded && (
        <div className="border-t border-white/5 text-center">
          <button
            type="button"
            className="text-[11px] text-slate-400 hover:text-cyan2-400 py-1.5 w-full"
            onClick={() => setExpanded(true)}
          >
            {lineCount - 18} more line{lineCount - 18 === 1 ? "" : "s"}…
          </button>
        </div>
      )}
    </div>
  );
}

// Convenience hook that debounces the markdown view when text is
// streaming so the user sees content even at 60+ tokens/second.
export function useStableMarkdown(text: string, intervalMs = 60): string {
  const [stable, setStable] = useState(text);
  useEffect(() => {
    const id = setTimeout(() => setStable(text), intervalMs);
    return () => clearTimeout(id);
  }, [text, intervalMs]);
  return stable;
}
