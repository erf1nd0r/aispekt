import type { CheckContext, RepoContext } from "./types";

/** Build the shared parsed view every check consumes. Pure and deterministic. */
/** Regex scans are quadratic on pathological single lines; cap what they see. */
const MAX_SCAN_LINE = 10_000;

export function buildContext(
  fileName: string,
  content: string,
  repo?: RepoContext,
): CheckContext {
  const lines = content
    .split("\n")
    .map((l) => (l.length > MAX_SCAN_LINE ? l.slice(0, MAX_SCAN_LINE) : l));
  const prose: boolean[] = new Array(lines.length).fill(true);
  const codeSpans: { line: number; text: string }[] = [];

  let inFence = false;
  for (let i = 0; i < lines.length; i++) {
    const line = lines[i] ?? "";
    if (/^\s*(```|~~~)/.test(line)) {
      inFence = !inFence;
      prose[i] = false;
      continue;
    }
    if (inFence) {
      prose[i] = false;
      codeSpans.push({ line: i + 1, text: line });
      continue;
    }
    for (const m of line.matchAll(/`([^`]+)`/g)) {
      codeSpans.push({ line: i + 1, text: m[1] ?? "" });
    }
  }

  return { fileName, content, lines, prose, codeSpans, repo };
}

export function excerptOf(line: string, max = 120): string {
  const t = line.trim();
  return t.length <= max ? t : t.slice(0, max - 1) + "…";
}

/** ~4 chars per token — the standard rough estimate for English/markdown. */
export function estimateTokens(content: string): number {
  return Math.ceil(content.length / 4);
}

export function normalizeLine(line: string): string {
  return line
    .replace(/^[\s\-*+>]*(\d+[.)])?\s*/, "")
    .toLowerCase()
    .replace(/[`*_~"'!?.,:;()[\]]/g, "")
    .replace(/\s+/g, " ")
    .trim();
}

export function wordSet(s: string): Set<string> {
  return new Set(s.split(" ").filter((w) => w.length > 2));
}

export function jaccard(a: Set<string>, b: Set<string>): number {
  if (a.size === 0 || b.size === 0) return 0;
  let inter = 0;
  for (const w of a) if (b.has(w)) inter++;
  return inter / (a.size + b.size - inter);
}

/** Normalize a repo-relative path reference for lookup. */
export function normalizePath(p: string): string {
  return p.replace(/^\.\//, "").replace(/\/+$/, "");
}

/**
 * Strip shared leading directories until one of rootMarkers sits at root.
 * Browser folder drops include the dropped folder's own name as the first
 * segment of every path; a repo root must not carry that prefix.
 */
export function stripCommonRoot(
  paths: string[],
  rootMarkers: string[],
): { paths: string[]; prefix: string } {
  let current = paths;
  let prefix = "";
  while (
    current.length > 0 &&
    !rootMarkers.some((m) => current.includes(m)) &&
    current.every((p) => p.includes("/"))
  ) {
    const firsts = new Set(current.map((p) => p.split("/")[0] ?? ""));
    if (firsts.size !== 1) break;
    const seg = [...firsts][0];
    if (!seg) break;
    current = current.map((p) => p.split("/").slice(1).join("/"));
    prefix += seg + "/";
  }
  return { paths: current, prefix };
}
