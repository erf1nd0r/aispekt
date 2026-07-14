import { readFileSync } from "node:fs";
import { join } from "node:path";
import type { RepoContext } from "../src/engine/types";

export const FIXTURES = join(import.meta.dir, "fixtures");

export function fixture(name: string): string {
  return readFileSync(join(FIXTURES, name), "utf8");
}

export const GOOD = fixture("good.md");
export const BAD = fixture("bad.md");

/** A small in-memory repo whose CLAUDE.md has drift problems. */
export function driftRepo(): { content: string; repo: RepoContext } {
  const claude = [
    "# repo instructions",
    "",
    "@docs/conventions.md",
    "",
    "- Build: `bun run build`",
    "- Ship: `bun run deploy:prod`",
    "- Handlers live in `src/api/handlers/`",
    "- Old worker code is in `src/worker/legacy/`",
    "- Never commit secrets.",
    "",
    "This project is a small demo API used for testing the prune analyzer end to end.",
  ].join("\n");
  const readme = [
    "# demo",
    "",
    "This project is a small demo API used for testing the prune analyzer end to end.",
    "",
    "Run `bun install` to start.",
  ].join("\n");
  const pkg = JSON.stringify({ name: "demo", scripts: { build: "vite build", test: "bun test" } });
  return {
    content: claude,
    repo: {
      paths: ["CLAUDE.md", "README.md", "package.json", "src/api/handlers/users.ts"],
      contents: { "CLAUDE.md": claude, "README.md": readme, "package.json": pkg },
    },
  };
}
