/**
 * WASM parity: the Rust engine compiled to wasm32-unknown-unknown (the exact
 * module the playground fetches) must produce reports identical to the TS
 * engine for the same inputs. Runs the real public/aispekt.wasm via Bun's
 * WebAssembly — no browser required; the browser pass verifies rendering,
 * this verifies the engine.
 */
import { describe, expect, test } from "bun:test";
import { existsSync, readFileSync } from "node:fs";
import { join } from "node:path";
import { analyze } from "../src/engine/analyze";
import type { AnalysisInput } from "../src/engine/types";

const WASM_PATH = join(import.meta.dir, "..", "public", "aispekt.wasm");

interface WasmExports {
  memory: WebAssembly.Memory;
  aispekt_alloc(len: number): number;
  aispekt_free(ptr: number, len: number): void;
  aispekt_analyze(ptr: number, len: number): number;
}

async function loadEngine(): Promise<WasmExports> {
  const { instance } = await WebAssembly.instantiate(readFileSync(WASM_PATH), {});
  return instance.exports as unknown as WasmExports;
}

function wasmAnalyze(e: WasmExports, input: AnalysisInput): unknown {
  const bytes = new TextEncoder().encode(JSON.stringify(input));
  const inPtr = e.aispekt_alloc(bytes.length);
  new Uint8Array(e.memory.buffer, inPtr, bytes.length).set(bytes);
  const outPtr = e.aispekt_analyze(inPtr, bytes.length);
  const len = new DataView(e.memory.buffer).getUint32(outPtr, true);
  const payload = new Uint8Array(e.memory.buffer, outPtr + 4, len).slice();
  e.aispekt_free(inPtr, bytes.length);
  e.aispekt_free(outPtr, 4 + len);
  return JSON.parse(new TextDecoder().decode(payload));
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

describe.if(existsSync(WASM_PATH))("wasm engine parity", () => {
  test("file-mode corpus matches the TS engine exactly", async () => {
    const engine = await loadEngine();
    for (const path of [...FIXTURES, ...CORPUS]) {
      const input: AnalysisInput = {
        fileName: path.split("/").pop() ?? "CLAUDE.md",
        content: readFileSync(path, "utf8"),
      };
      const ts = JSON.parse(JSON.stringify(analyze(input)));
      const rs = wasmAnalyze(engine, input);
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
    const rs = wasmAnalyze(engine, input);
    expect(rs).toEqual(ts);
  });

  test("invalid input returns a structured error, not a crash", async () => {
    const engine = await loadEngine();
    const bytes = new TextEncoder().encode("not json at all");
    const inPtr = engine.aispekt_alloc(bytes.length);
    new Uint8Array(engine.memory.buffer, inPtr, bytes.length).set(bytes);
    const outPtr = engine.aispekt_analyze(inPtr, bytes.length);
    const len = new DataView(engine.memory.buffer).getUint32(outPtr, true);
    const payload = JSON.parse(
      new TextDecoder().decode(new Uint8Array(engine.memory.buffer, outPtr + 4, len).slice()),
    );
    engine.aispekt_free(inPtr, bytes.length);
    engine.aispekt_free(outPtr, 4 + len);
    expect(payload.error).toBeDefined();
  });
});
