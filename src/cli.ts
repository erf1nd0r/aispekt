/**
 * aispekt CLI — same engine and rulepack as the web app, real filesystem.
 *
 *   bun src/cli.ts <file-or-dir> [--json] [--min <score>]
 *
 * Exits 0 when score >= min (default 60), 1 below, 2 on usage/IO errors.
 */
import { readdirSync, readFileSync, statSync } from "node:fs";
import { join, relative, basename } from "node:path";

const MAX_FILES_TRUNCATED = { value: false };
import { analyze } from "./engine/analyze";
import { version as PKG_VERSION } from "../package.json";
import type { AnalysisInput, Report, RepoContext, Severity } from "./engine/types";

const SKIP_DIRS = new Set(["node_modules", ".git", "dist", "build", ".next", ".venv", "vendor", "coverage"]);
const MAX_FILES = 200_000;
const INSTRUCTION_FILES = ["CLAUDE.md", "AGENTS.md"];

/** Breadth-first: root files always enumerate before deep subtrees. */
function walk(root: string): string[] {
  const out: string[] = [];
  const queue = [root];
  MAX_FILES_TRUNCATED.value = false;
  while (queue.length > 0) {
    const dir = queue.shift()!;
    let entries;
    try {
      entries = readdirSync(dir, { withFileTypes: true });
    } catch {
      continue;
    }
    for (const e of entries) {
      const abs = join(dir, e.name);
      const rel = relative(root, abs);
      let isDir = e.isDirectory();
      let isFile = e.isFile();
      if (e.isSymbolicLink()) {
        // Follow file symlinks (a symlinked CLAUDE.md is a real bridge);
        // mark dir symlinks unverifiable instead of recursing (cycle-safe).
        try {
          const st = statSync(abs);
          if (st.isFile()) isFile = true;
          else if (st.isDirectory()) {
            out.push(rel + "/");
            continue;
          }
        } catch {
          continue; // broken symlink
        }
      }
      if (isDir) {
        if (e.name === ".git") continue;
        if (SKIP_DIRS.has(e.name)) {
          out.push(rel + "/"); // unverifiable-directory marker
          continue;
        }
        queue.push(abs);
      } else if (isFile) {
        if (out.length >= MAX_FILES) {
          MAX_FILES_TRUNCATED.value = true;
          return out.sort();
        }
        out.push(rel);
      }
    }
  }
  return out.sort();
}

function buildInput(target: string): AnalysisInput {
  const st = statSync(target);
  if (st.isFile()) {
    return { fileName: basename(target), content: readFileSync(target, "utf8") };
  }
  const paths = walk(target);
  const instruction = INSTRUCTION_FILES.find((f) => paths.includes(f));
  if (!instruction) {
    throw new Error(`no CLAUDE.md or AGENTS.md at the root of ${target}`);
  }
  const contents: Record<string, string> = {};
  for (const name of [...INSTRUCTION_FILES, "package.json", "README.md"]) {
    if (paths.includes(name)) contents[name] = readFileSync(join(target, name), "utf8");
  }
  const repo: RepoContext = { paths, contents, truncated: MAX_FILES_TRUNCATED.value };
  return { fileName: instruction, content: contents[instruction] ?? "", repo };
}

const COLORS: Record<Severity, string> = { error: "\x1b[31m", warn: "\x1b[33m", info: "\x1b[36m" };
const RESET = "\x1b[0m";
const BOLD = "\x1b[1m";

function printHuman(report: Report): void {
  const g = report.grade;
  const gradeColor = g === "A" ? "\x1b[32m" : g === "B" ? "\x1b[32m" : g === "C" ? "\x1b[33m" : "\x1b[31m";
  console.log(`\n${BOLD}aispekt${RESET} — ${report.fileName} (${report.mode} mode)`);
  console.log(
    `${BOLD}${gradeColor}${report.score}/100 (${g})${RESET}  ·  ${report.lineCount} lines · ~${report.tokenEstimate} tokens loaded every session · rulepack v${report.rulepackVersion}\n`,
  );
  for (const n of report.notices) {
    console.log(`  ⚠️  ${n}\n`);
  }
  if (report.findings.length === 0) {
    console.log("  No findings. Lean file.\n");
  }
  for (const f of report.findings) {
    console.log(`  ${COLORS[f.severity]}${f.severity.toUpperCase().padEnd(5)}${RESET} L${String(f.line).padStart(4)}  ${BOLD}${f.ruleName}${RESET} [${f.evidenceTier}]`);
    console.log(`        ${f.message}`);
    if (f.excerpt) console.log(`        > ${f.excerpt}`);
    console.log(`        fix: ${f.recommendation}`);
    console.log(`        evidence: ${f.evidenceUrl}\n`);
  }
  for (const n of report.agentNotes) {
    console.log(`  ${BOLD}${n.agent}:${RESET} ${n.note}`);
  }
  console.log("");
}

function main(argv: string[]): number {
  const args = argv.slice(2);
  if (args.includes("--version") || args.includes("-v")) {
    console.log(PKG_VERSION);
    return 0;
  }
  let json = false;
  let min = 60;
  let target: string | undefined;
  for (let i = 0; i < args.length; i++) {
    const a = args[i]!;
    if (a === "--json") json = true;
    else if (a === "--min") {
      const v = args[++i];
      if (v === undefined || v.trim() === "" || Number.isNaN(Number(v))) {
        console.error("aispekt: --min requires a numeric value");
        return 2;
      }
      min = Number(v);
    } else if (a === "--version" || a === "-v") continue;
    else if (!a.startsWith("--") && target === undefined) target = a;
    else {
      console.error(`aispekt: unknown argument "${a}"`);
      return 2;
    }
  }
  if (!target) {
    console.error("usage: aispekt <file-or-dir> [--json] [--min <score>]");
    return 2;
  }
  let report: Report;
  try {
    report = analyze(buildInput(target));
  } catch (err) {
    console.error(`aispekt: ${err instanceof Error ? err.message : String(err)}`);
    return 2;
  }
  if (json) {
    console.log(JSON.stringify(report, null, 2));
  } else {
    printHuman(report);
  }
  return report.score >= min ? 0 : 1;
}

process.exit(main(process.argv));
