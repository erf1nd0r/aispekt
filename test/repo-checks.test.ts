import { describe, expect, test } from "bun:test";
import { analyze } from "../src/engine/analyze";
import { BAD, driftRepo } from "./helpers";

describe("repo-mode checks", () => {
  const { content, repo } = driftRepo();
  const report = analyze({ fileName: "CLAUDE.md", content, repo });

  test("ISC-22: dead-command flags scripts absent from package.json", () => {
    const f = report.findings.find((x) => x.ruleId === "dead-command");
    expect(f).toBeDefined();
    expect(f!.message).toContain("deploy:prod");
    // `bun run build` exists and must not be flagged
    expect(
      report.findings.filter((x) => x.ruleId === "dead-command" && x.message.includes('"build"')),
    ).toHaveLength(0);
  });

  test("ISC-23: stale-path flags missing paths, spares existing ones", () => {
    const stale = report.findings.filter((x) => x.ruleId === "stale-path");
    expect(stale.some((f) => f.message.includes("src/worker/legacy"))).toBe(true);
    expect(stale.some((f) => f.message.includes("src/api/handlers"))).toBe(false);
  });

  test("ISC-24: readme-redundancy flags lines duplicating README", () => {
    const f = report.findings.find((x) => x.ruleId === "readme-redundancy");
    expect(f).toBeDefined();
    expect(f!.excerpt).toContain("small demo API");
  });

  test("ISC-26: import-resolution flags unresolved @-imports", () => {
    const f = report.findings.find((x) => x.ruleId === "import-resolution");
    expect(f).toBeDefined();
    expect(f!.message).toContain("docs/conventions.md");
  });

  test("ISC-25: bridge-missing fires for AGENTS.md without CLAUDE.md bridge", () => {
    const agentsOnly = analyze({
      fileName: "AGENTS.md",
      content: "# a\nRun `bun test`. Never skip.",
      repo: { paths: ["AGENTS.md", "package.json"], contents: {} },
    });
    expect(agentsOnly.findings.some((f) => f.ruleId === "bridge-missing")).toBe(true);

    const bridged = analyze({
      fileName: "AGENTS.md",
      content: "# a\nRun `bun test`. Never skip.",
      repo: {
        paths: ["AGENTS.md", "CLAUDE.md"],
        contents: { "CLAUDE.md": "@AGENTS.md\n" },
      },
    });
    expect(bridged.findings.some((f) => f.ruleId === "bridge-missing")).toBe(false);

    const driftedBridge = analyze({
      fileName: "AGENTS.md",
      content: "# a\nRun `bun test`. Never skip.",
      repo: {
        paths: ["AGENTS.md", "CLAUDE.md"],
        contents: { "CLAUDE.md": "# separate content, no import\n" },
      },
    });
    expect(driftedBridge.findings.some((f) => f.ruleId === "bridge-missing")).toBe(true);
  });

  test("repo rules never fire in file mode", () => {
    const fileOnly = analyze({ fileName: "CLAUDE.md", content: BAD });
    const repoRuleIds = ["dead-command", "stale-path", "readme-redundancy", "bridge-missing", "import-resolution"];
    for (const id of repoRuleIds) {
      expect(fileOnly.findings.some((f) => f.ruleId === id)).toBe(false);
    }
    expect(fileOnly.mode).toBe("file");
    expect(report.mode).toBe("repo");
  });

  test("ISC-23.1: dot-directory contents are not flagged when present in paths", () => {
    const r = analyze({
      fileName: "CLAUDE.md",
      content: "# x\n`.github/workflows/ci.yml` runs the contract check. Never skip it. Run `bun test`.",
      repo: { paths: ["CLAUDE.md", ".github/workflows/ci.yml"], contents: {} },
    });
    expect(r.findings.some((f) => f.ruleId === "stale-path")).toBe(false);
  });

  test("ISC-23.2: paths under unverifiable-directory markers are not flagged", () => {
    const r = analyze({
      fileName: "CLAUDE.md",
      content: "# x\nServes the `client/dist` bundle; assets at `client/dist/assets/app.js`. Never skip tests. Run `bun test`.",
      repo: { paths: ["CLAUDE.md", "client/dist/"], contents: {} },
    });
    expect(r.findings.some((f) => f.ruleId === "stale-path")).toBe(false);
  });

  test("ISC-23.3: npm scoped specifiers are never treated as file paths", () => {
    const r = analyze({
      fileName: "CLAUDE.md",
      content: "# x\nImport auth as `@fives/shared/auth0`. Never skip tests. Run `bun test`.",
      repo: { paths: ["CLAUDE.md"], contents: {} },
    });
    expect(r.findings.some((f) => f.ruleId === "stale-path")).toBe(false);
  });

  test("ISC-23.4: hostname-like tokens are never treated as file paths", () => {
    const r = analyze({
      fileName: "CLAUDE.md",
      content: "# x\nLive at `fives.gugus.gg/play` and `auth.gugus.gg/authorize`. Never skip tests. Run `bun test`.",
      repo: { paths: ["CLAUDE.md"], contents: {} },
    });
    expect(r.findings.some((f) => f.ruleId === "stale-path")).toBe(false);
  });

  test("ISC-23.4 guard: dotted filenames with real extensions still resolve as paths", () => {
    const r = analyze({
      fileName: "CLAUDE.md",
      content: "# x\nConfig in `wrangler.toml/nope/missing.ts`. Never skip tests. Run `bun test`.",
      repo: { paths: ["CLAUDE.md"], contents: {} },
    });
    expect(r.findings.some((f) => f.ruleId === "stale-path")).toBe(true);
  });

  test("ISC-23.5: context-relative refs existing as a path suffix are not flagged", () => {
    const r = analyze({
      fileName: "CLAUDE.md",
      content: "# x\nURL handling via `lib/route.ts`. Never skip tests. Run `bun test`.",
      repo: { paths: ["CLAUDE.md", "client/src/lib/route.ts"], contents: {} },
    });
    expect(r.findings.some((f) => f.ruleId === "stale-path")).toBe(false);
  });

  test("ISC-23.6: path comparison is case-insensitive", () => {
    const r = analyze({
      fileName: "CLAUDE.md",
      content: "# x\nSee `Docs/Setup.md`. Never skip tests. Run `bun test`.",
      repo: { paths: ["CLAUDE.md", "docs/setup.md"], contents: {} },
    });
    expect(r.findings.some((f) => f.ruleId === "stale-path")).toBe(false);
  });

  test("genuinely missing paths are still flagged after the guards", () => {
    const r = analyze({
      fileName: "CLAUDE.md",
      content: "# x\nOld code in `src/worker/legacy/handler.ts`. Never skip tests. Run `bun test`.",
      repo: { paths: ["CLAUDE.md", "src/api/users.ts"], contents: {} },
    });
    expect(r.findings.some((f) => f.ruleId === "stale-path")).toBe(true);
  });

  test("truncated walks disable absence-based checks entirely (no partial-list false positives)", () => {
    const r = analyze({
      fileName: "CLAUDE.md",
      content: "# x\n@docs/missing.md\nOld code in `src/definitely/gone/`. Never skip tests. Run `bun test`.",
      repo: { paths: ["CLAUDE.md"], contents: {}, truncated: true },
    });
    expect(r.findings.some((f) => f.ruleId === "stale-path")).toBe(false);
    expect(r.findings.some((f) => f.ruleId === "import-resolution")).toBe(false);
    expect(r.notices.some((n) => n.includes("cap"))).toBe(true);
  });

  test("symlink/copy bridge: identical CLAUDE.md and AGENTS.md content is bridged", () => {
    const content = "# shared instructions\nRun `bun test`. Never skip.";
    const r = analyze({
      fileName: "AGENTS.md",
      content,
      repo: {
        paths: ["AGENTS.md", "CLAUDE.md"],
        contents: { "AGENTS.md": content, "CLAUDE.md": content },
      },
    });
    expect(r.findings.some((f) => f.ruleId === "bridge-missing")).toBe(false);
  });

  test("browser folder mode: absent CLAUDE.md carries the symlink caveat", () => {
    const r = analyze({
      fileName: "AGENTS.md",
      content: "# a\nRun `bun test`. Never skip.",
      repo: { paths: ["AGENTS.md"], contents: {}, symlinksHidden: true },
    });
    const f = r.findings.find((x) => x.ruleId === "bridge-missing");
    expect(f).toBeDefined();
    expect(f!.message).toContain("symlink");
    // the fix text must match the caveat case, not contradict it (field report, iter 7):
    expect(f!.recommendation).toContain("CLI");
    expect(f!.recommendation).not.toContain("refuses");
  });

  test("bridge finding outside browser mode keeps the generic rulepack fix", () => {
    const r = analyze({
      fileName: "AGENTS.md",
      content: "# a\nRun `bun test`. Never skip.",
      repo: { paths: ["AGENTS.md"], contents: {} },
    });
    const f = r.findings.find((x) => x.ruleId === "bridge-missing");
    expect(f).toBeDefined();
    expect(f!.recommendation).toContain("@AGENTS.md");
  });

  test("ISC-23.9: git-tag / version identifiers are never treated as file paths", () => {
    const r = analyze({
      fileName: "AGENTS.md",
      content: [
        "# corp",
        "- Service clients: `shopnavigation/{service}/serviceclient/{version}` (e.g. `shopnavigation/shopmainnavigation/serviceclient/1.2.3`)",
        "- MainNavigationPath: (e.g. `shopnavigation/mainnavigationpath/1.2.3`)",
        "- Release tag: `releases/v2.0`",
        "- Never commit secrets. Run `bun test`.",
      ].join("\n"),
      repo: { paths: ["AGENTS.md", "src/x.ts"], contents: {} },
    });
    expect(r.findings.some((f) => f.ruleId === "stale-path")).toBe(false);
  });

  test("ISC-23.9 guard: version-ish DIRECTORY refs with real file leaves still flag", () => {
    const r = analyze({
      fileName: "AGENTS.md",
      content: "# x\nAssets in `themes/2.0/logo.png`. Never skip tests. Run `bun test`.",
      repo: { paths: ["AGENTS.md", "src/x.ts"], contents: {} },
    });
    expect(r.findings.some((f) => f.ruleId === "stale-path")).toBe(true);
  });

  test("aispekt-ignore marker suppresses a finding (canonical)", () => {
    const r = analyze({
      fileName: "CLAUDE.md",
      content: "# x\n<!-- aispekt-ignore -->\nOld code in `src/gone/away.ts`. Never skip tests. Run `bun test`.",
      repo: { paths: ["CLAUDE.md"], contents: {} },
    });
    expect(r.findings.some((f) => f.ruleId === "stale-path")).toBe(false);
  });

  test("prune-ignore marker still suppresses (legacy alias)", () => {
    const flagged = analyze({
      fileName: "CLAUDE.md",
      content: "# x\nOld code in `src/gone/away.ts`. Never skip tests. Run `bun test`.",
      repo: { paths: ["CLAUDE.md"], contents: {} },
    });
    expect(flagged.findings.some((f) => f.ruleId === "stale-path")).toBe(true);

    const ignoredInline = analyze({
      fileName: "CLAUDE.md",
      content: "# x\nOld code in `src/gone/away.ts`. <!-- prune-ignore --> Never skip tests. Run `bun test`.",
      repo: { paths: ["CLAUDE.md"], contents: {} },
    });
    expect(ignoredInline.findings.some((f) => f.ruleId === "stale-path")).toBe(false);

    const ignoredAbove = analyze({
      fileName: "CLAUDE.md",
      content: "# x\n<!-- prune-ignore -->\nOld code in `src/gone/away.ts`. Never skip tests. Run `bun test`.",
      repo: { paths: ["CLAUDE.md"], contents: {} },
    });
    expect(ignoredAbove.findings.some((f) => f.ruleId === "stale-path")).toBe(false);
  });

  test("home-dir @-imports are skipped, not flagged", () => {
    const r = analyze({
      fileName: "CLAUDE.md",
      content: "@~/.claude/personal.md\nRun `bun test`. Never skip.",
      repo: { paths: ["CLAUDE.md"], contents: {} },
    });
    expect(r.findings.some((f) => f.ruleId === "import-resolution")).toBe(false);
  });
});
