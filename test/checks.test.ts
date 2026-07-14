import { describe, expect, test } from "bun:test";
import { analyze } from "../src/engine/analyze";
import { BAD, GOOD } from "./helpers";

function ruleIds(content: string, fileName = "CLAUDE.md"): Set<string> {
  return new Set(analyze({ fileName, content }).findings.map((f) => f.ruleId));
}

const badIds = ruleIds(BAD);
const goodIds = ruleIds(GOOD);

const FIRES_ON_BAD = [
  "lint-leakage",
  "directory-tree",
  "vague-rules",
  "enforcement-prose",
  "secret-leak",
  "emphasis-overuse",
  "pointer-without-instruction",
  "duplicate-rules",
  "contradiction-lite",
] as const;

describe("file-mode checks: fire on bad, silent on good", () => {
  for (const id of FIRES_ON_BAD) {
    test(`${id} fires on bad fixture`, () => {
      expect(badIds.has(id)).toBe(true);
    });
    test(`${id} silent on good fixture`, () => {
      expect(goodIds.has(id)).toBe(false);
    });
  }

  test("ISC-12: no-commands fires when zero runnable commands present", () => {
    const ids = ruleIds("# project\n\nJust be nice to the codebase.\nNever break things.");
    expect(ids.has("no-commands")).toBe(true);
    expect(goodIds.has("no-commands")).toBe(false);
    expect(badIds.has("no-commands")).toBe(false); // bad has npm/yarn commands
  });

  test("ISC-17: size-cost fires past 200 lines and is labeled heuristic", () => {
    const long = Array.from({ length: 210 }, (_, i) => `- note line ${i} with no issues`).join("\n");
    const r = analyze({ fileName: "CLAUDE.md", content: `# x\nNever break things. Run \`bun test\`.\n${long}` });
    const f = r.findings.find((x) => x.ruleId === "size-cost");
    expect(f).toBeDefined();
    expect(f!.evidenceTier).toBe("heuristic");
    expect(goodIds.has("size-cost")).toBe(false);
  });

  test("ISC-20: boundaries-missing fires when no never/ask-first exists", () => {
    const ids = ruleIds("# y\n\n- Build: `bun run build`\n- Test: `bun test`");
    expect(ids.has("boundaries-missing")).toBe(true);
    expect(goodIds.has("boundaries-missing")).toBe(false);
    expect(badIds.has("boundaries-missing")).toBe(false); // bad has NEVER push
  });

  test("ISC-21: codex-cap fires past 32 KiB", () => {
    const big = "# big\nRun `bun test`. Never skip it.\n" + "x".repeat(33 * 1024);
    const ids = ruleIds(big);
    expect(ids.has("codex-cap")).toBe(true);
    expect(goodIds.has("codex-cap")).toBe(false);
  });

  test("ISC-13: enforcement-prose spares 'never commit secrets' boundaries", () => {
    const ids = ruleIds("# z\nNever commit secrets.\nRun `bun test`.");
    expect(ids.has("enforcement-prose")).toBe(false);
  });

  test("fenced code is not scanned as prose", () => {
    const ids = ruleIds(
      "# f\nRun `bun test`. Never skip.\n```\nUse 2 spaces for indentation\nwrite clean code\n```\n",
    );
    expect(ids.has("lint-leakage")).toBe(false);
    expect(ids.has("vague-rules")).toBe(false);
  });
});
