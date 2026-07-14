import { describe, expect, test } from "bun:test";
import { rulepack } from "../src/engine/analyze";

describe("rulepack integrity", () => {
  test("version and updated are set", () => {
    expect(rulepack.version).toMatch(/^\d+\.\d+\.\d+$/);
    expect(rulepack.updated).toMatch(/^\d{4}-\d{2}-\d{2}$/);
  });

  test("rule ids are unique", () => {
    const ids = rulepack.rules.map((r) => r.id);
    expect(new Set(ids).size).toBe(ids.length);
  });

  test("ISC-41: every rule has a non-empty evidenceUrl (trust antecedent)", () => {
    for (const r of rulepack.rules) {
      expect(r.evidenceUrl).toMatch(/^https:\/\//);
    }
  });

  test("every rule has valid enums, positive weights, and a summary", () => {
    for (const r of rulepack.rules) {
      expect(["error", "warn", "info"]).toContain(r.severity);
      expect(["measured", "official", "community", "heuristic"]).toContain(r.evidenceTier);
      expect(["file", "repo"]).toContain(r.scope);
      expect(r.weight).toBeGreaterThan(0);
      expect(r.maxPenalty).toBeGreaterThanOrEqual(r.weight);
      expect(r.summary.length).toBeGreaterThan(20);
      expect(r.recommendation.length).toBeGreaterThan(10);
      expect(r.lastVerified).toMatch(/^\d{4}-\d{2}-\d{2}$/);
    }
  });

  test("total possible penalty exceeds 100 so the floor is reachable", () => {
    const total = rulepack.rules.reduce((s, r) => s + r.maxPenalty, 0);
    expect(total).toBeGreaterThan(100);
  });
});
