/**
 * Bridge to the Rust engine compiled to wasm32-unknown-unknown (hand-rolled
 * C ABI, no wasm-bindgen). Protocol: write UTF-8 JSON AnalysisInput into
 * guest memory, call aispekt_analyze, read back a 4-byte-LE-length-prefixed
 * UTF-8 JSON report, free both buffers.
 *
 * The module is ~1.1 MB and lazy-loads on first analysis, so reading the
 * docs costs nothing.
 */
import type { AnalysisInput, Report } from "./engine/types";

interface WasmExports {
  memory: WebAssembly.Memory;
  aispekt_alloc(len: number): number;
  aispekt_free(ptr: number, len: number): void;
  aispekt_analyze(ptr: number, len: number): number;
}

let exportsPromise: Promise<WasmExports> | null = null;

async function load(): Promise<WasmExports> {
  // arrayBuffer + instantiate avoids the strict application/wasm MIME
  // requirement of instantiateStreaming across static hosts.
  const resp = await fetch("/aispekt.wasm");
  if (!resp.ok) throw new Error(`engine fetch failed: HTTP ${resp.status}`);
  const { instance } = await WebAssembly.instantiate(await resp.arrayBuffer(), {});
  return instance.exports as unknown as WasmExports;
}

export function engineReady(): Promise<unknown> {
  return (exportsPromise ??= load());
}

export async function analyze(input: AnalysisInput): Promise<Report> {
  const e = await (exportsPromise ??= load());
  const bytes = new TextEncoder().encode(JSON.stringify(input));
  const inPtr = e.aispekt_alloc(bytes.length);
  new Uint8Array(e.memory.buffer, inPtr, bytes.length).set(bytes);
  const outPtr = e.aispekt_analyze(inPtr, bytes.length);
  // Views must be re-taken after calls: guest allocation can grow (and
  // detach) the memory buffer.
  const len = new DataView(e.memory.buffer).getUint32(outPtr, true);
  const payload = new Uint8Array(e.memory.buffer, outPtr + 4, len).slice();
  e.aispekt_free(inPtr, bytes.length);
  e.aispekt_free(outPtr, 4 + len);
  const parsed = JSON.parse(new TextDecoder().decode(payload)) as Report & { error?: string };
  if (parsed.error) throw new Error(parsed.error);
  return parsed;
}
