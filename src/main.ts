import { analyze, rulepack } from "./engine/analyze";
import { stripCommonRoot } from "./engine/parse";
import type { AnalysisInput, RepoContext, Report } from "./engine/types";
import { renderReport, renderError } from "./ui/render";

const app = document.getElementById("app")!;

app.innerHTML = `
  <div class="wrap">
    <header class="hero">
      <h1><span class="scissors">🔍</span> aispekt</h1>
      <p class="tag">
        Is your <strong>AGENTS.md / CLAUDE.md</strong> helping your coding agent — or silently taxing it?
        Drop the file (or your whole repo folder) and get an evidence-cited answer:
        what to cut, what to fix, and why, with the study or official doc behind every finding.
      </p>
      <p class="privacy">100% in your browser. Nothing is uploaded, ever.</p>
    </header>

    <div id="dropzone" role="button" tabindex="0" aria-label="Drop an instruction file or repo folder">
      <div class="big">Drop AGENTS.md / CLAUDE.md here — or a whole repo folder</div>
      <div class="sub">Folder drops unlock repo-aware checks: dead commands, stale paths, README redundancy</div>
    </div>
    <!-- hidden pickers live OUTSIDE the dropzone: a programmatic input.click()
         bubbles, and the dropzone's click handler would re-fire the file picker
         over the folder picker (the "Choose folder shows .md filter" bug) -->
    <input type="file" id="file-input" accept=".md,.markdown,.txt" hidden />
    <input type="file" id="dir-input" webkitdirectory hidden />
    <div class="alt-actions">
      <button class="ghost" id="pick-file">Choose file</button>
      <button class="ghost" id="pick-dir">Choose folder</button>
      <button class="ghost" id="toggle-paste">Paste text instead</button>
    </div>
    <div id="paste-area">
      <textarea id="paste-text" placeholder="Paste your AGENTS.md / CLAUDE.md content here…" spellcheck="false"></textarea>
      <button class="primary" id="analyze-paste">Analyze</button>
    </div>

    <div id="results"></div>

    <footer>
      <span>rulepack v${rulepack.version} · evidence verified ${rulepack.updated} · ${rulepack.rules.length} rules</span>
      <span>analysis is deterministic and local · <a href="https://www.npmjs.com/package/aispekt" rel="noopener">npm</a> · <a href="https://github.com/erf1nd0r/aispekt" rel="noopener">github</a> · <code>bunx aispekt &lt;repo&gt;</code></span>
    </footer>
  </div>
`;

const results = document.getElementById("results")!;
const dropzone = document.getElementById("dropzone")!;
const fileInput = document.getElementById("file-input") as HTMLInputElement;
const dirInput = document.getElementById("dir-input") as HTMLInputElement;

const INSTRUCTION_NAMES = ["CLAUDE.md", "AGENTS.md"];
const SKIP_SEGMENTS = new Set(["node_modules", ".git", "dist", "build", ".next", "vendor", "coverage"]);
const MAX_FILES = 200_000;

function show(report: Report): void {
  results.innerHTML = renderReport(report);
  results.scrollIntoView({ behavior: "smooth", block: "start" });
}

function fail(message: string): void {
  results.innerHTML = renderError(message);
}

function analyzeSingle(fileName: string, content: string): void {
  show(analyze({ fileName, content }));
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
  const input: AnalysisInput = {
    fileName: instruction,
    content: contents[instruction] ?? "",
    repo,
  };
  show(analyze(input));
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

document.getElementById("pick-file")!.addEventListener("click", () => fileInput.click());
document.getElementById("pick-dir")!.addEventListener("click", () => dirInput.click());

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
