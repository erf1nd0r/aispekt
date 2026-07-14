import { describe, expect, test } from "bun:test";
import { mkdirSync, symlinkSync, writeFileSync } from "node:fs";
import { join } from "node:path";
import { FIXTURES } from "./helpers";

const ROOT = join(import.meta.dir, "..");

function runCli(args: string[]): { code: number; stdout: string; stderr: string } {
  const proc = Bun.spawnSync(["bun", "src/cli.ts", ...args], { cwd: ROOT });
  return {
    code: proc.exitCode,
    stdout: proc.stdout.toString(),
    stderr: proc.stderr.toString(),
  };
}

/** On-disk drift repo for real-filesystem dir mode (ISC-44). */
function makeRepoFixture(): string {
  const dir = join(FIXTURES, "repo");
  mkdirSync(join(dir, "src", "api", "handlers"), { recursive: true });
  mkdirSync(join(dir, ".github", "workflows"), { recursive: true });
  mkdirSync(join(dir, "dist"), { recursive: true });
  writeFileSync(
    join(dir, "CLAUDE.md"),
    [
      "# repo instructions",
      "@docs/conventions.md",
      "- Build: `bun run build`",
      "- Ship: `bun run deploy:prod`",
      "- Old code in `src/worker/legacy/`",
      "- CI runs `.github/workflows/ci.yml`; bundle lands in `dist/bundle.js`",
      "- Auth via `@acme/shared/auth0`; live at `demo.example.com/play`",
      "- Never commit secrets.",
    ].join("\n"),
  );
  writeFileSync(join(dir, "README.md"), "# demo\nRun `bun install`.\n");
  writeFileSync(
    join(dir, "package.json"),
    JSON.stringify({ name: "demo", scripts: { build: "vite build" } }),
  );
  writeFileSync(join(dir, "src", "api", "handlers", "users.ts"), "export {};\n");
  writeFileSync(join(dir, ".github", "workflows", "ci.yml"), "name: ci\n");
  writeFileSync(join(dir, "dist", "bundle.js"), "// built\n");
  return dir;
}

describe("CLI", () => {
  test("ISC-43: file mode prints score, grade, and findings", () => {
    const r = runCli(["test/fixtures/bad.md"]);
    expect(r.stdout).toContain("/100");
    expect(r.stdout).toMatch(/\((D|F)\)/);
    expect(r.stdout).toContain("evidence:");
  });

  test("ISC-45: exit 0 at/above min threshold, 1 below (default 60)", () => {
    expect(runCli(["test/fixtures/good.md"]).code).toBe(0);
    expect(runCli(["test/fixtures/bad.md"]).code).toBe(1);
    expect(runCli(["test/fixtures/bad.md", "--min", "10"]).code).toBe(0);
    expect(runCli(["test/fixtures/good.md", "--min", "101"]).code).toBe(1);
  });

  test("ISC-46: --json emits a machine-readable report", () => {
    const r = runCli(["test/fixtures/good.md", "--json"]);
    const report = JSON.parse(r.stdout) as { score: number; findings: unknown[]; rulepackVersion: string };
    expect(report.score).toBe(100);
    expect(Array.isArray(report.findings)).toBe(true);
    expect(report.rulepackVersion).toMatch(/^\d+\./);
  });

  test("ISC-44: dir mode runs repo-aware checks on the real filesystem", () => {
    const dir = makeRepoFixture();
    const r = runCli([dir, "--json", "--min", "0"]);
    expect(r.code).toBe(0);
    const report = JSON.parse(r.stdout) as {
      mode: string;
      findings: { ruleId: string; message: string }[];
    };
    expect(report.mode).toBe("repo");
    const ids = new Set(report.findings.map((f) => f.ruleId));
    expect(ids.has("dead-command")).toBe(true);
    expect(ids.has("stale-path")).toBe(true);
    expect(ids.has("import-resolution")).toBe(true);
  });

  test("ISC-23.1/.2/.3/.4: real-fs walk yields no false stale-paths (dot-dirs, dist, npm, hostname)", () => {
    const dir = makeRepoFixture();
    const r = runCli([dir, "--json", "--min", "0"]);
    const report = JSON.parse(r.stdout) as { findings: { ruleId: string; message: string }[] };
    const stale = report.findings.filter((f) => f.ruleId === "stale-path");
    expect(stale).toHaveLength(1);
    expect(stale[0]!.message).toContain("src/worker/legacy");
  });

  test("usage errors exit 2", () => {
    expect(runCli([]).code).toBe(2);
    expect(runCli(["does-not-exist.md"]).code).toBe(2);
  });

  test("ISC-23.8: symlinked CLAUDE.md is followed — recognized as a bridge, not flagged", () => {
    const dir = join(FIXTURES, "symlink-repo");
    mkdirSync(dir, { recursive: true });
    writeFileSync(join(dir, "AGENTS.md"), "# corp instructions\nRun `bun test`. Never skip.\n");
    try {
      symlinkSync("AGENTS.md", join(dir, "CLAUDE.md"));
    } catch {
      /* exists from a prior run */
    }
    const r = runCli([dir, "--json", "--min", "0"]);
    const report = JSON.parse(r.stdout) as {
      fileName: string;
      findings: { ruleId: string }[];
    };
    expect(report.fileName).toBe("CLAUDE.md");
    expect(report.findings.some((f) => f.ruleId === "bridge-missing")).toBe(false);
  });

  test("--min without a numeric value errors instead of silently passing (Forge minor #5)", () => {
    expect(runCli(["test/fixtures/bad.md", "--min"]).code).toBe(2);
    expect(runCli(["test/fixtures/bad.md", "--min", "abc"]).code).toBe(2);
  });
});
