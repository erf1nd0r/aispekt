import type { CheckContext, CheckFn, RawFinding } from "../types";
import { excerptOf, normalizeLine, wordSet, jaccard } from "../parse";

const LINT_LEAKAGE =
  /\b(\d+[- ]spaces?\b|spaces? (for|per) indent|indent(ation)? (with|using|of|is) |tabs?,? not spaces|spaces?,? not tabs|single quotes|double quotes|semicolons?\b|trailing commas?|max(imum)? line (length|width)|line length (of|under|max)|alphabetical(ly)? (order|sorted)|import order|sort(ed)? imports)/i;

const VAGUE_PHRASES = [
  "write clean code",
  "clean, professional code",
  "clean and professional",
  "best practices",
  "be careful",
  "use common sense",
  "use good judgment",
  "handle errors appropriately",
  "handle errors properly",
  "handle errors gracefully",
  "as appropriate",
  "when appropriate",
  "as needed",
  "high quality code",
  "high-quality code",
  "meaningful names",
  "self-documenting",
  "keep it simple",
  "professional code",
  "robust code",
  "well-tested code",
];

const COMMAND_TOKEN =
  /(^|[\s$>])(bun|bunx|npm|npx|yarn|pnpm|make|cargo|pytest|uv|pip3?|docker|git|wrangler|tsc|vite|eslint|ruff|prettier|go|swift|python3?|gradle|mvn|rake|mix|dotnet|\.\/[\w.-]+)\b/;

const SECRET_PATTERNS: { re: RegExp; label: string }[] = [
  { re: /sk-ant-[A-Za-z0-9_-]{10,}/, label: "Anthropic API key" },
  { re: /sk-[A-Za-z0-9]{20,}/, label: "API key (sk-...)" },
  { re: /ghp_[A-Za-z0-9]{20,}/, label: "GitHub personal access token" },
  { re: /github_pat_[A-Za-z0-9_]{20,}/, label: "GitHub fine-grained PAT" },
  { re: /AKIA[0-9A-Z]{16}/, label: "AWS access key id" },
  { re: /xox[baprs]-[A-Za-z0-9-]{10,}/, label: "Slack token" },
  { re: /-----BEGIN (RSA |EC |OPENSSH )?PRIVATE KEY-----/, label: "private key block" },
  { re: /eyJ[A-Za-z0-9_-]{20,}\.[A-Za-z0-9_-]{10,}\.[A-Za-z0-9_-]{10,}/, label: "JWT" },
];

const EMPHASIS_RE = /\b(IMPORTANT|YOU MUST|CRITICAL|NEVER EVER)\b/g;
const EMPHASIS_LIMIT = 5;

const DOC_REF_RE = /(@[\w./-]+\.md\b|\[[^\]]*\]\([^)]*\.md[^)]*\)|(?<![\w/])[\w-]+\/[\w./-]*\.md\b)/;
const READ_IMPERATIVE_RE = /\b(read|consult|follow|load|review|before starting|IMPORTANT)\b/i;

function proseLines(ctx: CheckContext): { i: number; line: string }[] {
  const out: { i: number; line: string }[] = [];
  for (let i = 0; i < ctx.lines.length; i++) {
    if (ctx.prose[i]) out.push({ i, line: ctx.lines[i] ?? "" });
  }
  return out;
}

export const lintLeakage: CheckFn = (ctx) => {
  const out: RawFinding[] = [];
  for (const { i, line } of proseLines(ctx)) {
    if (line.trim().length === 0) continue;
    if (LINT_LEAKAGE.test(line)) {
      out.push({
        line: i + 1,
        excerpt: excerptOf(line),
        message: "Formatter-enforceable style rule stated as prose.",
      });
    }
  }
  return out;
};

export const directoryTree: CheckFn = (ctx) => {
  const out: RawFinding[] = [];
  let blockStart = -1;
  let run = 0;
  for (let i = 0; i < ctx.lines.length; i++) {
    const line = ctx.lines[i] ?? "";
    const treeish =
      /[├└│]/.test(line) || /^\s{0,8}[\w.@-]+\/\s*(#.*)?$/.test(line);
    if (treeish) {
      if (run === 0) blockStart = i;
      run++;
    } else {
      if (run >= 3) {
        out.push({
          line: blockStart + 1,
          excerpt: excerptOf(ctx.lines[blockStart] ?? ""),
          message: `Directory-tree block of ${run} lines.`,
        });
      }
      run = 0;
    }
  }
  if (run >= 3 && blockStart >= 0) {
    out.push({
      line: blockStart + 1,
      excerpt: excerptOf(ctx.lines[blockStart] ?? ""),
      message: `Directory-tree block of ${run} lines.`,
    });
  }
  return out;
};

export const vagueRules: CheckFn = (ctx) => {
  const out: RawFinding[] = [];
  for (const { i, line } of proseLines(ctx)) {
    const lower = line.toLowerCase();
    if (line.includes("`")) continue; // concrete artifact present
    const hit = VAGUE_PHRASES.find((p) => lower.includes(p));
    if (hit) {
      out.push({
        line: i + 1,
        excerpt: excerptOf(line),
        message: `Vague phrase "${hit}" — nothing testable for the agent.`,
      });
    }
  }
  return out;
};

export const noCommands: CheckFn = (ctx) => {
  for (const span of ctx.codeSpans) {
    if (COMMAND_TOKEN.test(span.text)) return [];
  }
  return [
    {
      line: 1,
      excerpt: excerptOf(ctx.lines[0] ?? ""),
      message: "No runnable command found anywhere in the file.",
    },
  ];
};

export const enforcementProse: CheckFn = (ctx) => {
  const out: RawFinding[] = [];
  for (const { i, line } of proseLines(ctx)) {
    if (/secret/i.test(line)) continue; // "never commit secrets" is the canonical good boundary
    if (!/\b(never|always|must)\b/i.test(line)) continue;
    if (
      /\b(commit|push|force[- ]push|merge|deploy|format|lint|console\.log|debugger|\.env|prettier|eslint|typecheck|any\b)/i.test(
        line,
      )
    ) {
      out.push({
        line: i + 1,
        excerpt: excerptOf(line),
        message: "Zero-exception rule expressed as advisory prose.",
      });
    }
  }
  return out;
};

export const secretLeak: CheckFn = (ctx) => {
  const out: RawFinding[] = [];
  for (let i = 0; i < ctx.lines.length; i++) {
    const line = ctx.lines[i] ?? "";
    for (const { re, label } of SECRET_PATTERNS) {
      if (re.test(line)) {
        out.push({
          line: i + 1,
          excerpt: excerptOf(line.replace(re, (m) => m.slice(0, 8) + "…[redacted]")),
          message: `Token-shaped string: ${label}.`,
        });
        break;
      }
    }
  }
  return out;
};

export const emphasisOveruse: CheckFn = (ctx) => {
  let count = 0;
  let overflowLine = 0;
  for (let i = 0; i < ctx.lines.length; i++) {
    if (!ctx.prose[i]) continue;
    const matches = (ctx.lines[i] ?? "").match(EMPHASIS_RE);
    if (matches) {
      count += matches.length;
      if (count > EMPHASIS_LIMIT && overflowLine === 0) overflowLine = i + 1;
    }
  }
  if (count > EMPHASIS_LIMIT) {
    return [
      {
        line: overflowLine,
        excerpt: excerptOf(ctx.lines[overflowLine - 1] ?? ""),
        message: `${count} emphasis markers (IMPORTANT/YOU MUST/CRITICAL) — past ~${EMPHASIS_LIMIT} they cancel out.`,
      },
    ];
  }
  return [];
};

export const pointerWithoutInstruction: CheckFn = (ctx) => {
  const refLines: number[] = [];
  for (const { i, line } of proseLines(ctx)) {
    if (DOC_REF_RE.test(line)) refLines.push(i);
  }
  // cluster: >=3 doc-ref lines within an 8-line window
  for (let a = 0; a + 2 < refLines.length; a++) {
    const start = refLines[a] ?? 0;
    const third = refLines[a + 2] ?? 0;
    if (third - start <= 8) {
      const lo = Math.max(0, start - 3);
      const hi = Math.min(ctx.lines.length - 1, third + 1);
      let hasImperative = false;
      for (let i = lo; i <= hi; i++) {
        if (READ_IMPERATIVE_RE.test(ctx.lines[i] ?? "")) {
          hasImperative = true;
          break;
        }
      }
      if (!hasImperative) {
        return [
          {
            line: start + 1,
            excerpt: excerptOf(ctx.lines[start] ?? ""),
            message: "Cluster of doc links with no instruction to read them.",
          },
        ];
      }
      return [];
    }
  }
  return [];
};

export const sizeCost: CheckFn = (ctx) => {
  if (ctx.lines.length > 200) {
    return [
      {
        line: 201,
        excerpt: excerptOf(ctx.lines[200] ?? ""),
        message: `${ctx.lines.length} lines — Anthropic's target is under 200. (Cost argument, not adherence — see the cited caveat.)`,
      },
    ];
  }
  return [];
};

export const duplicateRules: CheckFn = (ctx) => {
  const seen = new Map<string, number>();
  const out: RawFinding[] = [];
  for (const { i, line } of proseLines(ctx)) {
    const norm = normalizeLine(line);
    if (norm.length < 20) continue;
    const first = seen.get(norm);
    if (first !== undefined) {
      out.push({
        line: i + 1,
        excerpt: excerptOf(line),
        message: `Duplicates line ${first + 1}.`,
      });
    } else {
      seen.set(norm, i);
    }
  }
  return out;
};

export const contradictionLite: CheckFn = (ctx) => {
  const out: RawFinding[] = [];
  const prose = proseLines(ctx);
  const tabLine = prose.find(({ line }) => /\btabs?\b/i.test(line) && /indent/i.test(line));
  const spaceLine = prose.find(
    ({ line }) => /\b\d+[- ]spaces?\b|spaces? for indent/i.test(line) && /indent/i.test(line),
  );
  if (tabLine && spaceLine && tabLine.i !== spaceLine.i) {
    out.push({
      line: Math.max(tabLine.i, spaceLine.i) + 1,
      excerpt: excerptOf(ctx.lines[Math.max(tabLine.i, spaceLine.i)] ?? ""),
      message: `Tabs (line ${tabLine.i + 1}) vs spaces (line ${spaceLine.i + 1}) for indentation.`,
    });
  }
  const managers = new Map<string, number>();
  for (const span of ctx.codeSpans) {
    const m = span.text.match(/\b(npm|yarn|pnpm|bun)\b(?= (run|install|test|add|create|x)\b| )/);
    if (m?.[1] && !managers.has(m[1])) managers.set(m[1], span.line);
  }
  if (managers.size >= 2) {
    const names = [...managers.keys()].sort();
    const line = Math.max(...managers.values());
    out.push({
      line,
      excerpt: excerptOf(ctx.lines[line - 1] ?? ""),
      message: `Mixed package managers in commands: ${names.join(", ")}.`,
    });
  }
  return out;
};

export const boundariesMissing: CheckFn = (ctx) => {
  for (const { line } of proseLines(ctx)) {
    if (/\b(never|don'?t|do not|must not|ask (first|before))\b/i.test(line)) return [];
  }
  return [
    {
      line: 1,
      excerpt: excerptOf(ctx.lines[0] ?? ""),
      message: "No never/ask-first boundary found.",
    },
  ];
};

export const codexCap: CheckFn = (ctx) => {
  const bytes = new TextEncoder().encode(ctx.content).length;
  if (bytes > 32 * 1024) {
    return [
      {
        line: 1,
        excerpt: `${(bytes / 1024).toFixed(1)} KiB`,
        message: `File is ${(bytes / 1024).toFixed(1)} KiB — Codex silently truncates at 32 KiB.`,
      },
    ];
  }
  return [];
};

/** README-redundancy lives here for reuse but is a repo-scoped rule. */
export function redundantWith(
  line: string,
  readmeNorms: { norm: string; words: Set<string> }[],
): boolean {
  const norm = normalizeLine(line);
  if (norm.length < 40) return false;
  const words = wordSet(norm);
  for (const r of readmeNorms) {
    if (r.norm === norm) return true;
    if (words.size >= 6 && jaccard(words, r.words) >= 0.8) return true;
  }
  return false;
}
