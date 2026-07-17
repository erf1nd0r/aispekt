/**
 * WASM parity: the Rust engine compiled to wasm32-unknown-unknown (the exact
 * module the playground fetches) must produce reports identical to the TS
 * engine for the same inputs. Runs the real public/aispekt.wasm via Bun's
 * WebAssembly — no browser required; the browser pass verifies rendering,
 * this verifies the engine. The bridge protocol is imported from
 * src/wasmEngine.ts so this suite exercises the shipped code path, not a copy.
 *
 * A missing artifact FAILS the suite (it would otherwise silently skip on
 * fresh clones and green-light unverified deploys) — build it with
 * `bun run build:wasm`, or set AISPEKT_ALLOW_MISSING_WASM=1 for a TS-only run.
 */
import { describe, expect, test } from "bun:test";
import { existsSync, readFileSync } from "node:fs";
import { join } from "node:path";
import { analyze } from "../src/engine/analyze";
import type { AnalysisInput } from "../src/engine/types";
import { callEngine, type WasmExports } from "../src/wasmEngine";

const WASM_PATH = join(import.meta.dir, "..", "public", "aispekt.wasm");
const ALLOW_MISSING = process.env["AISPEKT_ALLOW_MISSING_WASM"] === "1";

async function loadEngine(): Promise<WasmExports> {
  const { instance } = await WebAssembly.instantiate(readFileSync(WASM_PATH), {});
  return instance.exports as unknown as WasmExports;
}

const FIXTURES = ["good.md", "bad.md"].map((n) => join(import.meta.dir, "fixtures", n));
const CORPUS = [
  "contradictions.md",
  "emphasis.md",
  "empty.md",
  "minimal.md",
  "secrets.md",
  "unicode.md",
  "vague.md",
  "ignores.md",
].map((n) => join(import.meta.dir, "golden", "corpus", n));

describe("wasm engine parity", () => {
  if (!existsSync(WASM_PATH)) {
    if (ALLOW_MISSING) {
      test.skip("public/aispekt.wasm missing — explicitly allowed via AISPEKT_ALLOW_MISSING_WASM", () => {});
      return;
    }
    test("public/aispekt.wasm exists (run `bun run build:wasm`)", () => {
      throw new Error(
        "public/aispekt.wasm is missing — the parity suite would silently verify nothing. " +
          "Run `bun run build:wasm`, or set AISPEKT_ALLOW_MISSING_WASM=1 to skip deliberately.",
      );
    });
    return;
  }

  test("file-mode corpus matches the TS engine exactly", async () => {
    const engine = await loadEngine();
    for (const path of [...FIXTURES, ...CORPUS]) {
      const input: AnalysisInput = {
        fileName: path.split("/").pop() ?? "CLAUDE.md",
        content: readFileSync(path, "utf8"),
      };
      const ts = JSON.parse(JSON.stringify(analyze(input)));
      const rs = callEngine(engine, input);
      expect(rs).toEqual(ts);
    }
  });

  test("repo-mode input matches the TS engine exactly", async () => {
    const engine = await loadEngine();
    const root = join(import.meta.dir, "golden", "corpus", "drift-repo");
    const contents: Record<string, string> = {};
    for (const name of ["CLAUDE.md", "AGENTS.md", "package.json", "README.md"]) {
      contents[name] = readFileSync(join(root, name), "utf8");
    }
    const input: AnalysisInput = {
      fileName: "CLAUDE.md",
      content: contents["CLAUDE.md"]!,
      repo: {
        paths: ["AGENTS.md", "CLAUDE.md", "README.md", "package.json", "src/index.js"],
        contents,
      },
    };
    const ts = JSON.parse(JSON.stringify(analyze(input)));
    const rs = callEngine(engine, input);
    expect(rs).toEqual(ts);
  });

  test("invalid input surfaces a structured error, not a crash", async () => {
    const engine = await loadEngine();
    expect(() =>
      callEngine(engine, "not an input object" as unknown as AnalysisInput),
    ).toThrow();
  });
});
