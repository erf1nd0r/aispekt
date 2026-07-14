import { describe, expect, test } from "bun:test";
import fc from "fast-check";
import { analyze } from "../src/engine/analyze";

/**
 * Bloat lines are chosen so they can only ADD findings, never satisfy an
 * absence-based rule: no command tokens, no boundary words (never/do not/
 * must/ask), no emphasis markers, no doc links, no backticks.
 */
const BLOAT_LINES = [
  "├── src/",
  "│   ├── components/",
  "│   └── utils/",
  "└── public/",
  "please write clean code in this repo",
  "follow best practices at all times",
  "the code here should be high quality code",
  "keep it simple and use meaningful names",
] as const;

const lineArb = fc.oneof(
  fc.constantFrom(
    "# heading",
    "- Build: `bun run build`",
    "- Test: `bun test`",
    "Never commit secrets.",
    "Some ordinary prose about the project.",
    "IMPORTANT: check the config.",
    "",
  ),
  fc.string({ maxLength: 60 }).filter((s) => !s.includes("\r")),
);

const contentArb = fc
  .array(lineArb, { maxLength: 40 })
  .map((lines) => lines.join("\n"));

const bloatArb = fc
  .array(fc.constantFrom(...BLOAT_LINES), { minLength: 1, maxLength: 30 })
  .map((lines) => lines.join("\n"));

describe("engine properties", () => {
  test("ISC-1: determinism — analyze(x) is byte-identical across calls", () => {
    fc.assert(
      fc.property(contentArb, (content) => {
        const a = analyze({ fileName: "CLAUDE.md", content });
        const b = analyze({ fileName: "CLAUDE.md", content });
        expect(JSON.stringify(a)).toBe(JSON.stringify(b));
      }),
      { numRuns: 1000 },
    );
  });

  test("ISC-38: appending bloat never raises the score", () => {
    fc.assert(
      fc.property(contentArb, bloatArb, (content, bloat) => {
        const base = analyze({ fileName: "CLAUDE.md", content }).score;
        const bloated = analyze({
          fileName: "CLAUDE.md",
          content: content + "\n" + bloat,
        }).score;
        expect(bloated).toBeLessThanOrEqual(base);
      }),
      { numRuns: 1000 },
    );
  });

  test("score is always an integer in [0,100]", () => {
    fc.assert(
      fc.property(contentArb, (content) => {
        const r = analyze({ fileName: "AGENTS.md", content });
        expect(Number.isInteger(r.score)).toBe(true);
        expect(r.score).toBeGreaterThanOrEqual(0);
        expect(r.score).toBeLessThanOrEqual(100);
      }),
      { numRuns: 1000 },
    );
  });
});
