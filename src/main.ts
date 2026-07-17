import pack from "../rules/rulepack.json";
import { stripCommonRoot } from "./engine/parse";
import type { AnalysisInput, RepoContext, Report, RuleMeta } from "./engine/types";
import { renderReport, renderError } from "./ui/render";
import { analyze, engineReady } from "./wasmEngine";

// ---- docs: rules reference rendered from the embedded rulepack ----

function esc(s: string): string {
  return s.replaceAll("&", "&amp;").replaceAll("<", "&lt;").replaceAll(">", "&gt;");
}

const TIER_LABEL: Record<string, string> = {
  measured: "measured study",
  official: "official docs",
  community: "community",
  heuristic: "heuristic",
};

const rules = (pack as { rules: RuleMeta[]; version: string; updated: string }).rules;
document.getElementById("rules-list")!.innerHTML = rules
  .map(
    (r) => `
  <article class="rule-row sev-${r.severity}">
    <div class="head">
      <span class="sev">${r.severity}</span>
      <span class="rule">${esc(r.name)}</span>
      <code class="rule-id">${esc(r.id)}</code>
      <span class="scope-chip">${r.scope}</span>
      <a class="tier ${r.evidenceTier}" href="${esc(r.evidenceUrl)}" target="_blank" rel="noopener"
         title="Open the evidence behind this rule">${TIER_LABEL[r.evidenceTier] ?? r.evidenceTier} ↗</a>
    </div>
    <p class="msg">${esc(r.summary)}</p>
    <p class="fix"><b>Fix:</b> ${esc(r.recommendation)}</p>
  </article>`,
  )
  .join("");

document.getElementById("footer-pack")!.textContent =
  `rulepack v${pack.version} · evidence verified ${pack.updated} · ${rules.length} rules`;

// ---- playground: same walk plumbing as always, engine now WASM ----

const results = document.getElementById("results")!;
const dropzone = document.getElementById("dropzone")!;
const fileInput = document.getElementById("file-input") as HTMLInputElement;
const dirInput = document.getElementById("dir-input") as HTMLInputElement;

const INSTRUCTION_NAMES = ["CLAUDE.md", "AGENTS.md"];
const SKIP_SEGMENTS = new Set(["node_modules", ".git", "dist", "build", ".next", "vendor", "coverage"]);
const MAX_FILES = 200_000;

// Warm the engine the moment the user shows intent, so the first analysis
// doesn't pay the module fetch.
let warmed = false;
function warm(): void {
  if (!warmed) {
    warmed = true;
    void engineReady().catch(() => {
      warmed = false;
    });
  }
}
dropzone.addEventListener("pointerenter", warm, { once: false });
dropzone.addEventListener("focus", warm);

function show(report: Report): void {
  results.innerHTML = renderReport(report);
  results.scrollIntoView({ behavior: "smooth", block: "start" });
}

function fail(message: string): void {
  results.innerHTML = renderError(message);
}

function pending(): void {
  results.innerHTML = `<div class="clean">Analyzing…</div>`;
}

async function runAnalysis(input: AnalysisInput): Promise<void> {
  pending();
  try {
    show(await analyze(input));
  } catch (err) {
    fail(`Analysis failed: ${err instanceof Error ? err.message : String(err)}`);
  }
}

function analyzeSingle(fileName: string, content: string): void {
  void runAnalysis({ fileName, content });
}

/**
 * Files under skipped dirs become unverifiable-directory markers ("x/dist/")
 * instead of vanishing — the checker must never flag what it didn't look at.
 */
function pushWithSkipMarkers(rel: string, paths: string[], markers: Set<string>): boolean {
  const parts = rel.split("/");
  const skipIdx = parts.findIndex((seg) => SKIP_SEGMENTS.has(seg));
  if (skipIdx >= 0) {
    markers.add(parts.slice(0, skipIdx + 1).join("/") + "/");
    return false;
  }
  paths.push(rel);
  return true;
}

/**
 * webkitdirectory input: File objects with webkitRelativePath. FileList order
 * is traversal order (deep dirs may enumerate before root files!), so shallow
 * paths (≤2 segments) are always kept even past the cap — the root
 * CLAUDE.md/AGENTS.md must never be a truncation casualty.
 */
async function analyzeFileList(files: FileList): Promise<void> {
  const paths: string[] = [];
  const markers = new Set<string>();
  const byPath = new Map<string, File>();
  let truncated = false;
  for (const f of Array.from(files)) {
    const rel = f.webkitRelativePath || f.name;
    const shallow = rel.split("/").length <= 2;
    if (paths.length >= MAX_FILES && !shallow) {
      truncated = true;
      continue;
    }
    if (pushWithSkipMarkers(rel, paths, markers)) byPath.set(rel, f);
  }
  await analyzeRepo(
    [...paths, ...markers],
    (p) => byPath.get(p)?.text() ?? Promise.resolve(undefined),
    { truncated, symlinksHidden: true },
  );
}

function readAllEntries(dir: FileSystemDirectoryEntry): Promise<FileSystemEntry[]> {
  const reader = dir.createReader();
  return new Promise<FileSystemEntry[]>((resolve) => {
    const all: FileSystemEntry[] = [];
    const readBatch = () =>
      reader.readEntries((batch) => {
        if (batch.length === 0) return resolve(all);
        all.push(...batch);
        readBatch();
      }, () => resolve(all));
    readBatch();
  });
}

/**
 * Drag-and-drop directory: BREADTH-first walk. Depth-first exhausted the file
 * cap inside the first deep subtree and never reached the root AGENTS.md
 * (the "No CLAUDE.md or AGENTS.md found" bug on large repos) — BFS guarantees
 * shallow files enumerate before deep ones.
 */
async function walkEntryBfs(
  root: FileSystemEntry,
  paths: string[],
  readers: Map<string, () => Promise<string | undefined>>,
): Promise<{ truncated: boolean }> {
  const queue: { entry: FileSystemEntry; prefix: string }[] = [{ entry: root, prefix: "" }];
  let truncated = false;
  while (queue.length > 0) {
    const { entry, prefix } = queue.shift()!;
    if (entry.isFile) {
      if (paths.length >= MAX_FILES) {
        truncated = true;
        continue;
      }
      const rel = prefix + entry.name;
      paths.push(rel);
      readers.set(rel, () =>
        new Promise<string | undefined>((resolve) => {
          (entry as FileSystemFileEntry).file(
            (f) => f.text().then(resolve, () => resolve(undefined)),
            () => resolve(undefined),
          );
        }),
      );
    } else if (entry.isDirectory) {
      if (SKIP_SEGMENTS.has(entry.name)) {
        paths.push(prefix + entry.name + "/"); // unverifiable-directory marker
        continue;
      }
      const children = await readAllEntries(entry as FileSystemDirectoryEntry);
      for (const child of children) queue.push({ entry: child, prefix: prefix + entry.name + "/" });
    }
  }
  return { truncated };
}

async function analyzeRepo(
  rawPaths: string[],
  rawRead: (path: string) => Promise<string | undefined>,
  flags: { truncated?: boolean; symlinksHidden?: boolean } = {},
): Promise<void> {
  // Dropped/picked folders carry the folder's own name as a shared prefix —
  // strip it so root-level CLAUDE.md/AGENTS.md are found (ISC-29.1).
  const { paths, prefix } = stripCommonRoot(rawPaths.sort(), INSTRUCTION_NAMES);
  const read = (p: string) => rawRead(prefix + p);
  const instruction = INSTRUCTION_NAMES.find((n) => paths.includes(n));
  if (!instruction) {
    fail("No CLAUDE.md or AGENTS.md found at the root of the dropped folder.");
    return;
  }
  const contents: Record<string, string> = {};
  for (const name of [...INSTRUCTION_NAMES, "package.json", "README.md"]) {
    if (paths.includes(name)) {
      const text = await read(name);
      if (text !== undefined) contents[name] = text;
    }
  }
  const repo: RepoContext = { paths, contents, ...flags };
  await runAnalysis({ fileName: instruction, content: contents[instruction] ?? "", repo });
}

async function handleDrop(dt: DataTransfer): Promise<void> {
  const items = Array.from(dt.items);
  const entries = items
    .map((i) => i.webkitGetAsEntry?.())
    .filter((e): e is FileSystemEntry => e != null);
  const dirEntry = entries.find((e) => e.isDirectory);
  if (dirEntry) {
    const paths: string[] = [];
    const readers = new Map<string, () => Promise<string | undefined>>();
    const { truncated } = await walkEntryBfs(dirEntry, paths, readers);
    await analyzeRepo(paths, (p) => readers.get(p)?.() ?? Promise.resolve(undefined), {
      truncated,
      symlinksHidden: true,
    });
    return;
  }
  const file = dt.files[0];
  if (!file) {
    fail("Nothing readable was dropped.");
    return;
  }
  analyzeSingle(file.name, await file.text());
}

dropzone.addEventListener("dragover", (e) => {
  e.preventDefault();
  warm();
  dropzone.classList.add("dragover");
});
dropzone.addEventListener("dragleave", () => dropzone.classList.remove("dragover"));
dropzone.addEventListener("drop", (e) => {
  e.preventDefault();
  dropzone.classList.remove("dragover");
  if (e.dataTransfer) void handleDrop(e.dataTransfer);
});
dropzone.addEventListener("click", () => fileInput.click());
dropzone.addEventListener("keydown", (e) => {
  if (e.key === "Enter" || e.key === " ") fileInput.click();
});

document.getElementById("pick-file")!.addEventListener("click", () => {
  warm();
  fileInput.click();
});
document.getElementById("pick-dir")!.addEventListener("click", () => {
  warm();
  dirInput.click();
});

fileInput.addEventListener("change", async () => {
  const f = fileInput.files?.[0];
  if (f) analyzeSingle(f.name, await f.text());
  fileInput.value = "";
});
dirInput.addEventListener("change", async () => {
  if (dirInput.files && dirInput.files.length > 0) await analyzeFileList(dirInput.files);
  dirInput.value = "";
});

const pasteArea = document.getElementById("paste-area")!;
document.getElementById("toggle-paste")!.addEventListener("click", () => {
  warm();
  pasteArea.classList.toggle("open");
});
document.getElementById("analyze-paste")!.addEventListener("click", () => {
  const text = (document.getElementById("paste-text") as HTMLTextAreaElement).value;
  if (text.trim().length === 0) {
    fail("Paste some content first.");
    return;
  }
  analyzeSingle("CLAUDE.md", text);
});
