import type { CheckContext, CheckFn, RawFinding } from "../types";
import { excerptOf, normalizeLine, normalizePath, wordSet } from "../parse";
import { redundantWith } from "./file-checks";

function repoOf(ctx: CheckContext) {
  return ctx.repo;
}

/**
 * True when the reference is present — or cannot be proven absent. Entries
 * with a trailing "/" are unverifiable-directory markers (dirs the walker
 * deliberately skipped, e.g. dist/, node_modules/): anything under them
 * passes. Comparison is case-insensitive (macOS default fs). A reference
 * that exists as a suffix of some real path (context-relative mention like
 * `lib/route.ts` for client/src/lib/route.ts) also passes — flagging only
 * what is confidently absent.
 */
function pathExists(repo: { paths: string[] }, ref: string): boolean {
  const p = normalizePath(ref).toLowerCase();
  if (p.length === 0) return true;
  for (const existing of repo.paths) {
    const isDirMarker = existing.endsWith("/");
    const e = normalizePath(existing).toLowerCase();
    if (e === p || e.startsWith(p + "/")) return true;
    if (isDirMarker && p.startsWith(e + "/")) return true;
    if (e.endsWith("/" + p)) return true;
  }
  return false;
}

function repoContent(repo: { contents: Record<string, string> }, name: string): string | undefined {
  for (const key of Object.keys(repo.contents)) {
    if (normalizePath(key).toLowerCase() === name.toLowerCase()) return repo.contents[key];
  }
  return undefined;
}

export const deadCommand: CheckFn = (ctx) => {
  const repo = repoOf(ctx);
  if (!repo) return [];
  const pkgRaw = repoContent(repo, "package.json");
  if (pkgRaw === undefined) return [];
  let scripts: Record<string, unknown> = {};
  try {
    const pkg = JSON.parse(pkgRaw) as { scripts?: Record<string, unknown> };
    scripts = pkg.scripts ?? {};
  } catch {
    return [];
  }
  const out: RawFinding[] = [];
  const reported = new Set<string>();
  for (const span of ctx.codeSpans) {
    for (const m of span.text.matchAll(/\b(?:bun|npm|pnpm|yarn) run ([\w:.-]+)/g)) {
      const script = m[1] ?? "";
      if (script && !(script in scripts) && !reported.has(script)) {
        reported.add(script);
        out.push({
          line: span.line,
          excerpt: excerptOf(span.text),
          message: `Script "${script}" is not in package.json scripts.`,
        });
      }
    }
  }
  return out;
};

const FILE_EXTENSIONS =
  /^(md|markdown|json|ts|js|tsx|jsx|mjs|cjs|toml|yaml|yml|txt|css|html|svg|png|jpg|lock|env|sh|py|rs|go|swift)$/i;

/** "fives.gugus.gg" is a hostname; "wrangler.toml" and ".github" are not. */
function isHostnameLike(segment: string): boolean {
  if (!segment.includes(".") || segment.startsWith(".")) return false;
  const last = segment.split(".").pop() ?? "";
  return /^[a-z]{2,}$/i.test(last) && !FILE_EXTENSIONS.test(last);
}

export const stalePath: CheckFn = (ctx) => {
  const repo = repoOf(ctx);
  // A truncated walk cannot prove absence — never flag from a partial list.
  if (!repo || repo.paths.length === 0 || repo.truncated) return [];
  const out: RawFinding[] = [];
  const reported = new Set<string>();
  for (const span of ctx.codeSpans) {
    const token = span.text.trim();
    // path-like: contains a slash, no spaces, no URL, no glob
    if (
      !/^\.{0,2}\/?[\w@.-]+(\/[\w@.-]+)+\/?$/.test(token) ||
      token.includes("://")
    ) {
      continue;
    }
    // npm package specifiers (@scope/pkg/subpath) are module ids, not files
    if (token.startsWith("@")) continue;
    // hostname-like first segment (fives.gugus.gg/play) is a URL, not a path
    if (!token.startsWith("./") && isHostnameLike(normalizePath(token).split("/")[0] ?? "")) {
      continue;
    }
    const norm = normalizePath(token);
    // trailing bare version segment = identifier (git tag, package route),
    // not a file path: shopnavigation/mainnavigationpath/1.2.3
    const lastSeg = norm.split("/").pop() ?? "";
    if (/^v?\d+([.-]\d+)*$/.test(lastSeg)) continue;
    if (reported.has(norm)) continue;
    if (!pathExists(repo, norm)) {
      reported.add(norm);
      out.push({
        line: span.line,
        excerpt: excerptOf(token),
        message: `Path "${norm}" does not exist in the repo.`,
      });
    }
  }
  return out;
};

export const readmeRedundancy: CheckFn = (ctx) => {
  const repo = repoOf(ctx);
  if (!repo) return [];
  const readme = repoContent(repo, "README.md");
  if (readme === undefined) return [];
  const readmeNorms = readme
    .split("\n")
    .map((l) => normalizeLine(l))
    .filter((n) => n.length >= 40)
    .map((norm) => ({ norm, words: wordSet(norm) }));
  if (readmeNorms.length === 0) return [];
  const out: RawFinding[] = [];
  for (let i = 0; i < ctx.lines.length && out.length < 10; i++) {
    if (!ctx.prose[i]) continue;
    const line = ctx.lines[i] ?? "";
    if (redundantWith(line, readmeNorms)) {
      out.push({
        line: i + 1,
        excerpt: excerptOf(line),
        message: "Restates the README the agent already reads.",
      });
    }
  }
  return out;
};

export const bridgeMissing: CheckFn = (ctx) => {
  const repo = repoOf(ctx);
  if (!repo) return [];
  const hasAgents = repo.paths.some((p) => normalizePath(p).toLowerCase() === "agents.md");
  if (!hasAgents) return [];
  const hasClaude = repo.paths.some((p) => normalizePath(p).toLowerCase() === "claude.md");
  if (!hasClaude) {
    if (repo.symlinksHidden) {
      return [
        {
          line: 1,
          excerpt: "AGENTS.md present, CLAUDE.md not visible",
          message:
            "No CLAUDE.md is visible, so Claude Code may never read this AGENTS.md. Browsers hide symlinks from folder drops, so a symlinked CLAUDE.md would not show up here.",
          recommendation:
            "If CLAUDE.md already exists as a symlink, this is a false alarm — confirm with the CLI, which follows symlinks. If there is truly no CLAUDE.md, add a one-line one containing `@AGENTS.md`.",
        },
      ];
    }
    return [
      {
        line: 1,
        excerpt: "AGENTS.md present, CLAUDE.md absent",
        message: "AGENTS.md exists but Claude Code will never read it.",
      },
    ];
  }
  const claude = repoContent(repo, "CLAUDE.md");
  const agents = repoContent(repo, "AGENTS.md");
  // A symlinked or copied CLAUDE.md reads back identical to AGENTS.md — bridged.
  if (claude !== undefined && agents !== undefined && claude === agents) return [];
  if (claude !== undefined && !/@AGENTS\.md\b/.test(claude)) {
    return [
      {
        line: 1,
        excerpt: "CLAUDE.md lacks @AGENTS.md",
        message: "CLAUDE.md exists but does not import @AGENTS.md — the two files can drift.",
      },
    ];
  }
  return [];
};

export const importResolution: CheckFn = (ctx) => {
  const repo = repoOf(ctx);
  if (!repo || repo.paths.length === 0 || repo.truncated) return [];
  const out: RawFinding[] = [];
  const reported = new Set<string>();
  for (let i = 0; i < ctx.lines.length; i++) {
    if (!ctx.prose[i]) continue;
    const line = ctx.lines[i] ?? "";
    for (const m of line.matchAll(/(?:^|\s)@((?:[\w.-]+\/)*[\w.-]+\.(?:md|json))\b/g)) {
      const ref = m[1] ?? "";
      if (ref.startsWith("~")) continue; // home-dir imports can't be verified against the repo
      const norm = normalizePath(ref);
      if (reported.has(norm)) continue;
      if (!pathExists(repo, norm)) {
        reported.add(norm);
        out.push({
          line: i + 1,
          excerpt: excerptOf(line),
          message: `@-import "${norm}" does not resolve in the repo.`,
        });
      }
    }
  }
  return out;
};
