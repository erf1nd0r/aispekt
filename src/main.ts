import "@fontsource/jetbrains-mono/400.css";
import "@fontsource/jetbrains-mono/700.css";
import "@fontsource/ibm-plex-sans/400.css";
import "@fontsource/ibm-plex-sans/600.css";
import pack from "../rules/rulepack.json";
import { stripCommonRoot } from "./engine/parse";
import type { AnalysisInput, RepoContext, Report, RuleMeta } from "./engine/types";
import { esc, renderReport, renderError, TIER_LABEL } from "./ui/render";
import { analyze, engineReady } from "./wasmEngine";

// ---- docs: rules reference rendered from the embedded rulepack ----

const rules = (pack as { rules: RuleMeta[]; version: string; updated: string }).rules;
document.getElementById("rules-list")!.innerHTML = rules
  .map(
    (r) => `
  <article class="rule-row sev-${r.severity} tier-${r.evidenceTier}">
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

// ---- confidence ladder: ranked tier legend that filters the rules ----

const TIER_ORDER = ["measured", "official", "community", "heuristic"] as const;
const ladder = document.getElementById("tier-ladder")!;
ladder.innerHTML =
  `<span class="ladder-title">evidence confidence</span>` +
  TIER_ORDER.map((t) => {
    const count = rules.filter((r) => r.evidenceTier === t).length;
    return `<button class="ladder-step ${t}" data-tier="${t}" aria-pressed="false">
      <span class="tier ${t}">${TIER_LABEL[t]}</span><span class="ladder-count">${count}</span>
    </button>`;
  }).join("");

let activeTier: string | null = null;
ladder.addEventListener("click", (e) => {
  const btn = (e.target as HTMLElement).closest<HTMLButtonElement>(".ladder-step");
  if (!btn) return;
  activeTier = activeTier === btn.dataset["tier"] ? null : (btn.dataset["tier"] ?? null);
  for (const b of ladder.querySelectorAll<HTMLButtonElement>(".ladder-step")) {
    b.setAttribute("aria-pressed", String(b.dataset["tier"] === activeTier));
  }
  for (const row of document.querySelectorAll<HTMLElement>("#rules-list .rule-row")) {
    row.hidden = activeTier !== null && !row.classList.contains(`tier-${activeTier}`);
  }
});

// ---- scrollspy: highlight the section under the viewport in the rail ----

const tocLinks = new Map<string, HTMLAnchorElement>();
for (const a of document.querySelectorAll<HTMLAnchorElement>("#toc a[href^='#']")) {
  tocLinks.set(a.getAttribute("href")!.slice(1), a);
}
const spy = new IntersectionObserver(
  (entries) => {
    for (const entry of entries) {
      if (entry.isIntersecting) {
        for (const a of tocLinks.values()) a.classList.remove("active");
        tocLinks.get(entry.target.id)?.classList.add("active");
      }
    }
  },
  { rootMargin: "-20% 0px -70% 0px" },
);
for (const id of tocLinks.keys()) {
  const section = document.getElementById(id);
  if (section) spy.observe(section);
}

// ---- live-scoring hero: the real engine scoring a bloated sample on load ----

const HERO_SAMPLE = [
  "# My project",
  "",
  "Always write clean code and follow best practices.",
  "Use 2 spaces for indentation and single quotes everywhere.",
  "IMPORTANT: YOU MUST be careful. CRITICAL: stay focused.",
  "IMPORTANT: YOU MUST double-check. CRITICAL: NEVER EVER guess.",
  "src/",
  "  components/",
  "  utils/",
  "Handle errors appropriately, as needed.",
].join("\n");

const REDUCED_MOTION = window.matchMedia("(prefers-reduced-motion: reduce)").matches;

async function initHeroDemo(): Promise<void> {
  const demo = document.getElementById("hero-demo")!;
  const fileEl = document.getElementById("hero-file")!;
  const scoreEl = document.getElementById("hero-score")!;
  const gradeEl = document.getElementById("hero-grade")!;
  let report: Report;
  try {
    report = await analyze({ fileName: "CLAUDE.md", content: HERO_SAMPLE });
  } catch {
    return; // engine unavailable — the hero stays prose-only, docs unaffected
  }
  const lines = HERO_SAMPLE.split("\n");
  fileEl.innerHTML = lines
    .map(
      (l, i) =>
        `<div class="hero-line" data-line="${i + 1}"><span class="hero-ln">${i + 1}</span>${esc(l) || "&nbsp;"}</div>`,
    )
    .join("");
  demo.hidden = false;

  const finish = () => {
    scoreEl.textContent = String(report.score);
    gradeEl.textContent = `grade ${report.grade}`;
    demo.classList.add(`grade-${report.grade}`);
    for (const f of report.findings) {
      fileEl.querySelector(`[data-line="${f.line}"]`)?.classList.add("flagged");
    }
  };
  if (REDUCED_MOTION) {
    finish();
    return;
  }
  // deduct penalties one at a time: tick the number down, flash the line
  const steps = report.findings.map((f) => ({ line: f.line }));
  const total = 100 - report.score;
  let shown = 100;
  let i = 0;
  const tick = () => {
    if (i >= steps.length) {
      finish();
      return;
    }
    const target = 100 - Math.round((total * (i + 1)) / steps.length);
    const line = fileEl.querySelector(`[data-line="${steps[i]!.line}"]`);
    line?.classList.add("flash");
    setTimeout(() => {
      line?.classList.remove("flash");
      line?.classList.add("flagged");
    }, 340);
    const countDown = () => {
      if (shown > target) {
        shown--;
        scoreEl.textContent = String(shown);
        requestAnimationFrame(countDown);
      } else {
        i++;
        setTimeout(tick, 240);
      }
    };
    countDown();
  };
  setTimeout(tick, 500);
}

void engineReady()
  .then(() => initHeroDemo())
  .catch(() => {});

// ---- playground: same walk plumbing as always, engine now WASM ----

const results = document.getElementById("results")!;
const dropzone = document.getElementById("dropzone")!;
const fileInput = document.getElementById("file-input") as HTMLInputElement;
const dirInput = document.getElementById("dir-input") as HTMLInputElement;

const INSTRUCTION_NAMES = ["CLAUDE.md", "AGENTS.md"];
// Must stay in lockstep with SKIP_DIRS in src/cli.ts and crates/cli — a
// missing entry makes the playground and CLI walk different path sets for
// the same repo (the .venv drift this line fixes).
const SKIP_SEGMENTS = new Set(["node_modules", ".git", "dist", "build", ".next", ".venv", "vendor", "coverage"]);
const MAX_FILES = 200_000;

// Warm the engine the moment the user shows intent, so the first analysis
// doesn't pay the module fetch. engineReady memoizes and clears itself on
// failure, so repeated calls are free and a failed warm-up retries later.
function warm(): void {
  void engineReady().catch(() => {});
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
  results.innerHTML = `<div class="clean analyzing"><span class="sweep" aria-hidden="true"></span>Analyzing…</div>`;
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
