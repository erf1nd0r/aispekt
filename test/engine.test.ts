import { describe, expect, test } from "bun:test";
import { analyze, rulepack, validatePack } from "../src/engine/analyze";
import type { CheckFn } from "../src/engine/types";
import { BAD, GOOD } from "./helpers";

describe("engine core", () => {
  test("ISC-2: score is an integer 0-100 with grade from fixed thresholds", () => {
    for (const content of [GOOD, BAD, "", "# tiny"]) {
      const r = analyze({ fileName: "CLAUDE.md", content });
      expect(Number.isInteger(r.score)).toBe(true);
      expect(r.score).toBeGreaterThanOrEqual(0);
      expect(r.score).toBeLessThanOrEqual(100);
      const expected =
        r.score >= 90 ? "A" : r.score >= 75 ? "B" : r.score >= 60 ? "C" : r.score >= 40 ? "D" : "F";
      expect(r.grade).toBe(expected);
    }
  });

  test("ISC-3: scoring is penalty-only — base 100, every penalty positive", () => {
    const r = analyze({ fileName: "CLAUDE.md", content: BAD });
    const total = r.penalties.reduce((s, p) => s + p.penalty, 0);
    expect(r.score).toBe(Math.max(0, Math.min(100, Math.round(100 - total))));
    for (const p of r.penalties) expect(p.penalty).toBeGreaterThan(0);
    const clean = analyze({ fileName: "CLAUDE.md", content: GOOD });
    expect(clean.score).toBe(100);
  });

  test("ISC-4: every finding carries ruleId, severity, line, excerpt, recommendation", () => {
    const r = analyze({ fileName: "CLAUDE.md", content: BAD });
    expect(r.findings.length).toBeGreaterThan(0);
    for (const f of r.findings) {
      expect(f.ruleId.length).toBeGreaterThan(0);
      expect(["error", "warn", "info"]).toContain(f.severity);
      expect(f.line).toBeGreaterThanOrEqual(1);
      expect(typeof f.excerpt).toBe("string");
      expect(f.message.length).toBeGreaterThan(0);
      expect(f.recommendation.length).toBeGreaterThan(0);
    }
  });

  test("ISC-5: finding metadata comes from the rulepack", () => {
    const r = analyze({ fileName: "CLAUDE.md", content: BAD });
    for (const f of r.findings) {
      const meta = rulepack.rules.find((rule) => rule.id === f.ruleId);
      expect(meta).toBeDefined();
      expect(f.evidenceUrl).toBe(meta!.evidenceUrl);
      expect(f.evidenceTier).toBe(meta!.evidenceTier);
      expect(f.severity).toBe(meta!.severity);
      expect(f.recommendation).toBe(meta!.recommendation);
    }
  });

  test("ISC-6: rulepack/implementation mismatch throws in both directions", () => {
    const impl: CheckFn = () => [];
    const fakePack = {
      version: "0.0.0",
      updated: "2026-01-01",
      rules: [
        {
          id: "ghost-rule",
          name: "x",
          severity: "info" as const,
          weight: 1,
          maxPenalty: 1,
          evidenceTier: "heuristic" as const,
          evidenceUrl: "https://example.com",
          lastVerified: "2026-01-01",
          summary: "x",
          recommendation: "x",
          scope: "file" as const,
        },
      ],
    };
    expect(() => validatePack(fakePack, {})).toThrow(/no implementation/);
    expect(() => validatePack({ ...fakePack, rules: [] }, { orphan: impl })).toThrow(
      /not in the rulepack/,
    );
  });

  test("ISC-7: report surfaces rulepack version and update date", () => {
    const r = analyze({ fileName: "CLAUDE.md", content: GOOD });
    expect(r.rulepackVersion).toBe(rulepack.version);
    expect(r.rulepackUpdated).toBe(rulepack.updated);
    for (const rule of rulepack.rules) {
      expect(rule.lastVerified).toMatch(/^\d{4}-\d{2}-\d{2}$/);
    }
  });

  test("ISC-8: report includes token estimate and byte size", () => {
    const r = analyze({ fileName: "CLAUDE.md", content: GOOD });
    expect(r.tokenEstimate).toBe(Math.ceil(GOOD.length / 4));
    expect(r.byteSize).toBeGreaterThan(0);
    expect(r.lineCount).toBe(GOOD.split("\n").length);
  });

  test("agent notes cover the four major agents", () => {
    const r = analyze({ fileName: "AGENTS.md", content: GOOD });
    const agents = r.agentNotes.map((n) => n.agent);
    expect(agents).toEqual(["Claude Code", "OpenAI Codex", "Cursor", "GitHub Copilot"]);
    expect(r.agentNotes[0]!.note).toContain("@AGENTS.md");
  });
});
