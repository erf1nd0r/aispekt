import { describe, expect, test } from "bun:test";
import { stripCommonRoot } from "../src/engine/parse";

const MARKERS = ["CLAUDE.md", "AGENTS.md"];

describe("stripCommonRoot (ISC-29.1)", () => {
  test("strips the dropped folder's own name (drag-and-drop shape)", () => {
    const { paths, prefix } = stripCommonRoot(
      ["fives/CLAUDE.md", "fives/package.json", "fives/src/index.ts"],
      MARKERS,
    );
    expect(prefix).toBe("fives/");
    expect(paths).toContain("CLAUDE.md");
  });

  test("no-op when an instruction file is already at root", () => {
    const input = ["CLAUDE.md", "src/index.ts"];
    const { paths, prefix } = stripCommonRoot(input, MARKERS);
    expect(prefix).toBe("");
    expect(paths).toEqual(input);
  });

  test("strips multiple nested levels until a marker surfaces", () => {
    const { paths, prefix } = stripCommonRoot(
      ["outer/inner/AGENTS.md", "outer/inner/README.md"],
      MARKERS,
    );
    expect(prefix).toBe("outer/inner/");
    expect(paths).toContain("AGENTS.md");
  });

  test("stops when paths diverge and no marker exists", () => {
    const input = ["a/x.md", "b/y.md"];
    const { paths, prefix } = stripCommonRoot(input, MARKERS);
    expect(prefix).toBe("");
    expect(paths).toEqual(input);
  });

  test("directory markers with trailing slash survive stripping", () => {
    const { paths } = stripCommonRoot(
      ["fives/CLAUDE.md", "fives/node_modules/"],
      MARKERS,
    );
    expect(paths).toContain("node_modules/");
  });
});
