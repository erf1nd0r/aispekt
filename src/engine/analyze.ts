import pack from "../../rules/rulepack.json";
import type {
  AgentNote,
  AnalysisInput,
  CheckFn,
  Finding,
  Grade,
  Report,
  RuleMeta,
  RulePack,
  RulePenalty,
} from "./types";
import { buildContext, estimateTokens } from "./parse";
import {
  boundariesMissing,
  codexCap,
  contradictionLite,
  directoryTree,
  duplicateRules,
  emphasisOveruse,
  enforcementProse,
  lintLeakage,
  noCommands,
  pointerWithoutInstruction,
  secretLeak,
  sizeCost,
  vagueRules,
} from "./checks/file-checks";
import {
  bridgeMissing,
  deadCommand,
  importResolution,
  readmeRedundancy,
  stalePath,
} from "./checks/repo-checks";

export const rulepack = pack as RulePack;

const IMPLS: Record<string, CheckFn> = {
  "lint-leakage": lintLeakage,
  "directory-tree": directoryTree,
  "vague-rules": vagueRules,
  "no-commands": noCommands,
  "enforcement-prose": enforcementProse,
  "secret-leak": secretLeak,
  "emphasis-overuse": emphasisOveruse,
  "pointer-without-instruction": pointerWithoutInstruction,
  "size-cost": sizeCost,
  "duplicate-rules": duplicateRules,
  "contradiction-lite": contradictionLite,
  "boundaries-missing": boundariesMissing,
  "codex-cap": codexCap,
  "dead-command": deadCommand,
  "stale-path": stalePath,
  "readme-redundancy": readmeRedundancy,
  "bridge-missing": bridgeMissing,
  "import-resolution": importResolution,
};

/**
 * Rulepack and implementations must agree exactly — a rule with no code and
 * code with no rule are both configuration drift, and drift throws.
 */
export function validatePack(rp: RulePack, impls: Record<string, CheckFn>): void {
  const packIds = new Set(rp.rules.map((r) => r.id));
  const implIds = new Set(Object.keys(impls));
  for (const id of packIds) {
    if (!implIds.has(id)) throw new Error(`rulepack rule "${id}" has no implementation`);
  }
  for (const id of implIds) {
    if (!packIds.has(id)) throw new Error(`implementation "${id}" is not in the rulepack`);
  }
  for (const r of rp.rules) {
    // https-only is a security invariant, not a style rule: evidenceUrl lands
    // in an href, and a javascript:/data: value there would be XSS if a rule
    // pack ever sourced URLs from outside the bundle.
    if (!/^https:\/\//.test(r.evidenceUrl))
      throw new Error(`rule "${r.id}" evidenceUrl must be https://`);
    if (r.weight <= 0 || r.maxPenalty <= 0)
      throw new Error(`rule "${r.id}" has non-positive weight/maxPenalty`);
  }
}

validatePack(rulepack, IMPLS);

function gradeOf(score: number): Grade {
  if (score >= 90) return "A";
  if (score >= 75) return "B";
  if (score >= 60) return "C";
  if (score >= 40) return "D";
  return "F";
}

function agentNotes(input: AnalysisInput, lineCount: number, byteSize: number): AgentNote[] {
  const notes: AgentNote[] = [];
  const isClaude = /claude\.md$/i.test(input.fileName);
  const isAgents = /agents\.md$/i.test(input.fileName);
  notes.push({
    agent: "Claude Code",
    note:
      (isAgents
        ? "Does not read AGENTS.md natively — needs a CLAUDE.md containing `@AGENTS.md`. "
        : "") +
      `Injects ${isClaude ? "this file" : "CLAUDE.md"} as an advisory user message (no compliance guarantee); official size target is under 200 lines${lineCount > 200 ? ` — this file is ${lineCount}` : ""}. Zero-exception rules belong in hooks.`,
  });
  notes.push({
    agent: "OpenAI Codex",
    note:
      byteSize > 32 * 1024
        ? `Truncates at 32 KiB — this file is ${(byteSize / 1024).toFixed(1)} KiB, everything past the cap is silently dropped.`
        : `Reads AGENTS.md natively (nearest file wins, 32 KiB cap — this file is ${(byteSize / 1024).toFixed(1)} KiB). Supports AGENTS.override.md for local tweaks.`,
  });
  notes.push({
    agent: "Cursor",
    note: "Reads AGENTS.md natively. Path-scoped rules need .cursor/rules/*.mdc — plain .md files there are silently ignored.",
  });
  notes.push({
    agent: "GitHub Copilot",
    note: "Reads AGENTS.md natively (coding agent, since Aug 2025) alongside .github/copilot-instructions.md.",
  });
  return notes;
}

export function analyze(input: AnalysisInput): Report {
  const ctx = buildContext(input.fileName, input.content, input.repo);
  const mode = input.repo ? "repo" : "file";

  const findings: Finding[] = [];
  const penalties: RulePenalty[] = [];
  let totalPenalty = 0;

  // Escape hatch for heuristics: an `aispekt-ignore` marker (legacy: prune-ignore) on the finding's
  // line or the line above suppresses it (like eslint-disable-next-line).
  const isIgnored = (line: number): boolean =>
    /(aispekt|prune)-ignore/.test(ctx.lines[line - 1] ?? "") ||
    /(aispekt|prune)-ignore/.test(ctx.lines[line - 2] ?? "");

  for (const rule of rulepack.rules) {
    if (rule.scope === "repo" && !input.repo) continue;
    const impl = IMPLS[rule.id];
    if (!impl) throw new Error(`no implementation for rule "${rule.id}"`);
    const raw = impl(ctx).filter((f) => !isIgnored(f.line));
    if (raw.length === 0) continue;
    for (const f of raw) {
      findings.push({
        ...f,
        ruleId: rule.id,
        ruleName: rule.name,
        severity: rule.severity,
        recommendation: f.recommendation ?? rule.recommendation,
        evidenceUrl: rule.evidenceUrl,
        evidenceTier: rule.evidenceTier,
      });
    }
    const penalty = Math.min(rule.maxPenalty, raw.length * rule.weight);
    penalties.push({ ruleId: rule.id, count: raw.length, penalty });
    totalPenalty += penalty;
  }

  const score = Math.max(0, Math.min(100, Math.round(100 - totalPenalty)));
  const byteSize = new TextEncoder().encode(input.content).length;

  const severityOrder = { error: 0, warn: 1, info: 2 } as const;
  findings.sort(
    (a, b) =>
      severityOrder[a.severity] - severityOrder[b.severity] ||
      a.line - b.line ||
      a.ruleId.localeCompare(b.ruleId),
  );

  const notices: string[] = [];
  if (input.repo?.truncated) {
    notices.push(
      `Repo walk hit the ${input.repo.paths.length.toLocaleString()}-path cap — stale-path and @-import drift checks were skipped rather than run against a partial file list.`,
    );
  }

  return {
    fileName: input.fileName,
    mode,
    score,
    grade: gradeOf(score),
    lineCount: ctx.lines.length,
    byteSize,
    tokenEstimate: estimateTokens(input.content),
    findings,
    penalties,
    agentNotes: agentNotes(input, ctx.lines.length, byteSize),
    notices,
    rulepackVersion: rulepack.version,
    rulepackUpdated: rulepack.updated,
  };
}

export function ruleMeta(id: string): RuleMeta | undefined {
  return rulepack.rules.find((r) => r.id === id);
}
