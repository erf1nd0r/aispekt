import { describe, expect, test } from "bun:test";
import { analyze } from "../src/engine/analyze";
import { BAD, GOOD } from "./helpers";

describe("calibration against the evidence", () => {
  const good = analyze({ fileName: "CLAUDE.md", content: GOOD });
  const bad = analyze({ fileName: "CLAUDE.md", content: BAD });

  test("ISC-39: minimal command-focused good file scores >= 90 (no completeness punishment)", () => {
    expect(good.score).toBeGreaterThanOrEqual(90);
    expect(good.grade).toBe("A");
  });

  test("ISC-40: bad fixture scores at least 30 points below good fixture", () => {
    expect(bad.score).toBeLessThanOrEqual(good.score - 30);
  });

  test("bad fixture lands in the failing grades", () => {
    expect(["D", "F"]).toContain(bad.grade);
  });
});
